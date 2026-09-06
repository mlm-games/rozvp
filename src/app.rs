use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use repose_bevy::{ReposePlugin, ReposePluginSettings};
use repose_core::{prelude::Modifier, remember};
use repose_ui::overlay::OverlayHandle;

use crate::asset_tracking::AssetsLoading;
use crate::dev_tools::DevToolsPlugin;
use crate::game::GamePlugin;
use crate::menus::{self, UiAction, UiBridge};
use crate::save::SaveData;
use crate::screens::ScreensPlugin;
use crate::theme::ThemePlugin;
use game_utils_bevy::{
    EcosystemPlugin,
    audio::AudioChannels,
    i18n::{self, I18nPlugin, LocaleResources},
    post_process::{ScreenEffectSettings, sync_post_process_settings},
    save::{SaveManager, SavePlugin},
    screen_effects::{CameraBase, FlashWhite},
    time_scale::TimeScaleControl,
    transitions::Transition,
};

const TRANSLATION_KEYS: &[&str] = &[
    "app-title",
    "adventure",
    "mini-games",
    "puzzle",
    "survival",
    "zen-garden",
    "almanac",
    "store",
    "settings",
    "help",
    "quit",
    "paused",
    "resume",
    "restart",
    "main-menu",
    "quit-to-title",
    "save",
    "back",
    "master-volume",
    "sfx-volume",
    "music-volume",
    "language",
    "choose-your-seeds",
    "your-bank",
    "available-packets",
    "lets-rock",
    "level-complete",
    "game-over",
    "zombies-ate-your-brains",
    "not-enough-sun",
    "try-again",
    "continue",
    "loading",
];

const LOCALES: &[(&str, &str)] = &[
    ("en", include_str!("../assets/locales/en/main.ftl")),
    ("es", include_str!("../assets/locales/es/main.ftl")),
    ("fr", include_str!("../assets/locales/fr/main.ftl")),
    ("de", include_str!("../assets/locales/de/main.ftl")),
    ("ja", include_str!("../assets/locales/ja/main.ftl")),
    ("zh", include_str!("../assets/locales/zh/main.ftl")),
    ("pt", include_str!("../assets/locales/pt/main.ftl")),
];

#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
pub enum AppState {
    #[default]
    Splash,
    Loading,
    Title,
    InGame,
}

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct Paused(pub bool);

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum OverlayMenu {
    #[default]
    None,
    Settings,
    Credits,
    Pause,
    SeedChooser,
    Award,
    LevelComplete,
    GameOver,
    NotEnoughSun,
}

#[derive(Resource, Default)]
pub struct PendingUnpause(pub Option<Timer>);

/// Set by the UI when the player confirms a restart; consumed by the game's
/// `apply_restart` system (which runs even while paused, so it works from
/// GameOver/Pause overlays).
#[derive(Resource, Default)]
pub struct PendingRestart(pub bool);

#[derive(Clone, Debug, Default)]
pub struct SeedSlotUi {
    pub seed_name: String,
    pub cost: i32,
    /// 0.0..=1.0 recharge progress (1 = ready).
    pub ready: f32,
    pub affordable: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, Default)]
pub struct AdviceUi {
    pub text: String,
    pub visible: bool,
}

#[derive(Resource, Clone)]
pub struct SharedUi {
    pub phase: AppState,
    pub paused: bool,
    pub loading_progress: f32,
    pub overlay: OverlayMenu,
    pub master_vol: f32,
    pub sfx_vol: f32,
    pub music_vol: f32,
    #[allow(dead_code)] // kept so saves stay compatible
    pub high_score: u32,
    pub transition_alpha: f32,
    pub flash_alpha: f32,
    pub language: String,
    pub saved_language: String,
    pub available_languages: Vec<String>,
    pub translations: HashMap<String, String>,

    pub sun: i32,
    pub level_name: String,
    /// 0..=1 wave progress meter.
    pub progress: f32,
    pub flags_total: u32,
    pub flags_done: u32,
    pub seed_bank: Vec<SeedSlotUi>,
    pub shovel_selected: bool,
    pub advice: AdviceUi,

    pub chooser_picks: Vec<String>,
    pub adventure_level: u32,
    pub unlocked_seed_names: Vec<String>,
    pub pending_award_seed: Option<String>,
}

impl Default for SharedUi {
    fn default() -> Self {
        Self {
            phase: AppState::Splash,
            paused: false,
            loading_progress: 0.0,
            overlay: OverlayMenu::None,
            master_vol: 1.0,
            sfx_vol: 1.0,
            music_vol: 0.8,
            high_score: 0,
            transition_alpha: 0.0,
            flash_alpha: 0.0,
            language: "en".to_string(),
            saved_language: "en".to_string(),
            available_languages: vec!["en".to_string()],
            translations: HashMap::new(),
            sun: 50,
            level_name: crate::game::level_flow::level_label(0),
            progress: 0.0,
            flags_total: 1,
            flags_done: 0,
            seed_bank: vec![
                SeedSlotUi {
                    seed_name: "Sunflower".into(),
                    cost: 50,
                    ready: 1.0,
                    affordable: true,
                    selected: false,
                },
                SeedSlotUi {
                    seed_name: "Peashooter".into(),
                    cost: 100,
                    ready: 1.0,
                    affordable: true,
                    selected: false,
                },
            ],
            shovel_selected: false,
            advice: AdviceUi::default(),
            chooser_picks: vec!["Sunflower".into(), "Peashooter".into()],
            adventure_level: 0,
            unlocked_seed_names: crate::game::level_flow::starting_unlocked_seeds(),
            pending_award_seed: None,
        }
    }
}

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        let shared = Arc::new(Mutex::new(SharedUi::default()));
        let actions = Arc::new(Mutex::new(Vec::<UiAction>::new()));
        let shared_ui = shared.clone();
        let actions_ui = actions.clone();

        app.init_state::<AppState>()
            .insert_resource(Paused(false))
            .insert_resource(OverlayMenu::None)
            .insert_resource(PendingUnpause(None))
            .insert_resource(PendingRestart::default())
            .insert_resource(UiBridge {
                shared: shared.clone(),
                actions: actions.clone(),
            })
            .add_plugins(ReposePlugin::with_settings(
                ReposePluginSettings {
                    clear_alpha: 0.0,
                    compose_every_frame: true,
                    msaa_samples: 1,
                    overlay: true,
                    ..Default::default()
                },
                move |_s, _c| {
                    let st = shared_ui.lock().unwrap().clone();
                    let acts = actions_ui.clone();
                    let overlay_rc = remember(OverlayHandle::new);
                    let overlay = (*overlay_rc).clone();
                    let root = menus::compose_root(overlay.clone(), st, acts);
                    overlay.host(Modifier::new().fill_max_size(), root)
                },
            ))
            .add_plugins((
                ThemePlugin,
                EcosystemPlugin::<AppState>::new(I18nPlugin::new(TRANSLATION_KEYS, LOCALES)),
                SavePlugin::<SaveData>::new(SaveManager::new(
                    "com",
                    "mlm-games",
                    "rozvp",
                    "save.ron",
                    1,
                )),
                ScreensPlugin,
                GamePlugin,
                DevToolsPlugin,
            ))
            .add_systems(Startup, setup_camera)
            .add_systems(
                Update,
                (
                    apply_saved_settings,
                    sync_shared_ui,
                    sync_post_process_settings::<AppState>,
                    process_ui_actions,
                    handle_pause_input,
                    tick_pending_unpause,
                    sync_virtual_time_with_pause,
                )
                    .chain(),
            );
    }
}

fn apply_saved_settings(
    save: Res<SaveData>,
    bridge: Res<UiBridge>,
    mut locale: ResMut<LocaleResources>,
) {
    if !save.is_added() && !save.is_changed() {
        return;
    }
    if locale
        .available
        .iter()
        .any(|l| l == &save.settings.language)
    {
        locale.set_locale(&save.settings.language);
    }
    // Load adventure progress + unlocks (save is authoritative on load).
    if let Ok(mut ui) = bridge.shared.lock() {
        ui.adventure_level = save.adventure_level;
        if !save.unlocked_seed_names.is_empty() {
            ui.unlocked_seed_names = save.unlocked_seed_names.clone();
        } else {
            ui.unlocked_seed_names = crate::game::level_flow::starting_unlocked_seeds();
        }
        crate::game::level_flow::normalize_progress_ui(&mut ui);
    }
}

/// Copies runtime progress into the save struct.
fn persist_progress(save: &mut SaveData, ui: &SharedUi) {
    save.adventure_level = ui.adventure_level;
    save.unlocked_seed_names = ui.unlocked_seed_names.clone();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Transform::from_xyz(0.0, 0.0, 1000.0),
        CameraBase {
            translation: Vec3::new(0.0, 0.0, 1000.0),
            rotation: 0.0,
        },
        ScreenEffectSettings::default(),
    ));
}

fn sync_shared_ui(
    state: Res<State<AppState>>,
    paused: Res<Paused>,
    overlay: Res<OverlayMenu>,
    bridge: Res<UiBridge>,
    save: Res<SaveData>,
    transition: Res<Transition<AppState>>,
    flash: Res<FlashWhite>,
    locale: Res<LocaleResources>,
    mut channels: ResMut<AudioChannels>,
    loading: Option<Res<AssetsLoading>>,
    asset_server: Res<AssetServer>,
) {
    let Ok(mut ui) = bridge.shared.lock() else {
        return;
    };
    ui.phase = state.get().clone();
    ui.paused = paused.0;
    ui.overlay = *overlay;
    if *overlay != OverlayMenu::Settings {
        ui.master_vol = save.settings.master_volume;
        ui.sfx_vol = save.settings.sfx_volume;
        ui.music_vol = save.settings.music_volume;
    }
    ui.transition_alpha = transition.overlay_alpha;
    ui.flash_alpha = flash.amount;
    ui.language = locale.current.clone();
    ui.available_languages = locale.available.clone();
    ui.translations = i18n::get_current_translations(&locale);
    ui.loading_progress = match loading {
        Some(l) if !l.0.is_empty() => {
            l.0.iter()
                .filter(|h| asset_server.is_loaded_with_dependencies(h.id()))
                .count() as f32
                / l.0.len() as f32
        }
        _ => 1.0,
    };
    channels.master = save.settings.master_volume;
    channels.sfx = save.settings.sfx_volume;
    channels.music = save.settings.music_volume;
}

fn tick_pending_unpause(
    real: Res<Time<Real>>,
    mut pending: ResMut<PendingUnpause>,
    mut paused: ResMut<Paused>,
) {
    let Some(timer) = pending.0.as_mut() else {
        return;
    };
    if timer.tick(real.delta()).just_finished() {
        pending.0 = None;
        paused.0 = false;
    }
}

fn set_vol(bridge: &UiBridge, field: impl Fn(&mut SharedUi) -> &mut f32, v: f32) {
    if let Ok(mut ui) = bridge.shared.lock() {
        *field(&mut ui) = v.clamp(0.0, 1.0);
    }
}

/// All seeds the chooser knows about; visibility is gated by
/// `SharedUi.unlocked_seed_names` (see level_flow).
pub const CHOOSER_SEEDS: &[&str] = &[
    "Sunflower",
    "Peashooter",
    "Wall-nut",
    "Cherry Bomb",
    "Potato Mine",
    "Snow Pea",
    "Chomper",
    "Repeater",
    "Squash",
    "Threepeater",
    "Jalapeno",
    "Spikeweed",
    "Torchwood",
    "Tall-nut",
    "Garlic",
    "Hypno-shroom",
    "Ice-shroom",
    "Doom-shroom",
];

fn process_ui_actions(
    bridge: Res<UiBridge>,
    mut paused: ResMut<Paused>,
    mut overlay: ResMut<OverlayMenu>,
    mut save: ResMut<SaveData>,
    mut exit: MessageWriter<AppExit>,
    mut transition: ResMut<Transition<AppState>>,
    manager: Res<SaveManager>,
    mut pending_unpause: ResMut<PendingUnpause>,
    mut pending_restart: ResMut<PendingRestart>,
    mut locale: ResMut<LocaleResources>,
) {
    let Ok(mut q) = bridge.actions.lock() else {
        return;
    };
    for action in q.drain(..) {
        match action {
            UiAction::OpenAdventure => {
                *overlay = OverlayMenu::SeedChooser;
            }
            UiAction::ChooserPick(name) => {
                if let Ok(mut ui) = bridge.shared.lock() {
                    if !ui.chooser_picks.contains(&name) && ui.chooser_picks.len() < 10 {
                        ui.chooser_picks.push(name);
                    }
                }
            }
            UiAction::ChooserRemove(i) => {
                if let Ok(mut ui) = bridge.shared.lock() {
                    if i < ui.chooser_picks.len() {
                        ui.chooser_picks.remove(i);
                    }
                }
            }
            UiAction::ConfirmSeedChooser => {
                if let Ok(mut ui) = bridge.shared.lock() {
                    crate::game::level_flow::normalize_progress_ui(&mut ui);
                    // Chooser only offers unlocked seeds; prune any stale picks.
                    let unlocked = ui.unlocked_seed_names.clone();
                    ui.chooser_picks.retain(|p| unlocked.iter().any(|u| u == p));
                    persist_progress(&mut save, &ui);
                }
                let _ = manager.save(&*save);
                if let Ok(mut ui) = bridge.shared.lock() {
                    if ui.chooser_picks.is_empty() {
                        continue;
                    }
                    ui.seed_bank = ui
                        .chooser_picks
                        .iter()
                        .map(|name| {
                            let cost = crate::game::defs::seed_def(name)
                                .map(|d| d.cost)
                                .unwrap_or(100);
                            crate::app::SeedSlotUi {
                                seed_name: name.clone(),
                                cost,
                                ready: 1.0,
                                affordable: true,
                                selected: false,
                            }
                        })
                        .collect();
                    ui.sun = 50;
                    ui.progress = 0.0;
                    ui.flags_done = 0;
                }
                *overlay = OverlayMenu::None;
                transition.begin_to_state(AppState::InGame);
            }
            UiAction::DialogOk => {
                match *overlay {
                    OverlayMenu::LevelComplete => {
                        // Decide the award first, then route.
                        let award = bridge
                            .shared
                            .lock()
                            .ok()
                            .map(|ui| {
                                crate::game::level_flow::award_for_completed_level(
                                    ui.adventure_level,
                                )
                            })
                            .flatten();

                        if let Some(seed) = award {
                            if let Ok(mut ui) = bridge.shared.lock() {
                                ui.pending_award_seed = Some(seed.to_string());
                            }
                            *overlay = OverlayMenu::Award;
                        } else {
                            if let Ok(mut ui) = bridge.shared.lock() {
                                crate::game::level_flow::advance_after_level_complete(&mut ui);
                                persist_progress(&mut save, &ui);
                            }
                            let _ = manager.save(&*save);
                            *overlay = OverlayMenu::None;
                            transition.begin_to_state(AppState::Title);
                        }
                    }
                    OverlayMenu::Award => {
                        if let Ok(mut ui) = bridge.shared.lock() {
                            if let Some(seed) = ui.pending_award_seed.take() {
                                crate::game::level_flow::ensure_seed_unlocked(&mut ui, &seed);
                            }
                            crate::game::level_flow::advance_after_level_complete(&mut ui);
                            crate::game::level_flow::normalize_progress_ui(&mut ui);
                            persist_progress(&mut save, &ui);
                        }
                        let _ = manager.save(&*save);
                        *overlay = OverlayMenu::None;
                        transition.begin_to_state(AppState::Title);
                    }
                    _ => {
                        *overlay = OverlayMenu::None;
                    }
                }
            }
            UiAction::SelectSeedSlot(i) => {
                if let Ok(mut ui) = bridge.shared.lock() {
                    if let Some(slot) = ui.seed_bank.get(i) {
                        if slot.ready >= 1.0 && slot.affordable {
                            for (idx, s) in ui.seed_bank.iter_mut().enumerate() {
                                s.selected = idx == i;
                            }
                            ui.shovel_selected = false;
                        } else if !slot.affordable {
                            for s in ui.seed_bank.iter_mut() {
                                s.selected = false;
                            }
                            *overlay = OverlayMenu::NotEnoughSun;
                        }
                    }
                }
            }
            UiAction::ToggleShovel => {
                if let Ok(mut ui) = bridge.shared.lock() {
                    ui.shovel_selected = !ui.shovel_selected;
                    for s in ui.seed_bank.iter_mut() {
                        s.selected = false;
                    }
                }
            }
            UiAction::ClearCursor => {
                if let Ok(mut ui) = bridge.shared.lock() {
                    ui.shovel_selected = false;
                    for s in ui.seed_bank.iter_mut() {
                        s.selected = false;
                    }
                }
            }
            UiAction::OpenMiniGames | UiAction::OpenPuzzle | UiAction::OpenSurvival => {
                // Challenge modes arrive in a later phase.
            }
            UiAction::OpenZenGarden | UiAction::OpenAlmanac | UiAction::OpenStore => {
                // Meta screens arrive in a later phase.
            }
            UiAction::OpenPause => {
                paused.0 = true;
                *overlay = OverlayMenu::Pause;
                pending_unpause.0 = None;
            }
            UiAction::RestartLevel => {
                if let Ok(mut ui) = bridge.shared.lock() {
                    ui.advice.visible = false;
                }
                *overlay = OverlayMenu::None;
                pending_restart.0 = true;
                // Keep paused until apply_restart swaps the level in, so the
                // sim doesn't step mid-fade; it unpauses when done.
            }

            UiAction::OpenSettings => {
                if let Ok(mut ui) = bridge.shared.lock() {
                    ui.saved_language = locale.current.clone();
                }
                *overlay = OverlayMenu::Settings;
            }
            UiAction::OpenCredits => *overlay = OverlayMenu::Credits,
            UiAction::CloseOverlay => {
                if *overlay == OverlayMenu::Settings
                    && let Ok(ui) = bridge.shared.lock()
                {
                    locale.set_locale(&ui.saved_language);
                }
                match *overlay {
                    OverlayMenu::NotEnoughSun => *overlay = OverlayMenu::None,
                    OverlayMenu::SeedChooser if !paused.0 => *overlay = OverlayMenu::None,
                    OverlayMenu::Settings | OverlayMenu::Credits if paused.0 => {
                        *overlay = OverlayMenu::Pause;
                    }
                    OverlayMenu::Pause if paused.0 => {
                        *overlay = OverlayMenu::None;
                        pending_unpause.0 = Some(Timer::from_seconds(0.2, TimerMode::Once));
                    }
                    _ => {
                        *overlay = OverlayMenu::None;
                    }
                }
            }
            UiAction::Resume => {
                *overlay = OverlayMenu::None;
                pending_unpause.0 = Some(Timer::from_seconds(0.2, TimerMode::Once));
            }
            UiAction::QuitToTitle => {
                paused.0 = false;
                *overlay = OverlayMenu::None;
                pending_unpause.0 = None;
                transition.begin_to_state(AppState::Title);
            }
            UiAction::QuitApp => {
                exit.write(AppExit::Success);
            }
            UiAction::SetMasterVol(v) => set_vol(&bridge, |ui| &mut ui.master_vol, v),
            UiAction::SetSfxVol(v) => set_vol(&bridge, |ui| &mut ui.sfx_vol, v),
            UiAction::SetMusicVol(v) => set_vol(&bridge, |ui| &mut ui.music_vol, v),
            UiAction::SaveSettings => {
                if let Ok(ui) = bridge.shared.lock() {
                    save.settings.master_volume = ui.master_vol;
                    save.settings.sfx_volume = ui.sfx_vol;
                    save.settings.music_volume = ui.music_vol;
                    save.settings.language = locale.current.clone();
                }
                let _ = manager.save(&*save);
                if let Ok(mut ui) = bridge.shared.lock() {
                    ui.saved_language = locale.current.clone();
                }
                if paused.0 {
                    *overlay = OverlayMenu::Pause;
                } else {
                    *overlay = OverlayMenu::None;
                }
            }
            UiAction::NextLanguage => {
                let available = locale.available.clone();
                let current = locale.current.clone();
                let idx = available.iter().position(|l| *l == current).unwrap_or(0);
                let next = (idx + 1) % available.len();
                if let Some(next_locale) = available.get(next) {
                    locale.set_locale(next_locale);
                }
            }
            UiAction::SetLanguage(ref lang) => {
                if locale.available.contains(lang) {
                    locale.set_locale(lang);
                }
            }
        }
    }
}

fn handle_pause_input(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<AppState>>,
    mut paused: ResMut<Paused>,
    mut overlay: ResMut<OverlayMenu>,
    mut pending_unpause: ResMut<PendingUnpause>,
    transition: Res<Transition<AppState>>,
) {
    if *state.get() != AppState::InGame {
        return;
    }
    if transition.block_input {
        return;
    }
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    match *overlay {
        OverlayMenu::None if !paused.0 => {
            paused.0 = true;
            *overlay = OverlayMenu::Pause;
            pending_unpause.0 = None;
        }
        OverlayMenu::Pause => {
            *overlay = OverlayMenu::None;
            pending_unpause.0 = Some(Timer::from_seconds(0.2, TimerMode::Once));
        }
        OverlayMenu::Settings | OverlayMenu::Credits => {
            if paused.0 {
                *overlay = OverlayMenu::Pause;
            } else {
                *overlay = OverlayMenu::None;
            }
        }
        _ => {}
    }
}

fn sync_virtual_time_with_pause(
    paused: Res<Paused>,
    mut ctrl: ResMut<TimeScaleControl>,
    #[cfg(feature = "physics")] mut rapier_config: Query<
        &mut bevy_rapier2d::plugin::RapierConfiguration,
    >,
) {
    ctrl.paused = paused.0;
    #[cfg(feature = "physics")]
    for mut config in &mut rapier_config {
        config.physics_pipeline_active = !paused.0;
    }
}
