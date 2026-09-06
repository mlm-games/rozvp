//! Core gameplay systems: level setup/teardown, sun economy, planting, HUD sync.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rand::RngExt;

use crate::app::{OverlayMenu, Paused};
use crate::game::board::Board;
use crate::game::constants::*;
use crate::game::defs::{PlantKind, seed_def};
use crate::game::specials::CherryBombFuse;
use crate::game::sunflower::SunProducer;
use crate::game::tick::{FrameTicks, GameTime};
use crate::game::zombie::WaveState;
use crate::menus::UiBridge;

/// Marks every entity spawned for a level so it can be nuked on exit.
#[derive(Component)]
pub struct GameplayCleanup;

#[derive(Component, Clone, Copy, Debug)]
pub struct Plant {
    pub kind: PlantKind,
    pub row: usize,
    pub col: usize,
    pub hp: i32,
    pub attack_cooldown_remaining: i32,
    pub attack_cooldown_max: i32,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct SunDrop {
    pub born_tick: i64,
    pub value: i32,
}

#[derive(Resource, Debug, Default)]
pub struct SeedBankRuntime {
    pub recharge_remaining: Vec<i32>,
}

/// Set when a click was consumed by an earlier system this frame (e.g. sun pickup),
/// so later click systems ignore the same press.
#[derive(Resource, Debug, Default)]
pub struct ClickConsumedThisFrame(pub bool);

/// Counts actual sun collections (survives spending, unlike `board.sun > STARTING_SUN`).
#[derive(Resource, Debug, Default)]
pub struct SunStats {
    pub collected_total: u32,
}

pub fn reset_click_consumed(mut consumed: ResMut<ClickConsumedThisFrame>) {
    consumed.0 = false;
}

/// Rebuilds every wave plan + counter for `level` into `wave`. Called from
/// `enter_level` / `apply_restart` while the UI lock is already held, so
/// `ui.adventure_level` is read without extra lock round-trips.
fn load_wave_plans(wave: &mut WaveState, level: u32) {
    let (plans, spent) = crate::game::zombie::build_level_waves(level);
    let recipes = crate::game::level_flow::build_level_recipes(level);

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

pub fn enter_level(
    mut commands: Commands,
    mut board: ResMut<Board>,
    mut runtime: ResMut<SeedBankRuntime>,
    mut stats: ResMut<SunStats>,
    mut consumed: ResMut<ClickConsumedThisFrame>,
    bridge: Res<UiBridge>,
    mut overlay: ResMut<OverlayMenu>,
    mut paused: ResMut<Paused>,
    mut wave: ResMut<WaveState>,
) {
    board.reset();
    *stats = SunStats::default();
    consumed.0 = false;
    runtime.recharge_remaining.clear();

    *overlay = OverlayMenu::None;
    paused.0 = false;

    if let Ok(mut ui) = bridge.shared.lock() {
        crate::game::level_flow::normalize_progress_ui(&mut ui);

        board.stage = crate::game::level_flow::stage_for_level(ui.adventure_level);
        spawn_level_entities(&mut commands, board.stage);

        *wave = WaveState::default();
        load_wave_plans(&mut wave, ui.adventure_level);
        runtime.recharge_remaining = vec![0; ui.seed_bank.len()];
        ui.sun = board.sun;
        ui.shovel_selected = false;
        ui.progress = 0.0;
        ui.flags_total = wave.num_waves.max(1);
        ui.flags_done = 0;
        for slot in &mut ui.seed_bank {
            slot.selected = false;
            slot.ready = 1.0;
            slot.affordable = board.sun >= slot.cost;
        }
    }
}

/// Lawn placeholder art: checkerboard tiles + porch strip, tinted per stage.
fn spawn_level_entities(commands: &mut Commands, stage: crate::game::board::Stage) {
    use crate::game::board::Stage;

    let (tile_a, tile_b, porch) = match stage {
        Stage::Day => (
            Color::srgb(0.385, 0.635, 0.235),
            Color::srgb(0.345, 0.585, 0.215),
            Color::srgb(0.55, 0.42, 0.26),
        ),
        Stage::Night => (
            Color::srgb(0.20, 0.32, 0.24),
            Color::srgb(0.16, 0.27, 0.20),
            Color::srgb(0.36, 0.31, 0.28),
        ),
    };

    for row in 0..LAWN_ROWS {
        for col in 0..LAWN_COLS {
            let shade = if (row + col) % 2 == 0 { tile_a } else { tile_b };
            let center = Board::grid_center_logic(col, row);
            commands.spawn((
                GameplayCleanup,
                Sprite {
                    color: shade,
                    custom_size: Some(Vec2::new(GRID_CELL_W, GRID_CELL_H)),
                    ..default()
                },
                Board::logic_to_world(center, -10.0),
            ));
        }
    }
    // Porch (left of lawn).
    commands.spawn((
        GameplayCleanup,
        Sprite {
            color: porch,
            custom_size: Some(Vec2::new(LAWN_XMIN, GRID_CELL_H * LAWN_ROWS as f32)),
            ..default()
        },
        Board::logic_to_world(
            Vec2::new(
                LAWN_XMIN * 0.5,
                LAWN_YMIN + GRID_CELL_H * LAWN_ROWS as f32 * 0.5,
            ),
            -11.0,
        ),
    ));
}

/// Consumes a UI restart request. Runs outside the paused-gated chain so it
/// works from the Pause / GameOver overlays; it also unpauses.
pub fn apply_restart(
    mut pending: ResMut<crate::app::PendingRestart>,
    mut commands: Commands,
    q: Query<Entity, With<GameplayCleanup>>,
    mut board: ResMut<Board>,
    mut runtime: ResMut<SeedBankRuntime>,
    mut stats: ResMut<SunStats>,
    mut consumed: ResMut<ClickConsumedThisFrame>,
    mut wave: ResMut<WaveState>,
    mut advice: ResMut<crate::game::advice::AdviceState>,
    bridge: Res<UiBridge>,
    mut paused: ResMut<Paused>,
    mut overlay: ResMut<OverlayMenu>,
) {
    if !pending.0 {
        return;
    }
    pending.0 = false;

    for e in &q {
        commands.entity(e).try_despawn();
    }
    board.reset();
    *wave = WaveState::default();
    *stats = SunStats::default();
    consumed.0 = false;
    *advice = crate::game::advice::AdviceState::default();
    runtime.recharge_remaining.clear();

    paused.0 = false;
    *overlay = OverlayMenu::None;

    if let Ok(mut ui) = bridge.shared.lock() {
        crate::game::level_flow::normalize_progress_ui(&mut ui);

        board.stage = crate::game::level_flow::stage_for_level(ui.adventure_level);
        spawn_level_entities(&mut commands, board.stage);
        super::mower::spawn_mowers(commands);
        crate::game::advice::apply_intro_stage(&mut advice, board.stage);

        load_wave_plans(&mut wave, ui.adventure_level);
        runtime.recharge_remaining = vec![0; ui.seed_bank.len()];
        ui.sun = board.sun;
        ui.shovel_selected = false;
        ui.progress = 0.0;
        ui.flags_total = wave.num_waves.max(1);
        ui.flags_done = 0;
        for slot in &mut ui.seed_bank {
            slot.selected = false;
            slot.ready = 1.0;
            slot.affordable = board.sun >= slot.cost;
        }
    }
}

pub fn exit_level(
    mut commands: Commands,
    q: Query<Entity, With<GameplayCleanup>>,
    mut board: ResMut<Board>,
    mut runtime: ResMut<SeedBankRuntime>,
    mut stats: ResMut<SunStats>,
) {
    for e in &q {
        commands.entity(e).try_despawn();
    }
    board.reset();
    runtime.recharge_remaining.clear();
    *stats = SunStats::default();
}

pub fn tick_seed_recharge(frame_ticks: Res<FrameTicks>, mut runtime: ResMut<SeedBankRuntime>) {
    if frame_ticks.0 <= 0 {
        return;
    }
    for rem in &mut runtime.recharge_remaining {
        if *rem > 0 {
            *rem = (*rem - frame_ticks.0).max(0);
        }
    }
}

pub fn spawn_sky_sun(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    game_time: Res<GameTime>,
    mut board: ResMut<Board>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    // Night: no sky sun (mushrooms/nocturnal levels only get droppers).
    if board.stage == crate::game::board::Stage::Night {
        return;
    }
    board.sky_sun_timer -= frame_ticks.0;
    while board.sky_sun_timer <= 0 {
        board.sky_sun_timer += SKY_SUN_INTERVAL_TICKS;

        let mut rng = rand::rng();
        let logic_x = rng.random_range(LAWN_XMIN + 40.0..LAWN_XMIN + GRID_CELL_W * 8.0);
        let logic_y = LAWN_YMIN - 30.0;
        let target_y = rng.random_range(LAWN_YMIN + GRID_CELL_H..LAWN_YMIN + GRID_CELL_H * 4.0);

        commands.spawn((
            GameplayCleanup,
            SunDrop {
                born_tick: game_time.ticks,
                value: SUN_VALUE,
            },
            SkySun {
                target_logic_y: target_y,
            },
            Sprite {
                color: Color::srgb(1.0, 0.9, 0.2),
                custom_size: Some(Vec2::splat(28.0)),
                ..default()
            },
            Board::logic_to_world(Vec2::new(logic_x, logic_y), SUN_Z),
        ));
    }
}

/// Sky sun falls until it reaches its target row, then sits.
const SUN_FALL_SPEED_PPS: f32 = 22.0;

#[derive(Component, Clone, Copy, Debug)]
pub struct SkySun {
    pub target_logic_y: f32,
}

pub fn fall_and_expire_suns(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    game_time: Res<GameTime>,
    mut suns: Query<(Entity, &mut Transform, Option<&SkySun>, &SunDrop)>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    for (e, mut tf, sky, sun) in &mut suns {
        if let Some(sky) = sky {
            let target_world_y = Board::logic_to_world(Vec2::ZERO.with_y(sky.target_logic_y), 0.0)
                .translation
                .y;
            // Falling = decreasing world y toward the target.
            if tf.translation.y > target_world_y {
                tf.translation.y -= SUN_FALL_SPEED_PPS * frame_ticks.0 as f32 / TICK_HZ;
                if tf.translation.y < target_world_y {
                    tf.translation.y = target_world_y;
                }
            }
        }
        if game_time.ticks - sun.born_tick >= SUN_LIFETIME_TICKS as i64 {
            commands.entity(e).try_despawn();
        }
    }
}

pub fn collect_sun_clicks(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    suns: Query<(Entity, &Transform, &SunDrop)>,
    mut commands: Commands,
    mut board: ResMut<Board>,
    overlay: Res<OverlayMenu>,
    mut consumed: ResMut<ClickConsumedThisFrame>,
    mut stats: ResMut<SunStats>,
) {
    if !mouse.just_pressed(MouseButton::Left) || *overlay != OverlayMenu::None {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, cam_tf)) = camera_q.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(world) = camera.viewport_to_world_2d(cam_tf, cursor) else {
        return;
    };

    for (e, tf, sun) in &suns {
        if tf.translation.truncate().distance(world) <= SUN_PICK_RADIUS {
            board.sun += sun.value;
            stats.collected_total += 1;
            consumed.0 = true;
            commands.entity(e).try_despawn();
            break;
        }
    }
}

pub fn handle_board_clicks(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut commands: Commands,
    mut board: ResMut<Board>,
    mut runtime: ResMut<SeedBankRuntime>,
    bridge: Res<UiBridge>,
    mut overlay: ResMut<OverlayMenu>,
    consumed: Res<ClickConsumedThisFrame>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if consumed.0 || *overlay != OverlayMenu::None {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, cam_tf)) = camera_q.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(world) = camera.viewport_to_world_2d(cam_tf, cursor) else {
        return;
    };

    let logic = Board::world_to_logic(world);
    let Some((col, row)) = Board::logic_to_grid(logic.x, logic.y) else {
        return;
    };

    let Ok(mut ui) = bridge.shared.lock() else {
        return;
    };

    if ui.shovel_selected {
        if let Some(entity) = board.cells[row][col].take() {
            commands.entity(entity).try_despawn();
        }
        ui.shovel_selected = false;
        return;
    }

    let Some(slot_idx) = ui.seed_bank.iter().position(|s| s.selected) else {
        return;
    };
    let Some(slot) = ui.seed_bank.get(slot_idx).cloned() else {
        return;
    };
    let Some(def) = seed_def(&slot.seed_name) else {
        return;
    };

    let clear_selection = |ui: &mut crate::app::SharedUi| {
        for s in &mut ui.seed_bank {
            s.selected = false;
        }
    };

    if runtime
        .recharge_remaining
        .get(slot_idx)
        .copied()
        .unwrap_or(0)
        > 0
    {
        clear_selection(&mut ui);
        return;
    }
    if board.sun < def.cost {
        clear_selection(&mut ui);
        *overlay = OverlayMenu::NotEnoughSun;
        return;
    }
    if !board.can_plant(col, row) {
        clear_selection(&mut ui);
        return;
    }

    board.sun -= def.cost;
    if slot_idx >= runtime.recharge_remaining.len() {
        runtime.recharge_remaining.resize(slot_idx + 1, 0);
    }
    runtime.recharge_remaining[slot_idx] = def.recharge_ticks;
    clear_selection(&mut ui);

    let logic_center = Board::grid_center_logic(col, row);
    let world_tf = Board::logic_to_world(logic_center, PLANT_Z);

    let entity = commands
        .spawn((
            GameplayCleanup,
            Plant {
                kind: def.kind,
                row,
                col,
                hp: def.hp,
                attack_cooldown_remaining: 0,
                attack_cooldown_max: def.attack_cooldown_ticks,
            },
            Sprite {
                color: def.color,
                custom_size: Some(match def.kind {
                    PlantKind::WallNut => Vec2::new(58.0, 64.0),
                    PlantKind::CherryBomb => Vec2::new(54.0, 54.0),
                    // Ground hazard reads as a flat strip.
                    PlantKind::Spikeweed => Vec2::new(64.0, 22.0),
                    _ => Vec2::new(56.0, 72.0),
                }),
                ..default()
            },
            world_tf,
        ))
        .id();

    if def.kind == PlantKind::Sunflower {
        commands.entity(entity).insert(SunProducer::default());
    }

    if def.kind == PlantKind::CherryBomb {
        commands.entity(entity).insert(CherryBombFuse::default());
    }

    if def.kind == PlantKind::PotatoMine {
        commands
            .entity(entity)
            .insert(crate::game::specials::PotatoMineState::default());
    }

    if def.kind == PlantKind::Chomper {
        commands
            .entity(entity)
            .insert(crate::game::specials::ChomperState::default());
    }

    match def.kind {
        PlantKind::Jalapeno => {
            commands
                .entity(entity)
                .insert(crate::game::specials::JalapenoFuse::default());
        }
        PlantKind::Spikeweed => {
            commands
                .entity(entity)
                .insert(crate::game::specials::SpikeweedState::default());
        }
        PlantKind::IceShroom => {
            commands
                .entity(entity)
                .insert(crate::game::specials::IceShroomFuse::default());
        }
        PlantKind::DoomShroom => {
            commands
                .entity(entity)
                .insert(crate::game::specials::DoomShroomFuse::default());
        }
        // Squash resolves atomically; no persistent state component.
        _ => {}
    }

    board.cells[row][col] = Some(entity);
}

pub fn sync_ui(
    board: Res<Board>,
    runtime: Res<SeedBankRuntime>,
    wave: Res<WaveState>,
    zombies: Query<Entity, With<crate::game::zombie::Zombie>>,
    bridge: Res<UiBridge>,
) {
    let Ok(mut ui) = bridge.shared.lock() else {
        return;
    };

    ui.sun = board.sun;

    // Progress spans the whole level: resolved zombies over total planned.
    let total_all = wave.total_in_level.max(1) as f32;
    let spawned_all = wave.total_spawned_all as f32;
    let alive = zombies.iter().count() as f32;
    let resolved = (spawned_all - alive).max(0.0);
    ui.progress = (resolved / total_all).clamp(0.0, 1.0);

    // flags_total/done are owned by load_wave_plans / advance_or_complete_level.

    for (i, slot) in ui.seed_bank.iter_mut().enumerate() {
        let total_ticks = seed_def(&slot.seed_name)
            .map(|d| d.recharge_ticks)
            .unwrap_or(FAST_RECHARGE_TICKS)
            .max(1);
        let rem = runtime.recharge_remaining.get(i).copied().unwrap_or(0);
        let ready = 1.0 - rem as f32 / total_ticks as f32;
        slot.ready = ready.clamp(0.0, 1.0);
        slot.affordable = board.sun >= slot.cost;
    }
}
