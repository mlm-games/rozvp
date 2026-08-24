//! Instant-plant + special behaviors (Cherry Bomb blast, Potato Mine,
//! Chomper bite).

use bevy::prelude::*;

use crate::game::board::Board;
use crate::game::constants::{
    CHERRY_BOMB_DAMAGE, CHERRY_BOMB_FUSE_TICKS, CHOMPER_CHEW_TICKS, CHOMPER_REACH,
    DOOM_CRATER_TICKS, DOOM_SHROOM_DAMAGE, DOOM_SHROOM_FUSE_TICKS, ICE_SHROOM_CHILL_TICKS,
    ICE_SHROOM_FREEZE_TICKS, ICE_SHROOM_FUSE_TICKS, JALAPENO_DAMAGE, JALAPENO_FUSE_TICKS,
    LAWN_COLS, POTATO_MINE_ARM_TICKS, SPIKEWEED_DAMAGE, SPIKEWEED_HIT_INTERVAL_TICKS,
    SQUASH_DAMAGE, SQUASH_TRIGGER_RANGE_PX,
};
use crate::game::defs::PlantKind;
use crate::game::systems::{GameplayCleanup, Plant};
use crate::game::tick::FrameTicks;
use crate::game::zombie::{Chilled, Frozen, Zombie};

#[derive(Component, Clone, Copy, Debug)]
pub struct CherryBombFuse {
    pub remaining: i32,
}

impl Default for CherryBombFuse {
    fn default() -> Self {
        Self {
            remaining: CHERRY_BOMB_FUSE_TICKS, // 120 = 1.2 s
        }
    }
}

/// Strict tile rule: only zombies standing on the bomb's cell or its 8
/// neighbors (Chebyshev distance <= 1) are hit. Off-grid zombies (spawn strip /
/// house edge) are skipped — lane centers make this a non-issue on Day 1.
pub fn tick_cherry_bombs(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut board: ResMut<Board>,
    mut bombs: Query<(Entity, &Plant, &mut CherryBombFuse)>,
    mut zombies: Query<(Entity, &mut Zombie, &Transform)>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }

    for (bomb_e, plant, mut fuse) in &mut bombs {
        if plant.kind != PlantKind::CherryBomb {
            continue;
        }

        fuse.remaining -= frame_ticks.0;
        if fuse.remaining > 0 {
            continue;
        }

        for (z_e, mut zombie, z_tf) in &mut zombies {
            let z_logic = Board::world_to_logic(z_tf.translation.truncate());
            let Some((z_col, z_row)) = Board::logic_to_grid(z_logic.x, z_logic.y) else {
                continue;
            };

            let d_row = (z_row as i32 - plant.row as i32).abs();
            let d_col = (z_col as i32 - plant.col as i32).abs();
            if d_row <= 1 && d_col <= 1 {
                // Armor first, overflow to body; 1800 >= bucket total (1370).
                if zombie.take_damage(CHERRY_BOMB_DAMAGE) {
                    commands.entity(z_e).try_despawn();
                }
            }
        }

        board.cells[plant.row][plant.col] = None;
        commands.entity(bomb_e).try_despawn();
    }
}

// Potato Mine

#[derive(Component, Clone, Copy, Debug)]
pub struct PotatoMineState {
    pub armed: bool,
    pub arm_timer: i32,
}

impl Default for PotatoMineState {
    fn default() -> Self {
        Self {
            armed: false,
            arm_timer: POTATO_MINE_ARM_TICKS, // 15 s
        }
    }
}

/// Only arms the mine. Triggering is handled from zombie movement/eating
/// (`move_and_eat_zombies`), so dormant mines are edible and armed mines
/// detonate on contact.
pub fn tick_potato_mines(
    frame_ticks: Res<FrameTicks>,
    mut mines: Query<(&Plant, &mut PotatoMineState), Without<Zombie>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }

    for (plant, mut state) in &mut mines {
        if plant.kind != PlantKind::PotatoMine {
            continue;
        }

        if state.armed {
            continue;
        }

        state.arm_timer -= frame_ticks.0;
        if state.arm_timer <= 0 {
            state.armed = true;
        }
    }
}

// Chomper

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ChomperState {
    /// >0 while digesting (cannot bite).
    pub chewing_timer: i32,
}

/// Melee: ready chompers instantly kill the first zombie in reach ahead of them,
/// then enter a long chew cooldown. While chewing they are still edible.
pub fn tick_chompers(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut chompers: Query<(&Plant, &Transform, &mut ChomperState), Without<Zombie>>,
    mut zombies: Query<(Entity, &mut Zombie, &Transform), Without<Plant>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }

    for (plant, tf, mut state) in &mut chompers {
        if plant.kind != PlantKind::Chomper {
            continue;
        }

        if state.chewing_timer > 0 {
            state.chewing_timer = (state.chewing_timer - frame_ticks.0).max(0);
            continue;
        }

        let plant_x = Board::world_to_logic(tf.translation.truncate()).x;

        for (z_e, mut zombie, z_tf) in &mut zombies {
            if zombie.row != plant.row {
                continue;
            }
            let z_x = Board::world_to_logic(z_tf.translation.truncate()).x;
            // Bite zone: slightly behind plant center through CHOMPER_REACH ahead.
            if z_x > plant_x - 10.0 && z_x < plant_x + CHOMPER_REACH {
                if zombie.take_damage(9999) {
                    commands.entity(z_e).try_despawn();
                }
                state.chewing_timer = CHOMPER_CHEW_TICKS;
                break;
            }
        }
    }
}

// Squash
// One-time crush: smashes the nearest enemy in trigger range, then is spent.
// No persistent state component needed — resolution is atomic.

/// Squash: nearest non-hypnotized zombie in ±range on its row gets crushed.
pub fn tick_squash(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut board: ResMut<Board>,
    squashes: Query<(Entity, &Plant, &Transform), Without<Zombie>>,
    mut zombies: Query<(Entity, &mut Zombie, &Transform), Without<Plant>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }

    for (sq_e, plant, tf) in &squashes {
        if plant.kind != PlantKind::Squash {
            continue;
        }
        let sq_x = tf.translation.x;

        // Snapshot scan; nearest target wins.
        let mut target: Option<(Entity, f32)> = None;
        for (z_e, z, z_tf) in &zombies {
            if z.hypnotized || z.row != plant.row {
                continue;
            }
            let d = (z_tf.translation.x - sq_x).abs();
            if d <= SQUASH_TRIGGER_RANGE_PX && target.is_none_or(|(_, bd)| d < bd) {
                target = Some((z_e, d));
            }
        }

        if let Some((z_e, _)) = target
            && let Ok((_, mut zombie, _)) = zombies.get_mut(z_e)
            && zombie.take_damage(SQUASH_DAMAGE)
        {
            commands.entity(z_e).try_despawn();
            board.cells[plant.row][plant.col] = None;
            commands.entity(sq_e).try_despawn();
        }
    }
}

// Jalapeño
#[derive(Component, Clone, Copy, Debug)]
pub struct JalapenoFuse {
    pub remaining: i32,
}

impl Default for JalapenoFuse {
    fn default() -> Self {
        Self {
            remaining: JALAPENO_FUSE_TICKS,
        }
    }
}

/// Incinerates every non-hypnotized zombie in the plant's row.
pub fn tick_jalapenos(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut board: ResMut<Board>,
    mut jals: Query<(Entity, &Plant, &mut JalapenoFuse)>,
    mut zombies: Query<(Entity, &mut Zombie), Without<Plant>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }

    for (j_e, plant, mut fuse) in &mut jals {
        if plant.kind != PlantKind::Jalapeno {
            continue;
        }
        fuse.remaining -= frame_ticks.0;
        if fuse.remaining > 0 {
            continue;
        }

        for (z_e, mut zombie) in &mut zombies {
            if zombie.row == plant.row && !zombie.hypnotized && zombie.take_damage(JALAPENO_DAMAGE)
            {
                commands.entity(z_e).try_despawn();
            }
        }
        board.cells[plant.row][plant.col] = None;
        commands.entity(j_e).try_despawn();
    }
}

// Spikeweed
#[derive(Component, Clone, Debug, Default)]
pub struct SpikeweedState {
    /// Per-zombie hit cooldowns: (entity, remaining_ticks).
    pub cooldowns: Vec<(Entity, i32)>,
}

/// Ground hazard: 20 dmg per second to each walker standing on it.
/// Zombies never block on or eat it (see `move_and_eat_zombies`).
pub fn tick_spikeweeds(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    weeds: Query<(&Plant, &Transform, &mut SpikeweedState), Without<Zombie>>,
    mut zombies: Query<(Entity, &mut Zombie, &Transform), Without<Plant>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }

    for (plant, tf, mut state) in weeds {
        if plant.kind != PlantKind::Spikeweed {
            continue;
        }

        for (_, rem) in state.cooldowns.iter_mut() {
            *rem -= frame_ticks.0;
        }
        state.cooldowns.retain(|(_, rem)| *rem > 0);

        let mut hits: Vec<Entity> = Vec::new();
        for (z_e, _z, z_tf) in &zombies {
            if (z_tf.translation.x - tf.translation.x).abs() <= 40.0
                && !state.cooldowns.iter().any(|(e, _)| *e == z_e)
            {
                hits.push(z_e);
            }
        }

        for z_e in hits {
            state.cooldowns.push((z_e, SPIKEWEED_HIT_INTERVAL_TICKS));
            if let Ok((_, mut zombie, _)) = zombies.get_mut(z_e)
                && zombie.take_damage(SPIKEWEED_DAMAGE)
            {
                commands.entity(z_e).try_despawn();
            }
        }
    }
}

// Ice-shroom
#[derive(Component, Clone, Copy, Debug)]
pub struct IceShroomFuse {
    pub remaining: i32,
}

impl Default for IceShroomFuse {
    fn default() -> Self {
        Self {
            remaining: ICE_SHROOM_FUSE_TICKS,
        }
    }
}

/// Freezes every non-hypnotized zombie solid, then a lingering chill takes
/// over on thaw (`Frozen` -> `Chilled` conversion happens in the move system).
pub fn tick_ice_shrooms(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut board: ResMut<Board>,
    mut ices: Query<(Entity, &Plant, &mut IceShroomFuse)>,
    zombies: Query<(Entity, &Zombie)>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }

    for (i_e, plant, mut fuse) in &mut ices {
        if plant.kind != PlantKind::IceShroom {
            continue;
        }
        fuse.remaining -= frame_ticks.0;
        if fuse.remaining > 0 {
            continue;
        }

        for (z_e, zombie) in &zombies {
            if zombie.hypnotized {
                continue;
            }
            commands.entity(z_e).remove::<Chilled>();
            commands.entity(z_e).insert(Frozen {
                remaining_ticks: ICE_SHROOM_FREEZE_TICKS,
                chill_after_ticks: ICE_SHROOM_CHILL_TICKS,
            });
        }
        board.cells[plant.row][plant.col] = None;
        commands.entity(i_e).try_despawn();
    }
}

// Doom-shroom ─────────────────────────────────────────────
#[derive(Component, Clone, Copy, Debug)]
pub struct DoomShroomFuse {
    pub remaining: i32,
}

impl Default for DoomShroomFuse {
    fn default() -> Self {
        Self {
            remaining: DOOM_SHROOM_FUSE_TICKS,
        }
    }
}

/// Crater left by Doom-shroom; blocks planting while it exists.
#[derive(Component, Clone, Copy, Debug)]
pub struct Crater {
    pub remaining: i32,
    pub row: usize,
    pub col: usize,
}

/// 5x5 devastation (Chebyshev <= 2): kills nearly everything, destroys plants
/// in the area, and leaves an unplantable crater that expires after 180 s.
pub fn tick_doom_shrooms(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut board: ResMut<Board>,
    mut dooms: Query<(Entity, &Plant, &mut DoomShroomFuse)>,
    mut zombies: Query<(Entity, &mut Zombie, &Transform), Without<Plant>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }

    for (d_e, plant, mut fuse) in &mut dooms {
        if plant.kind != PlantKind::DoomShroom {
            continue;
        }
        fuse.remaining -= frame_ticks.0;
        if fuse.remaining > 0 {
            continue;
        }

        // Zombies in the blast radius.
        for (z_e, mut zombie, z_tf) in &mut zombies {
            if zombie.hypnotized {
                continue;
            }
            let z_logic = Board::world_to_logic(z_tf.translation.truncate());
            let Some((z_col, z_row)) = Board::logic_to_grid(z_logic.x, z_logic.y) else {
                continue;
            };
            let dr = (z_row as i32 - plant.row as i32).abs();
            let dc = (z_col as i32 - plant.col as i32).abs();
            if dr <= 2 && dc <= 2 && zombie.take_damage(DOOM_SHROOM_DAMAGE) {
                commands.entity(z_e).try_despawn();
            }
        }

        // Plants in the blast radius are consumed by the crater.
        for dr in -2i32..=2 {
            for dc in -2i32..=2 {
                let r = plant.row as i32 + dr;
                let c = plant.col as i32 + dc;
                if r < 0
                    || c < 0
                    || r >= crate::game::constants::LAWN_ROWS as i32
                    || c >= LAWN_COLS as i32
                {
                    continue;
                }
                let (r, c) = (r as usize, c as usize);
                if let Some(p_e) = board.cells[r][c].take() {
                    commands.entity(p_e).try_despawn();
                }
            }
        }

        // The crater itself occupies the cell, blocking replanting.
        let center = Board::grid_center_logic(plant.col, plant.row);
        let crater_e = commands
            .spawn((
                GameplayCleanup,
                Crater {
                    remaining: DOOM_CRATER_TICKS,
                    row: plant.row,
                    col: plant.col,
                },
                Sprite {
                    color: Color::srgb(0.15, 0.12, 0.10),
                    custom_size: Some(Vec2::new(70.0, 60.0)),
                    ..default()
                },
                Board::logic_to_world(center, 1.0),
            ))
            .id();
        board.cells[plant.row][plant.col] = Some(crater_e);
        commands.entity(d_e).try_despawn();
    }
}

pub fn tick_craters(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut board: ResMut<Board>,
    mut craters: Query<(Entity, &mut Crater)>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    for (c_e, mut crater) in &mut craters {
        crater.remaining -= frame_ticks.0;
        if crater.remaining <= 0 {
            if board.cells[crater.row][crater.col] == Some(c_e) {
                board.cells[crater.row][crater.col] = None;
            }
            commands.entity(c_e).try_despawn();
        }
    }
}
