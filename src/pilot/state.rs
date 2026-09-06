//! Pilot sim state: tick clock, board, wave director, UI share.
//! Mirrors `game::tick`, `game::board`, `game::zombie::WaveState`,
//! `game::advice`, and the gameplay-relevant slice of `app::SharedUi`
//! without bevy types.

use std::collections::HashMap;
use std::sync::Mutex;

use repame_sim::bevy_ecs::component::{
    ComponentId, Mutable, RequiredComponentsRegistrator, StorageType,
};
use repame_sim::bevy_ecs::prelude::*;
use repame_sim::bevy_ecs::resource::IsResource;

use super::comps::PlantKind;
use crate::pilot::constants::{
    FIRST_WAVE_DELAY_TICKS, LAWN_COLS, LAWN_ROWS, SKY_SUN_INTERVAL_TICKS, STARTING_SUN,
};

macro_rules! resource {
    ($($t:ty),*) => {
        $(
            // Mirrors `#[derive(Resource)]`: SparseSet storage plus the
            // `IsResource` required-component registration. Without the
            // marker, resource insertion silently fails to track (0.19.1).
            impl Component for $t {
                const STORAGE_TYPE: StorageType = StorageType::SparseSet;
                type Mutability = Mutable;
                fn register_required_components(
                    _id: ComponentId,
                    required: &mut RequiredComponentsRegistrator,
                ) {
                    let rid = if let Some(id) = required
                        .components_registrator()
                        .component_id::<$t>()
                    {
                        id
                    } else {
                        required.components_registrator().register_component::<$t>()
                    };
                    required.register_required::<IsResource>(move || IsResource::new(rid));
                }
            }
            impl Resource for $t {}
        )*
    };
}

/// Absolute sim time in 100 Hz ticks since level start.
#[derive(Debug, Default)]
pub struct GameTime {
    pub ticks: i64,
}

/// Whole ticks to advance this schedule run (1 inside the pilot driver,
///
/// 0 when paused - systems early-out on `<= 0` exactly like the bevy build).
#[derive(Debug, Default)]
pub struct FrameTicks(pub i32);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Stage {
    #[default]
    Day,
    Night,
}

#[derive(Debug)]
pub struct Board {
    pub cells: [[Option<Entity>; LAWN_COLS]; LAWN_ROWS],
    pub sun: i32,
    pub sun_collected_total: u32,
    pub sky_sun_timer: i32,
    pub stage: Stage,
}

impl Default for Board {
    fn default() -> Self {
        Self {
            cells: [[None; LAWN_COLS]; LAWN_ROWS],
            sun: STARTING_SUN,
            sun_collected_total: 0,
            sky_sun_timer: SKY_SUN_INTERVAL_TICKS,
            stage: Stage::Day,
        }
    }
}

impl Board {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn can_plant(&self, col: usize, row: usize) -> bool {
        row < LAWN_ROWS && col < LAWN_COLS && self.cells[row][col].is_none()
    }
}

/// Logic-space helpers. Same formulas as `game::board` (top-left origin,
/// y-down, pixels); duplicated here to keep the pilot bevy-type-free.
pub fn logic_to_grid(logic_x: f32, logic_y: f32) -> Option<(usize, usize)> {
    use crate::pilot::constants::{GRID_CELL_H, GRID_CELL_W, LAWN_XMIN, LAWN_YMIN};
    if logic_x < LAWN_XMIN || logic_y < LAWN_YMIN {
        return None;
    }
    let col = ((logic_x - LAWN_XMIN) / GRID_CELL_W).floor() as usize;
    let row = ((logic_y - LAWN_YMIN) / GRID_CELL_H).floor() as usize;
    if col < LAWN_COLS && row < LAWN_ROWS {
        Some((col, row))
    } else {
        None
    }
}

pub fn grid_center_logic(col: usize, row: usize) -> (f32, f32) {
    use crate::pilot::constants::{GRID_CELL_H, GRID_CELL_W, LAWN_XMIN, LAWN_YMIN};
    (
        LAWN_XMIN + col as f32 * GRID_CELL_W + GRID_CELL_W * 0.5,
        LAWN_YMIN + row as f32 * GRID_CELL_H + GRID_CELL_H * 0.5,
    )
}

pub fn row_center_y(row: usize) -> f32 {
    use crate::pilot::constants::{GRID_CELL_H, LAWN_YMIN};
    LAWN_YMIN + row as f32 * GRID_CELL_H + GRID_CELL_H * 0.5
}

#[derive(Debug, Default)]
pub struct SeedBankRuntime {
    pub recharge_remaining: Vec<i32>,
}

#[derive(Debug, Default)]
pub struct ClickConsumedThisFrame(pub bool);

#[derive(Debug, Default)]
pub struct SunStats {
    pub collected_total: u32,
}

#[derive(Debug)]
pub struct WaveState {
    pub wave_plans: Vec<Vec<super::comps::ZombieKind>>,
    pub huge_flags: Vec<bool>,
    pub current_wave: u32,
    pub num_waves: u32,
    pub started: bool,
    pub between_waves: bool,
    pub cleared: bool,
    pub huge_wave: bool,
    pub start_delay_remaining: i32,
    pub inter_wave_remaining: i32,
    pub spawn_timer_remaining: i32,
    pub planned: Vec<super::comps::ZombieKind>,
    pub queued: u32,
    pub spawned: u32,
    pub total_in_level: u32,
    pub total_spawned_all: u32,
    pub budget_spent: u32,
}

impl Default for WaveState {
    fn default() -> Self {
        Self {
            wave_plans: Vec::new(),
            huge_flags: Vec::new(),
            current_wave: 0,
            num_waves: 1,
            started: false,
            between_waves: false,
            cleared: false,
            huge_wave: false,
            start_delay_remaining: FIRST_WAVE_DELAY_TICKS,
            inter_wave_remaining: 0,
            spawn_timer_remaining: 0,
            planned: Vec::new(),
            queued: 0,
            spawned: 0,
            total_in_level: 0,
            total_spawned_all: 0,
            budget_spent: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AdviceStage {
    NightIntro,
    #[default]
    ClickSun,
    PlantSunflower,
    ZombiesComing,
    HugeWave,
    Done,
}

#[derive(Debug, Default)]
pub struct AdviceState {
    pub stage: AdviceStage,
    pub hold_remaining: i32,
    pub huge_announced: bool,
}

/// Pointer clicks in logic space, queued by views, consumed by systems.
/// Replaces bevy mouse/window/camera queries.
#[derive(Debug, Default)]
pub struct ClickQueue {
    pub clicks: Vec<(f32, f32)>,
}

/// Overlay + pause + restart. Collapses `OverlayMenu`, `Paused`,
/// `PendingRestart`, and `Transition` blocking into pilot-owned state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Overlay {
    #[default]
    None,
    Pause,
    SeedChooser,
    Settings,
    Credits,
    Award,
    LevelComplete,
    GameOver,
    NotEnoughSun,
}

#[derive(Debug, Default)]
pub struct FlowControl {
    pub paused: bool,
    pub overlay: Overlay,
    pub pending_restart: bool,
}

impl FlowControl {
    pub fn sim_blocked(&self) -> bool {
        self.paused || self.overlay != Overlay::None
    }
}

#[derive(Clone, Debug)]
pub struct SeedSlot {
    pub seed_name: String,
    pub cost: i32,
    pub ready: f32,
    pub affordable: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, Default)]
pub struct AdviceUi {
    pub text: String,
    pub visible: bool,
}

/// Gameplay slice of `app::SharedUi`, owned by the pilot. Lives behind a
/// mutex as a world resource (mirrors `UiBridge`); views lock it per frame.
#[derive(Debug)]
pub struct PilotUi {
    pub phase: PilotPhase,
    pub sun: i32,
    pub level_name: String,
    pub progress: f32,
    pub flags_total: u32,
    pub flags_done: u32,
    pub seed_bank: Vec<SeedSlot>,
    pub shovel_selected: bool,
    pub advice: AdviceUi,
    pub adventure_level: u32,
    pub unlocked_seed_names: Vec<String>,
    pub pending_award_seed: Option<String>,
    pub chooser_picks: Vec<String>,
    pub master_vol: f32,
    pub sfx_vol: f32,
    pub music_vol: f32,
    pub language: String,
}

impl Default for PilotUi {
    fn default() -> Self {
        Self {
            phase: PilotPhase::default(),
            sun: 0,
            level_name: String::new(),
            progress: 0.0,
            flags_total: 0,
            flags_done: 0,
            seed_bank: Vec::new(),
            shovel_selected: false,
            advice: AdviceUi::default(),
            adventure_level: 0,
            unlocked_seed_names: Vec::new(),
            pending_award_seed: None,
            chooser_picks: Vec::new(),
            master_vol: 1.0,
            sfx_vol: 1.0,
            music_vol: 0.8,
            language: "en".to_string(),
        }
    }
}

#[derive(Debug, Default)]
pub struct UiShare {
    pub ui: Mutex<PilotUi>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PilotPhase {
    #[default]
    Splash,
    Loading,
    Title,
    InGame,
}

/// Per-zombie live-rig hosts, keyed by sim entity. NOT a world resource
/// (`PlayerHostRef` is `Rc`, hence `!Send`): owned by the pilot driver
/// next to the `Sim`, synced after each tick batch (presentational only).
#[derive(Default)]
pub struct RigMap {
    pub hosts: HashMap<Entity, RigEntry>,
}

/// One live rig: host handle plus edge-trigger state for the `die`
/// machine input (fired once when the corpse appears).
#[derive(Clone)]
pub struct RigEntry {
    pub host: repame_actors::PlayerHostRef,
    pub die_fired: bool,
}

/// Names of kinds with special plant visuals (mirrors `visuals.rs` needs).
pub fn plant_base_color(kind: PlantKind) -> [f32; 4] {
    super::comps::seed_def_by_kind(kind)
        .map(|d| d.color)
        .unwrap_or([1.0, 1.0, 1.0, 1.0])
}

resource!(
    GameTime,
    FrameTicks,
    Board,
    SeedBankRuntime,
    ClickConsumedThisFrame,
    SunStats,
    WaveState,
    AdviceState,
    ClickQueue,
    FlowControl,
    UiShare
);
