//! Economy chain: recharge, sky sun, sun fall/expire, planting, UI sync.
//! Faithful port of `game::systems` economy half + `sunflower.rs`.

use rand::RngExt;
use repame_sim::bevy_ecs::prelude::*;

use super::comps::{
    CherryBombFuse, ChomperState, DoomShroomFuse, GameplayCleanup, IceShroomFuse, JalapenoFuse,
    Plant, PlantKind, Pos, PotatoMineState, SeedDef, SkySun, SpikeweedState, SunDrop, SunProducer,
    seed_def,
};
use super::state::{
    Board, ClickConsumedThisFrame, ClickQueue, FlowControl, FrameTicks, GameTime, Overlay,
    SeedBankRuntime, Stage, SunStats, UiShare,
};
use crate::pilot::constants::*;

pub fn reset_click_consumed(
    mut consumed: ResMut<ClickConsumedThisFrame>,
    mut queue: ResMut<ClickQueue>,
) {
    consumed.0 = false;
    queue.clicks.clear();
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
    if board.stage == Stage::Night {
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
            Pos {
                x: logic_x,
                y: logic_y,
            },
        ));
    }
}

const SUN_FALL_SPEED_PPS: f32 = 22.0;

pub fn fall_and_expire_suns(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    game_time: Res<GameTime>,
    mut suns: Query<(Entity, &mut Pos, Option<&SkySun>, &SunDrop)>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    for (e, mut pos, sky, sun) in &mut suns {
        if let Some(sky) = sky {
            if pos.y < sky.target_logic_y {
                pos.y += SUN_FALL_SPEED_PPS * frame_ticks.0 as f32 / TICK_HZ;
                if pos.y > sky.target_logic_y {
                    pos.y = sky.target_logic_y;
                }
            }
        }
        if game_time.ticks - sun.born_tick >= SUN_LIFETIME_TICKS as i64 {
            commands.entity(e).try_despawn();
        }
    }
}

/// Sun pickup from the click queue (sun first, planting after).
pub fn collect_sun_clicks(
    mut commands: Commands,
    mut queue: ResMut<ClickQueue>,
    suns: Query<(Entity, &Pos, &SunDrop)>,
    mut board: ResMut<Board>,
    flow: Res<FlowControl>,
    mut consumed: ResMut<ClickConsumedThisFrame>,
    mut stats: ResMut<SunStats>,
) {
    if flow.overlay != Overlay::None {
        return;
    }
    let mut kept = Vec::new();
    for (x, y) in queue.clicks.drain(..) {
        if consumed.0 {
            kept.push((x, y));
            continue;
        }
        let mut picked = false;
        for (e, pos, sun) in &suns {
            let dx = pos.x - x;
            let dy = pos.y - y;
            if dx * dx + dy * dy <= SUN_PICK_RADIUS * SUN_PICK_RADIUS {
                board.sun += sun.value;
                stats.collected_total += 1;
                consumed.0 = true;
                commands.entity(e).try_despawn();
                picked = true;
                break;
            }
        }
        if !picked {
            kept.push((x, y));
        }
    }
    queue.clicks = kept;
}

pub fn handle_board_clicks(
    mut commands: Commands,
    mut board: ResMut<Board>,
    mut runtime: ResMut<SeedBankRuntime>,
    ui_share: Res<UiShare>,
    mut flow: ResMut<FlowControl>,
    consumed: Res<ClickConsumedThisFrame>,
    mut queue: ResMut<ClickQueue>,
) {
    if consumed.0 || flow.overlay != Overlay::None {
        return;
    }
    let click = queue.clicks.first().copied();
    let Some((x, y)) = click else { return };
    queue.clicks.clear();

    let Ok(mut ui) = ui_share.ui.lock() else {
        return;
    };
    let Some((col, row)) = super::state::logic_to_grid(x, y) else {
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
        drop(ui);
        flow.overlay = Overlay::NotEnoughSun;
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

    let (cx, cy) = super::state::grid_center_logic(col, row);
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
            Pos { x: cx, y: cy },
        ))
        .id();

    attach_plant_state(&mut commands, entity, def);
    board.cells[row][col] = Some(entity);

    fn clear_selection(ui: &mut super::state::PilotUi) {
        for s in &mut ui.seed_bank {
            s.selected = false;
        }
    }
}

fn attach_plant_state(commands: &mut Commands, entity: Entity, def: &SeedDef) {
    match def.kind {
        PlantKind::Sunflower => {
            commands.entity(entity).insert(SunProducer::default());
        }
        PlantKind::CherryBomb => {
            commands.entity(entity).insert(CherryBombFuse::default());
        }
        PlantKind::PotatoMine => {
            commands.entity(entity).insert(PotatoMineState::default());
        }
        PlantKind::Chomper => {
            commands.entity(entity).insert(ChomperState::default());
        }
        PlantKind::Jalapeno => {
            commands.entity(entity).insert(JalapenoFuse::default());
        }
        PlantKind::Spikeweed => {
            commands.entity(entity).insert(SpikeweedState::default());
        }
        PlantKind::IceShroom => {
            commands.entity(entity).insert(IceShroomFuse::default());
        }
        PlantKind::DoomShroom => {
            commands.entity(entity).insert(DoomShroomFuse::default());
        }
        _ => {}
    }
}

pub fn tick_sun_producers(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    game_time: Res<GameTime>,
    mut producers: Query<(&Plant, &Pos, &mut SunProducer)>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    let mut rng = rand::rng();
    for (plant, pos, mut producer) in &mut producers {
        if plant.kind != PlantKind::Sunflower {
            continue;
        }
        producer.countdown -= frame_ticks.0;
        if producer.countdown > 0 {
            continue;
        }
        producer.countdown += SUNFLOWER_SUN_INTERVAL_TICKS;
        let ox: f32 = rng.random_range(-14.0..14.0);
        commands.spawn((
            GameplayCleanup,
            SunDrop {
                born_tick: game_time.ticks,
                value: SUNFLOWER_SUN_VALUE,
            },
            Pos {
                x: pos.x + ox,
                y: pos.y + 26.0,
            },
        ));
    }
}

pub fn sync_ui(
    board: Res<Board>,
    runtime: Res<SeedBankRuntime>,
    wave: Res<super::state::WaveState>,
    zombies: Query<Entity, With<super::comps::Zombie>>,
    ui_share: Res<UiShare>,
) {
    let Ok(mut ui) = ui_share.ui.lock() else {
        return;
    };
    ui.sun = board.sun;
    let total_all = wave.total_in_level.max(1) as f32;
    let spawned_all = wave.total_spawned_all as f32;
    let alive = zombies.iter().count() as f32;
    let resolved = (spawned_all - alive).max(0.0);
    ui.progress = (resolved / total_all).clamp(0.0, 1.0);
    for (i, slot) in ui.seed_bank.iter_mut().enumerate() {
        let total_ticks = super::comps::seed_def(&slot.seed_name)
            .map(|d| d.recharge_ticks)
            .unwrap_or(FAST_RECHARGE_TICKS)
            .max(1);
        let rem = runtime.recharge_remaining.get(i).copied().unwrap_or(0);
        slot.ready = (1.0 - rem as f32 / total_ticks as f32).clamp(0.0, 1.0);
        slot.affordable = board.sun >= slot.cost;
    }
}
