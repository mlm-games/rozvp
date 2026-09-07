//! Wave director, level flow, and advice. Ports `zombie.rs` director half,
//! `level_flow.rs` (pure tables copied verbatim), and `advice.rs`.

use rand::RngExt;
use repame_sim::bevy_ecs::prelude::*;

use super::comps::{GameplayCleanup, PlantKind, Pos, Zombie, ZombieKind};
use super::state::{
    AdviceStage, AdviceState, Board, FlowControl, FrameTicks, Overlay, SeedBankRuntime, SeedSlot,
    Stage, SunStats, UiShare, WaveState,
};
use crate::pilot::constants::*;

fn zombie_point_cost(kind: ZombieKind) -> u32 {
    match kind {
        ZombieKind::Normal => POINT_NORMAL,
        ZombieKind::Flag => POINT_FLAG,
        ZombieKind::Conehead => POINT_CONE,
        ZombieKind::Buckethead => POINT_BUCKET,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WaveRecipe {
    pub budget: u32,
    pub max_cone: u32,
    pub max_bucket: u32,
    pub final_flag: bool,
    pub huge: bool,
}

fn build_wave_plan(recipe: WaveRecipe) -> (Vec<ZombieKind>, u32) {
    let mut rng = rand::rng();
    let mut remaining = recipe.budget;
    let mut plan = Vec::new();
    let mut cone_used = 0u32;
    let mut bucket_used = 0u32;
    let mut spent = 0u32;
    if recipe.final_flag {
        plan.push(ZombieKind::Flag);
        spent += zombie_point_cost(ZombieKind::Flag);
    }
    while remaining >= POINT_NORMAL {
        let can_bucket = bucket_used < recipe.max_bucket && remaining >= POINT_BUCKET;
        let can_cone = cone_used < recipe.max_cone && remaining >= POINT_CONE;
        let roll = rng.random_range(0..100);
        let kind = if can_bucket && roll < 12 {
            bucket_used += 1;
            ZombieKind::Buckethead
        } else if can_cone && roll < 40 {
            cone_used += 1;
            ZombieKind::Conehead
        } else {
            ZombieKind::Normal
        };
        let cost = zombie_point_cost(kind);
        if cost > remaining {
            plan.push(ZombieKind::Normal);
            remaining -= POINT_NORMAL;
            spent += POINT_NORMAL;
        } else {
            plan.push(kind);
            remaining -= cost;
            spent += cost;
        }
    }
    (plan, spent)
}

pub fn build_level_waves(adventure_level: u32) -> (Vec<Vec<ZombieKind>>, u32) {
    let recipes = build_level_recipes(adventure_level);
    let mut all = Vec::with_capacity(recipes.len());
    let mut total_spent = 0u32;
    for r in recipes {
        let (plan, spent) = build_wave_plan(r);
        total_spent += spent;
        all.push(plan);
    }
    (all, total_spent)
}

pub fn start_wave_if_needed(frame_ticks: Res<FrameTicks>, mut wave: ResMut<WaveState>) {
    if frame_ticks.0 <= 0 || wave.cleared || wave.wave_plans.is_empty() {
        return;
    }
    if wave.started {
        return;
    }
    if wave.between_waves {
        wave.inter_wave_remaining -= frame_ticks.0;
        if wave.inter_wave_remaining > 0 {
            return;
        }
        let next = wave.current_wave + 1;
        if next < wave.num_waves {
            begin_wave_index(&mut wave, next);
        } else {
            wave.between_waves = false;
        }
        return;
    }
    wave.start_delay_remaining -= frame_ticks.0;
    if wave.start_delay_remaining > 0 {
        return;
    }
    begin_wave_index(&mut wave, 0);
}

fn begin_wave_index(wave: &mut WaveState, idx: u32) {
    wave.current_wave = idx;
    wave.started = true;
    wave.between_waves = false;
    wave.planned = wave
        .wave_plans
        .get(idx as usize)
        .cloned()
        .unwrap_or_default();
    wave.queued = wave.planned.len() as u32;
    wave.spawned = 0;
    wave.spawn_timer_remaining = 0;
    wave.huge_wave = wave.huge_flags.get(idx as usize).copied().unwrap_or(false);
}

pub fn spawn_wave_zombies(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut wave: ResMut<WaveState>,
) {
    if frame_ticks.0 <= 0 || !wave.started || wave.queued == 0 {
        return;
    }
    wave.spawn_timer_remaining -= frame_ticks.0;
    if wave.spawn_timer_remaining > 0 {
        return;
    }
    wave.spawn_timer_remaining += SPAWN_STAGGER_TICKS;
    let spawn_idx = wave.spawned as usize;
    let Some(kind) = wave.planned.get(spawn_idx).copied() else {
        wave.queued = 0;
        return;
    };
    let mut rng = rand::rng();
    let row = rng.random_range(0..LAWN_ROWS);
    commands.spawn((
        GameplayCleanup,
        Zombie::new(kind, row),
        Pos {
            x: 760.0,
            y: super::state::row_center_y(row),
        },
    ));
    wave.spawned += 1;
    wave.total_spawned_all += 1;
    wave.queued = wave.queued.saturating_sub(1);
}

pub fn advance_or_complete_level(
    mut wave: ResMut<WaveState>,
    zombies: Query<(), With<Zombie>>,
    mut flow: ResMut<FlowControl>,
    ui_share: Res<UiShare>,
) {
    if wave.cleared || !wave.started || flow.overlay != Overlay::None {
        return;
    }
    if wave.queued > 0 || !zombies.is_empty() {
        return;
    }
    let next = wave.current_wave + 1;
    if next >= wave.num_waves {
        wave.cleared = true;
        if let Ok(mut ui) = ui_share.ui.lock() {
            ui.flags_done = wave.num_waves.max(1);
        }
        flow.overlay = Overlay::LevelComplete;
        flow.paused = true;
        return;
    }
    wave.started = false;
    wave.between_waves = true;
    wave.inter_wave_remaining = INTER_WAVE_DELAY_TICKS;
    wave.planned.clear();
    if let Ok(mut ui) = ui_share.ui.lock() {
        ui.flags_done = next;
        ui.flags_total = wave.num_waves.max(1);
    }
}

pub fn level_label(adventure_level: u32) -> String {
    let area = adventure_level / 10 + 1;
    let stage = adventure_level % 10 + 1;
    format!("{area}-{stage}")
}

pub fn stage_for_level(adventure_level: u32) -> Stage {
    match adventure_level / LEVELS_PER_AREA {
        0 => Stage::Day,
        _ => Stage::Night,
    }
}

pub fn starting_unlocked_seeds() -> Vec<String> {
    vec!["Sunflower".to_string(), "Peashooter".to_string()]
}

pub fn award_for_completed_level(completed_level: u32) -> Option<&'static str> {
    match completed_level {
        0 => Some("Wall-nut"),
        1 => Some("Cherry Bomb"),
        2 => Some("Potato Mine"),
        3 => Some("Snow Pea"),
        4 => Some("Chomper"),
        5 => Some("Repeater"),
        6 => Some("Squash"),
        7 => Some("Threepeater"),
        8 => Some("Tall-nut"),
        9 => Some("Garlic"),
        10 => Some("Spikeweed"),
        11 => Some("Torchwood"),
        12 => Some("Hypno-shroom"),
        13 => Some("Ice-shroom"),
        14 => Some("Doom-shroom"),
        15 => Some("Jalapeno"),
        _ => None,
    }
}

pub fn level_wave_count(adventure_level: u32) -> u32 {
    match adventure_level % 10 {
        0 => 4,
        1 => 6,
        2 => 8,
        _ => 10,
    }
}

pub fn wave_point_budget(adventure_level: u32, wave_index: u32) -> u32 {
    let n = level_wave_count(adventure_level);
    let last = n - 1;
    if wave_index == last && n >= 6 {
        if adventure_level == 0 { 4 } else { 10 }
    } else if adventure_level == 0 {
        [1, 1, 2, 3][wave_index as usize % 4]
    } else {
        let base = [1, 1, 1, 2, 2, 2, 3, 3, 3, 4][wave_index as usize % 10];
        base * (1 + adventure_level.saturating_sub(2) / 3)
    }
}

pub fn wave_is_huge(adventure_level: u32, wave_index: u32) -> bool {
    let n = level_wave_count(adventure_level);
    n >= 6 && wave_index + 1 == n
}

pub fn build_level_recipes(adventure_level: u32) -> Vec<WaveRecipe> {
    let n = level_wave_count(adventure_level);
    (0..n)
        .map(|w| {
            let huge = wave_is_huge(adventure_level, w);
            WaveRecipe {
                budget: wave_point_budget(adventure_level, w),
                max_cone: if adventure_level >= 1 {
                    if huge { 4 } else { 2 }
                } else {
                    0
                },
                max_bucket: if adventure_level >= 3 {
                    if huge { 2 } else { 1 }
                } else {
                    0
                },
                final_flag: huge,
                huge,
            }
        })
        .collect()
}

pub fn load_level(world: &mut World, adventure_level: u32) {
    let stage = stage_for_level(adventure_level);
    world.resource_mut::<Board>().stage = stage;
    {
        let mut wave = world.resource_mut::<WaveState>();
        let (plans, spent) = build_level_waves(adventure_level);
        let recipes = build_level_recipes(adventure_level);
        wave.wave_plans = plans;
        wave.huge_flags = recipes.iter().map(|r| r.huge).collect();
        wave.num_waves = wave.wave_plans.len() as u32;
        wave.total_in_level = wave.wave_plans.iter().map(|p| p.len() as u32).sum();
        wave.budget_spent = spent;
        wave.current_wave = 0;
        wave.planned.clear();
        wave.queued = 0;
        wave.spawned = 0;
        wave.total_spawned_all = 0;
        wave.started = false;
        wave.between_waves = false;
        wave.cleared = false;
        wave.huge_wave = false;
        wave.start_delay_remaining = FIRST_WAVE_DELAY_TICKS;
        wave.inter_wave_remaining = 0;
        wave.spawn_timer_remaining = 0;
    }
    world
        .resource_mut::<SeedBankRuntime>()
        .recharge_remaining
        .clear();
    if let Ok(mut ui) = world.resource::<UiShare>().ui.lock() {
        normalize_progress_ui(&mut ui);
        ui.sun = world.resource::<Board>().sun;
        ui.shovel_selected = false;
        ui.progress = 0.0;
        ui.flags_total = world.resource::<WaveState>().num_waves.max(1);
        ui.flags_done = 0;
        let sun = world.resource::<Board>().sun;
        for slot in &mut ui.seed_bank {
            slot.selected = false;
            slot.ready = 1.0;
            slot.affordable = sun >= slot.cost;
        }
    }
    // Advice intro for the stage.
    {
        let mut advice = world.resource_mut::<AdviceState>();
        *advice = AdviceState::default();
        if stage == Stage::Night {
            advice.stage = AdviceStage::NightIntro;
            advice.hold_remaining = HUGE_WAVE_ADVICE_TICKS;
        }
    }
}

fn normalize_progress_ui(ui: &mut super::state::PilotUi) {
    if ui.unlocked_seed_names.is_empty() {
        ui.unlocked_seed_names = starting_unlocked_seeds();
    }
    ui.level_name = level_label(ui.adventure_level);
    let unlocked = ui.unlocked_seed_names.clone();
    ui.seed_bank.retain(|s| unlocked.contains(&s.seed_name));
    if ui.seed_bank.is_empty() {
        for name in starting_unlocked_seeds() {
            if ui.unlocked_seed_names.contains(&name) {
                let cost = super::comps::seed_def(&name).map(|d| d.cost).unwrap_or(100);
                ui.seed_bank.push(SeedSlot {
                    seed_name: name,
                    cost,
                    ready: 1.0,
                    affordable: true,
                    selected: false,
                });
            }
        }
    }
}

pub fn tick_advice(
    frame_ticks: Res<FrameTicks>,
    mut advice: ResMut<AdviceState>,
    stats: Res<SunStats>,
    plants: Query<&super::comps::Plant>,
    wave: Res<WaveState>,
    ui_share: Res<UiShare>,
) {
    // No early-out: advice text publishes even while paused.
    if wave.huge_wave && !advice.huge_announced {
        advice.stage = AdviceStage::HugeWave;
        advice.hold_remaining = HUGE_WAVE_ADVICE_TICKS;
        advice.huge_announced = true;
    }
    match advice.stage {
        AdviceStage::NightIntro => {
            advice.hold_remaining -= frame_ticks.0.max(0);
            if advice.hold_remaining <= 0 {
                advice.stage = AdviceStage::ClickSun;
            }
        }
        AdviceStage::ClickSun => {
            if stats.collected_total > 0 {
                advice.stage = AdviceStage::PlantSunflower;
            }
        }
        AdviceStage::PlantSunflower => {
            if plants.iter().any(|p| p.kind == PlantKind::Sunflower) {
                advice.stage = AdviceStage::ZombiesComing;
                advice.hold_remaining = ADVICE_HOLD_TICKS;
            }
        }
        AdviceStage::ZombiesComing => {
            if wave.started {
                advice.hold_remaining -= frame_ticks.0.max(0);
                if advice.hold_remaining <= 0 {
                    advice.stage = AdviceStage::Done;
                }
            }
        }
        AdviceStage::HugeWave => {
            advice.hold_remaining -= frame_ticks.0.max(0);
            if advice.hold_remaining <= 0 {
                advice.stage = AdviceStage::Done;
            }
        }
        AdviceStage::Done => {}
    }
    if let Ok(mut ui) = ui_share.ui.lock() {
        // Published as FTL keys; views translate at render time so a
        // mid-level language switch re-renders without re-ticking.
        let (text, visible) = match advice.stage {
            AdviceStage::NightIntro => ("advice-night-intro".to_string(), true),
            AdviceStage::ClickSun => ("advice-click-sun".to_string(), true),
            AdviceStage::PlantSunflower => ("advice-plant-sunflower".to_string(), true),
            AdviceStage::ZombiesComing => ("advice-zombies-coming".to_string(), true),
            AdviceStage::HugeWave => ("advice-huge-wave".to_string(), true),
            AdviceStage::Done => (String::new(), false),
        };
        ui.advice.text = text;
        ui.advice.visible = visible;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_labels_match_bevy_build() {
        assert_eq!(level_label(0), "1-1");
        assert_eq!(level_label(9), "1-10");
        assert_eq!(level_label(10), "2-1");
    }

    #[test]
    fn level_zero_wave_budgets() {
        let recipes = build_level_recipes(0);
        assert_eq!(recipes.len(), 4);
        assert!(recipes.iter().all(|r| !r.huge));
        assert_eq!(
            recipes.iter().map(|r| r.budget).collect::<Vec<_>>(),
            vec![1, 1, 2, 3]
        );
    }
}
