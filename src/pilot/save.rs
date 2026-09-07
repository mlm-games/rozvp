//! Pilot save: RON `save.ron` through the rustbox storage pattern.
//! `game_utils::save_store::SaveStore` over `FsStorage` (OPFS on wasm, so
//! one path covers desktop and web) with the `is_intact_ron` validator:
//! crash-safe temp+rename writes, `.bak` rotation, corrupt quarantine.
//! Same struct shape as the retired bevy build, so old files still load.

use std::path::{Path, PathBuf};

use game_utils::save_store::{LoadStatus, SaveStore};
use game_utils::storage::FsStorage;
use serde::{Deserialize, Serialize};

use super::state::PilotUi;

pub const SAVE_VERSION: u32 = 1;
pub const SAVE_FILE_NAME: &str = "save.ron";

#[derive(Clone, Serialize, Deserialize)]
pub struct SettingsData {
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub music_volume: f32,
    pub language: String,
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            sfx_volume: 1.0,
            music_volume: 0.8,
            language: "en".to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SaveData {
    #[serde(default)]
    pub version: u32,
    pub high_score: u32,
    /// Adventure progress, 0-based (0 => next level is "1-1").
    #[serde(default)]
    pub adventure_level: u32,
    /// Seed names unlocked through level completion.
    #[serde(default)]
    pub unlocked_seed_names: Vec<String>,
    pub settings: SettingsData,
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            high_score: 0,
            adventure_level: 0,
            unlocked_seed_names: Vec::new(),
            settings: SettingsData::default(),
        }
    }
}

/// Save directory: platform data dir, temp fallback (mirrors rustbox
/// `levels_dir`). On wasm `ProjectDirs` is `None` and `FsStorage` routes
/// to OPFS; the dir value is only a namespace there. On Android the stored
/// runtime path wins, then the Godot-`user://`-equivalent hardcoded path.
pub fn save_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        return game_utils::android_data_dir("org.rozvp.app");
    }
    #[cfg(not(target_os = "android"))]
    {
        if let Some(proj) = directories::ProjectDirs::from("com", "mlm-games", "rozvp") {
            let dir = proj.data_dir().to_path_buf();
            if std::fs::create_dir_all(&dir).is_ok() {
                return dir;
            }
        }
        let dir = std::env::temp_dir().join("com-mlm-games-rozvp");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }
}

fn store_in(dir: &Path) -> SaveStore<FsStorage> {
    SaveStore::new_with_storage(dir.to_path_buf(), SAVE_FILE_NAME, FsStorage)
        .with_validator(SaveStore::<FsStorage>::is_intact_ron)
}

/// Canonical save location (for tests/debugging; IO goes via the store).
pub fn save_path() -> Option<PathBuf> {
    Some(save_dir().join(SAVE_FILE_NAME))
}

pub fn load() -> SaveData {
    load_from_store(&save_dir())
}

pub fn save(data: &SaveData) -> anyhow::Result<()> {
    save_to_store(&save_dir(), data)
}

pub fn load_from(path: &Path) -> anyhow::Result<SaveData> {
    let Some(parent) = path.parent() else {
        anyhow::bail!("save path has no parent dir");
    };
    Ok(load_from_store(parent))
}

pub fn save_to(path: &Path, data: &SaveData) -> anyhow::Result<()> {
    let Some(parent) = path.parent() else {
        anyhow::bail!("save path has no parent dir");
    };
    save_to_store(parent, data)
}

fn load_from_store(dir: &Path) -> SaveData {
    let store = store_in(dir);
    let res = store.load(&SaveStore::<FsStorage>::is_intact_ron, &[]);
    let bytes = match res.status {
        LoadStatus::Ok | LoadStatus::Corrupt => res.data,
        LoadStatus::Missing | LoadStatus::Unreadable => None,
    };
    bytes
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|s| ron::de::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_to_store(dir: &Path, data: &SaveData) -> anyhow::Result<()> {
    let mut owned = data.clone();
    owned.version = SAVE_VERSION;
    let text = ron::ser::to_string_pretty(&owned, Default::default())
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    store_in(dir)
        .write(text.as_bytes())
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Push save contents into the UI share (boot + settings load).
pub fn apply_to_ui(data: &SaveData, ui: &mut PilotUi) {
    ui.adventure_level = data.adventure_level;
    ui.unlocked_seed_names = data.unlocked_seed_names.clone();
    ui.master_vol = data.settings.master_volume;
    ui.sfx_vol = data.settings.sfx_volume;
    ui.music_vol = data.settings.music_volume;
    ui.language = data.settings.language.clone();
    ui.level_name = super::levels::level_label(ui.adventure_level);
}

/// Snapshot UI progress back into save shape (persist points).
pub fn capture_from_ui(ui: &PilotUi) -> SaveData {
    SaveData {
        version: SAVE_VERSION,
        high_score: 0,
        adventure_level: ui.adventure_level,
        unlocked_seed_names: ui.unlocked_seed_names.clone(),
        settings: SettingsData {
            master_volume: ui.master_vol,
            sfx_volume: ui.sfx_vol,
            music_volume: ui.music_vol,
            language: ui.language.clone(),
        },
    }
}

/// Persist current UI progress. Failures are silent on purpose: a missing
/// save backend must never break the game loop.
pub fn persist_ui(ui: &PilotUi) {
    let _ = save(&capture_from_ui(ui));
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_utils::storage::MemoryStorage;

    fn mem_store() -> SaveStore<MemoryStorage> {
        SaveStore::new_with_storage(
            PathBuf::from("/rozvp-save-test"),
            SAVE_FILE_NAME,
            MemoryStorage::new(),
        )
        .with_validator(SaveStore::<MemoryStorage>::is_intact_ron)
    }

    fn sample() -> SaveData {
        SaveData {
            version: SAVE_VERSION,
            high_score: 0,
            adventure_level: 4,
            unlocked_seed_names: vec!["Wall-nut".to_string(), "Cherry Bomb".to_string()],
            settings: SettingsData {
                master_volume: 0.9,
                sfx_volume: 0.7,
                music_volume: 0.5,
                language: "fr".to_string(),
            },
        }
    }

    #[test]
    fn roundtrip_preserves_progress() {
        let store = mem_store();
        let text = ron::ser::to_string_pretty(&sample(), Default::default()).unwrap();
        store.write(text.as_bytes()).expect("write works");
        let res = store.load(&SaveStore::<MemoryStorage>::is_intact_ron, &[]);
        assert!(matches!(res.status, LoadStatus::Ok));
        let back: SaveData =
            ron::de::from_str(&String::from_utf8(res.data.unwrap()).unwrap()).unwrap();
        assert_eq!(back.adventure_level, 4);
        assert_eq!(back.unlocked_seed_names.len(), 2);
        assert_eq!(back.settings.language, "fr");
    }

    #[test]
    fn corrupt_file_falls_back_to_default() {
        let store = mem_store();
        store.write(b"{ not valid ron").expect("write works");
        let res = store.load(&SaveStore::<MemoryStorage>::is_intact_ron, &[]);
        assert!(matches!(res.status, LoadStatus::Corrupt));
        // Validator rejects it, so the loader path yields default.
        let back: SaveData = res
            .data
            .and_then(|b| String::from_utf8(b).ok())
            .and_then(|s| ron::de::from_str::<SaveData>(&s).ok())
            .unwrap_or_default();
        assert_eq!(back.adventure_level, 0);
    }

    #[test]
    fn fs_roundtrip_in_temp_dir() {
        let dir = std::env::temp_dir().join("rozvp-pilot-save-test");
        let _ = std::fs::remove_dir_all(&dir);
        save_to_store(&dir, &sample()).expect("save works");
        let back = load_from_store(&dir);
        assert_eq!(back.adventure_level, 4);
        assert_eq!(back.version, SAVE_VERSION);
        assert_eq!(back.settings.language, "fr");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_loads_default() {
        let back = load_from_store(Path::new("/nonexistent-rozvp-save-test"));
        assert_eq!(back.adventure_level, 0);
        assert_eq!(back.settings.music_volume, 0.8);
    }

    #[test]
    fn apply_and_capture_are_inverse() {
        let mut ui = PilotUi::default();
        let data = SaveData {
            adventure_level: 7,
            unlocked_seed_names: vec!["Snow Pea".to_string()],
            settings: SettingsData {
                language: "de".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        apply_to_ui(&data, &mut ui);
        assert_eq!(ui.level_name, "1-8");
        let back = capture_from_ui(&ui);
        assert_eq!(back.adventure_level, 7);
        assert_eq!(back.settings.language, "de");
    }
}
