//! Pilot save: RON `save.ron` byte-compatible with the bevy build.
//! Same struct shape (`SaveData`/`SettingsData`), same directory
//! (`ProjectDirs("com", "mlm-games", "rozvp")` data dir), same pretty RON
//! encoding as `game_utils::save`, so progress carries across shells.

use std::path::PathBuf;

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

/// Canonical save location. `None` where `ProjectDirs` is unavailable
/// (wasm): callers treat that as "no save yet".
pub fn save_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "mlm-games", "rozvp")
        .map(|proj| proj.data_dir().join(SAVE_FILE_NAME))
}

pub fn load() -> SaveData {
    save_path()
        .and_then(|p| load_from(&p).ok())
        .unwrap_or_default()
}

pub fn save(data: &SaveData) -> anyhow::Result<()> {
    let Some(path) = save_path() else {
        return Ok(());
    };
    save_to(&path, data)
}

pub fn load_from(path: &std::path::Path) -> anyhow::Result<SaveData> {
    let text = std::fs::read_to_string(path)?;
    Ok(ron::de::from_str(&text)?)
}

pub fn save_to(path: &std::path::Path, data: &SaveData) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut owned = data.clone();
    owned.version = SAVE_VERSION;
    let text = ron::ser::to_string_pretty(&owned, Default::default())?;
    std::fs::write(path, text)?;
    Ok(())
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

/// Persist current UI progress. Failures are silent on purpose (same as
/// the bevy build: a missing save dir must never break the game loop).
pub fn persist_ui(ui: &PilotUi) {
    let _ = save(&capture_from_ui(ui));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_progress() {
        let dir = std::env::temp_dir().join("rozvp-pilot-save-test");
        let path = dir.join(SAVE_FILE_NAME);
        let _ = std::fs::remove_dir_all(&dir);
        let data = SaveData {
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
        };
        save_to(&path, &data).expect("save works");
        let back = load_from(&path).expect("load works");
        assert_eq!(back.adventure_level, 4);
        assert_eq!(back.unlocked_seed_names.len(), 2);
        assert_eq!(back.settings.language, "fr");
        assert_eq!(back.version, SAVE_VERSION);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_loads_default() {
        let back = load_from(std::path::Path::new("/nonexistent/rozvp-save-test.ron"));
        assert!(back.is_err());
        let rate = SaveData::default();
        assert_eq!(rate.adventure_level, 0);
        assert_eq!(rate.settings.music_volume, 0.8);
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
