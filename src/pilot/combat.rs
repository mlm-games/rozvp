//! Combat chain: shooters, peas, specials, zombie move/eat, hypno brawls,
//! mowers, lose/win, corpse countdown. Faithful port of `projectile.rs`,
//! `specials.rs`, `mower.rs`, and the combat half of `zombie.rs` into
//! logic-space `Pos` (bevy world offsets cancel in every distance used).

use rand::RngExt;
use repame_sim::bevy_ecs::prelude::*;

use super::comps::{
    CherryBombFuse, Chilled, ChomperState, Crater, DoomShroomFuse, Dying, Eating, Frozen,
    GameplayCleanup, IceShroomFuse, JalapenoFuse, LawnMower, PeaKind, PeaProjectile, Plant,
    PlantKind, Pos, PotatoMineState, SpikeweedState, Zombie,
};
use super::state::{
    Board, FlowControl, FrameTicks, grid_center_logic, logic_to_grid, row_center_y,
};
use crate::pilot::constants::*;

/// Lethal damage marker: corpse plays the fall one-shot (`Dying`), rig
/// listens for the same transition. Explosive/devour kills despawn
/// instantly at their call sites (matches bevy build).
pub fn kill_zombie(commands: &mut Commands, e: Entity) {
    commands.entity(e).insert(Dying {
        remaining_ticks: 80,
    });
    commands.entity(e).remove::<Eating>();
}

pub fn tick_plants_and_fire(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut plants: Query<(&mut Plant, &Pos), Without<Zombie>>,
    zombies: Query<(&Zombie, &Pos), Without<Plant>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    for (mut plant, pos) in &mut plants {
        if plant.attack_cooldown_remaining > 0 {
            plant.attack_cooldown_remaining =
                (plant.attack_cooldown_remaining - frame_ticks.0).max(0);
        }
        if !matches!(
            plant.kind,
            PlantKind::Peashooter
                | PlantKind::SnowPea
                | PlantKind::Repeater
                | PlantKind::Threepeater
        ) || plant.attack_cooldown_remaining > 0
        {
            continue;
        }
        let is_threepeater = plant.kind == PlantKind::Threepeater;
        let has_target_ahead = zombies.iter().any(|(zombie, zpos)| {
            let lane_ok = if is_threepeater {
                (zombie.row as i32 - plant.row as i32).abs() <= 1
            } else {
                zombie.row == plant.row
            };
            lane_ok && zpos.x > pos.x + 12.0
        });
        if !has_target_ahead {
            continue;
        }
        plant.attack_cooldown_remaining = plant.attack_cooldown_max;
        let is_snow = plant.kind == PlantKind::SnowPea;
        commands.spawn((
            GameplayCleanup,
            PeaProjectile {
                kind: if is_snow {
                    PeaKind::Snow
                } else {
                    PeaKind::Normal
                },
                row: plant.row,
                damage: PEA_DAMAGE,
                speed_pps: PEA_SPEED_PPS,
            },
            Pos {
                x: pos.x + 26.0,
                y: pos.y + 8.0,
            },
        ));
        if plant.kind == PlantKind::Repeater {
            commands.spawn((
                GameplayCleanup,
                PeaProjectile {
                    kind: PeaKind::Normal,
                    row: plant.row,
                    damage: PEA_DAMAGE,
                    speed_pps: PEA_SPEED_PPS,
                },
                Pos {
                    x: pos.x + 8.0,
                    y: pos.y + 8.0,
                },
            ));
        }
        if is_threepeater {
            for target_row in [plant.row.wrapping_sub(1), plant.row + 1] {
                if target_row >= LAWN_ROWS {
                    continue;
                }
                commands.spawn((
                    GameplayCleanup,
                    PeaProjectile {
                        kind: PeaKind::Normal,
                        row: target_row,
                        damage: PEA_DAMAGE,
                        speed_pps: PEA_SPEED_PPS,
                    },
                    Pos {
                        x: pos.x + 26.0,
                        y: row_center_y(target_row) + 8.0,
                    },
                ));
            }
        }
    }
}

pub fn move_peas_and_hit(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut peas: Query<(Entity, &mut Pos, &mut PeaProjectile), Without<Zombie>>,
    mut zombies: Query<(Entity, &mut Zombie, &Pos, Has<Dying>), Without<PeaProjectile>>,
    torchwoods: Query<(&Plant, &Pos), (Without<Zombie>, Without<PeaProjectile>)>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    let dt = frame_ticks.0 as f32 / TICK_HZ;
    for (pea_e, mut pea_pos, mut pea) in &mut peas {
        pea_pos.x += pea.speed_pps * dt;
        if pea.kind == PeaKind::Normal {
            let lit = torchwoods.iter().any(|(tp, tpos)| {
                tp.kind == PlantKind::Torchwood
                    && tp.row == pea.row
                    && (pea_pos.x - tpos.x).abs() < 20.0
            });
            if lit {
                pea.kind = PeaKind::Fire;
                pea.damage = FIRE_PEA_DAMAGE;
            }
        }
        let mut best: Option<(Entity, f32)> = None;
        for (z_e, zombie, zpos, dying) in &zombies {
            if dying || zombie.row != pea.row {
                continue;
            }
            let dx = pea_pos.x - zpos.x;
            let dy = pea_pos.y - zpos.y;
            if dx * dx + dy * dy > PEA_HIT_RADIUS * PEA_HIT_RADIUS {
                continue;
            }
            match best {
                Some((_, best_x)) if zpos.x >= best_x => {}
                _ => best = Some((z_e, zpos.x)),
            }
        }
        let mut despawn_pea = false;
        if let Some((target, _)) = best {
            if let Ok((_, mut zombie, _, _)) = zombies.get_mut(target) {
                if zombie.take_damage(pea.damage) {
                    kill_zombie(&mut commands, target);
                } else if pea.kind == PeaKind::Snow {
                    commands.entity(target).insert(Chilled {
                        remaining_ticks: SNOW_PEA_CHILL_TICKS,
                    });
                }
            }
            despawn_pea = true;
            let (hx, hy) = (pea_pos.x, pea_pos.y);
            repame_fx::burst(
                &mut commands,
                hx,
                hy,
                &super::fx::pea_puff(),
                6,
                &mut rand::rng(),
            );
        }
        if despawn_pea || pea_pos.x > BOARD_WIDTH + 40.0 {
            commands.entity(pea_e).try_despawn();
        }
    }
}

pub fn tick_cherry_bombs(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut board: ResMut<Board>,
    mut bombs: Query<(Entity, &Plant, &mut CherryBombFuse)>,
    mut zombies: Query<(Entity, &mut Zombie, &Pos)>,
    mut trauma: ResMut<repame_fx::Trauma>,
    mut flash: ResMut<repame_fx::Flash>,
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
        for (z_e, mut zombie, zpos) in &mut zombies {
            let Some((z_col, z_row)) = logic_to_grid(zpos.x, zpos.y) else {
                continue;
            };
            // Chebyshev <= 1 (3x3). Hits hypnotized too (matches bevy build).
            if z_row.abs_diff(plant.row) <= 1
                && z_col.abs_diff(plant.col) <= 1
                && zombie.take_damage(CHERRY_BOMB_DAMAGE)
            {
                commands.entity(z_e).try_despawn();
            }
        }
        let (bx, by) = grid_center_logic(plant.col, plant.row);
        repame_fx::burst(
            &mut commands,
            bx,
            by,
            &super::fx::explosion(),
            24,
            &mut rand::rng(),
        );
        trauma.add(0.6);
        flash.trigger([1.0, 1.0, 1.0, 0.5], 12);
        board.cells[plant.row][plant.col] = None;
        commands.entity(bomb_e).try_despawn();
    }
}

pub fn tick_potato_mines(
    frame_ticks: Res<FrameTicks>,
    mut mines: Query<(&Plant, &mut PotatoMineState), Without<Zombie>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    for (plant, mut state) in &mut mines {
        if plant.kind != PlantKind::PotatoMine || state.armed {
            continue;
        }
        state.arm_timer -= frame_ticks.0;
        if state.arm_timer <= 0 {
            state.armed = true;
        }
    }
}

pub fn tick_chompers(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut chompers: Query<(&Plant, &Pos, &mut ChomperState), Without<Zombie>>,
    mut zombies: Query<(Entity, &mut Zombie, &Pos), Without<Plant>>,
    mut trauma: ResMut<repame_fx::Trauma>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    for (plant, pos, mut state) in &mut chompers {
        if plant.kind != PlantKind::Chomper {
            continue;
        }
        if state.chewing_timer > 0 {
            state.chewing_timer = (state.chewing_timer - frame_ticks.0).max(0);
            continue;
        }
        let mut victim: Option<Entity> = None;
        for (z_e, zombie, zpos) in &zombies {
            if zombie.row != plant.row {
                continue;
            }
            if zpos.x > pos.x - 10.0 && zpos.x < pos.x + CHOMPER_REACH {
                victim = Some(z_e);
                break;
            }
        }
        if let Some(v_e) = victim
            && let Ok((_, mut zombie, zpos)) = zombies.get_mut(v_e)
        {
            let (hx, hy) = (zpos.x, zpos.y);
            if zombie.take_damage(9999) {
                commands.entity(v_e).try_despawn();
            }
            repame_fx::burst(
                &mut commands,
                hx,
                hy,
                &super::fx::thump(),
                10,
                &mut rand::rng(),
            );
            trauma.add(0.25);
            state.chewing_timer = CHOMPER_CHEW_TICKS;
        }
    }
}

pub fn tick_squash(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut board: ResMut<Board>,
    squashes: Query<(Entity, &Plant, &Pos), Without<Zombie>>,
    mut zombies: Query<(Entity, &mut Zombie, &Pos), Without<Plant>>,
    mut trauma: ResMut<repame_fx::Trauma>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    for (sq_e, plant, pos) in &squashes {
        if plant.kind != PlantKind::Squash {
            continue;
        }
        let mut target: Option<(Entity, f32)> = None;
        for (z_e, zombie, zpos) in &zombies {
            if zombie.hypnotized || zombie.row != plant.row {
                continue;
            }
            let d = (zpos.x - pos.x).abs();
            if d <= SQUASH_TRIGGER_RANGE_PX && target.is_none_or(|(_, bd)| d < bd) {
                target = Some((z_e, d));
            }
        }
        if let Some((z_e, _)) = target
            && let Ok((_, mut zombie, zpos)) = zombies.get_mut(z_e)
        {
            let (hx, hy) = (zpos.x, zpos.y);
            if zombie.take_damage(SQUASH_DAMAGE) {
                commands.entity(z_e).try_despawn();
                board.cells[plant.row][plant.col] = None;
                commands.entity(sq_e).try_despawn();
                repame_fx::burst(
                    &mut commands,
                    hx,
                    hy,
                    &super::fx::thump(),
                    10,
                    &mut rand::rng(),
                );
                trauma.add(0.2);
            }
        }
    }
}

pub fn tick_jalapenos(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut board: ResMut<Board>,
    mut jals: Query<(Entity, &Plant, &mut JalapenoFuse)>,
    mut zombies: Query<(Entity, &mut Zombie), Without<Plant>>,
    mut trauma: ResMut<repame_fx::Trauma>,
    mut flash: ResMut<repame_fx::Flash>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    for (jal_e, plant, mut fuse) in &mut jals {
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
        let (jx, jy) = grid_center_logic(plant.col, plant.row);
        repame_fx::burst(
            &mut commands,
            jx,
            jy,
            &super::fx::explosion(),
            24,
            &mut rand::rng(),
        );
        trauma.add(0.6);
        flash.trigger([1.0, 1.0, 1.0, 0.5], 12);
        board.cells[plant.row][plant.col] = None;
        commands.entity(jal_e).try_despawn();
    }
}

pub fn tick_spikeweeds(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut weeds: Query<(&Plant, &Pos, &mut SpikeweedState), Without<Zombie>>,
    mut zombies: Query<(Entity, &mut Zombie, &Pos), Without<Plant>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    for (plant, pos, mut state) in &mut weeds {
        if plant.kind != PlantKind::Spikeweed {
            continue;
        }
        for (_, rem) in state.cooldowns.iter_mut() {
            *rem -= frame_ticks.0;
        }
        state.cooldowns.retain(|(_, rem)| *rem > 0);
        let mut hits = Vec::new();
        for (z_e, _, zpos) in &zombies {
            if (zpos.x - pos.x).abs() <= 40.0 && !state.cooldowns.iter().any(|(e, _)| *e == z_e) {
                hits.push(z_e);
            }
        }
        for z_e in hits {
            state.cooldowns.push((z_e, SPIKEWEED_HIT_INTERVAL_TICKS));
            if let Ok((_, mut zombie, _)) = zombies.get_mut(z_e) {
                if zombie.take_damage(SPIKEWEED_DAMAGE) {
                    commands.entity(z_e).try_despawn();
                }
            }
        }
    }
}

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
    for (ice_e, plant, mut fuse) in &mut ices {
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
        commands.entity(ice_e).try_despawn();
    }
}

pub fn tick_doom_shrooms(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut board: ResMut<Board>,
    mut dooms: Query<(Entity, &Plant, &mut DoomShroomFuse)>,
    mut zombies: Query<(Entity, &mut Zombie, &Pos), Without<Plant>>,
    mut trauma: ResMut<repame_fx::Trauma>,
    mut flash: ResMut<repame_fx::Flash>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    for (doom_e, plant, mut fuse) in &mut dooms {
        if plant.kind != PlantKind::DoomShroom {
            continue;
        }
        fuse.remaining -= frame_ticks.0;
        if fuse.remaining > 0 {
            continue;
        }
        for (z_e, mut zombie, zpos) in &mut zombies {
            if zombie.hypnotized {
                continue;
            }
            let Some((z_col, z_row)) = logic_to_grid(zpos.x, zpos.y) else {
                continue;
            };
            if z_row.abs_diff(plant.row) <= 2
                && z_col.abs_diff(plant.col) <= 2
                && zombie.take_damage(DOOM_SHROOM_DAMAGE)
            {
                commands.entity(z_e).try_despawn();
            }
        }
        for dr in -2..=2 {
            for dc in -2..=2 {
                let r = plant.row as i32 + dr;
                let c = plant.col as i32 + dc;
                if r < 0 || c < 0 || r >= LAWN_ROWS as i32 || c >= LAWN_COLS as i32 {
                    continue;
                }
                if let Some(p_e) = board.cells[r as usize][c as usize].take() {
                    // Matches bevy build: unconditional (craters in range
                    // are consumed too; tick_craters guards via its check).
                    commands.entity(p_e).try_despawn();
                }
            }
        }
        let (cx, cy) = grid_center_logic(plant.col, plant.row);
        repame_fx::burst(
            &mut commands,
            cx,
            cy,
            &super::fx::explosion(),
            32,
            &mut rand::rng(),
        );
        trauma.add(0.8);
        flash.trigger([1.0, 1.0, 1.0, 0.6], 14);
        let crater_e = commands
            .spawn((
                GameplayCleanup,
                Crater {
                    remaining: DOOM_CRATER_TICKS,
                    row: plant.row,
                    col: plant.col,
                },
                Pos { x: cx, y: cy },
            ))
            .id();
        board.cells[plant.row][plant.col] = Some(crater_e);
        commands.entity(doom_e).try_despawn();
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

pub fn move_and_eat_zombies(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut board: ResMut<Board>,
    mut zombies: Query<
        (
            Entity,
            &mut Zombie,
            &mut Pos,
            Option<&mut Chilled>,
            Option<&mut Frozen>,
            Has<Dying>,
        ),
        Without<Plant>,
    >,
    mut plants: Query<(Entity, &mut Plant, &Pos, Option<&PotatoMineState>), Without<Zombie>>,
    mut trauma: ResMut<repame_fx::Trauma>,
    mut flash: ResMut<repame_fx::Flash>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    let dt = frame_ticks.0 as f32 / TICK_HZ;
    for (z_e, mut zombie, mut zpos, chilled_opt, frozen_opt, dying) in &mut zombies {
        commands.entity(z_e).remove::<Eating>();
        if dying {
            continue;
        }
        let chilled_now = if let Some(mut chilled) = chilled_opt {
            chilled.remaining_ticks = (chilled.remaining_ticks - frame_ticks.0).max(0);
            if chilled.remaining_ticks <= 0 {
                commands.entity(z_e).remove::<Chilled>();
                false
            } else {
                true
            }
        } else {
            false
        };
        if let Some(mut frozen) = frozen_opt {
            frozen.remaining_ticks -= frame_ticks.0;
            if frozen.remaining_ticks <= 0 {
                let chill_after = frozen.chill_after_ticks;
                commands.entity(z_e).remove::<Frozen>();
                if chill_after > 0 {
                    commands.entity(z_e).insert(Chilled {
                        remaining_ticks: chill_after,
                    });
                }
            } else {
                continue;
            }
        }
        if zombie.hypnotized {
            zpos.x += zombie.speed_pps * dt;
            if zpos.x > LAWN_XMIN + LAWN_COLS as f32 * GRID_CELL_W {
                commands.entity(z_e).try_despawn();
            }
            continue;
        }
        let z_x = zpos.x;
        let mut blocking: Option<Entity> = None;
        for (p_e, plant, _, _) in &plants {
            if plant.row != zombie.row || plant.kind == PlantKind::Spikeweed {
                continue;
            }
            let (p_x, _) = grid_center_logic(plant.col, plant.row);
            if p_x <= z_x + 6.0 && z_x - p_x <= ZOMBIE_EAT_REACH {
                blocking = Some(p_e);
                break;
            }
        }
        if let Some(p_e) = blocking {
            commands.entity(z_e).insert(Eating);
            zombie.eat_cooldown_remaining = (zombie.eat_cooldown_remaining - frame_ticks.0).max(0);
            if let Ok((plant_e, mut plant, _, mine_state)) = plants.get_mut(p_e) {
                if plant.kind == PlantKind::PotatoMine {
                    if let Some(state) = mine_state {
                        if state.armed {
                            if zombie.take_damage(POTATO_MINE_DAMAGE) {
                                kill_zombie(&mut commands, z_e);
                            }
                            let (mx, my) = grid_center_logic(plant.col, plant.row);
                            repame_fx::burst(
                                &mut commands,
                                mx,
                                my,
                                &super::fx::explosion(),
                                16,
                                &mut rand::rng(),
                            );
                            trauma.add(0.5);
                            flash.trigger([1.0, 1.0, 1.0, 0.5], 12);
                            board.cells[plant.row][plant.col] = None;
                            commands.entity(plant_e).try_despawn();
                            continue;
                        }
                    }
                }
                if zombie.eat_cooldown_remaining == 0 {
                    if plant.kind == PlantKind::HypnoShroom {
                        zombie.hypnotized = true;
                        zombie.eat_cooldown_remaining = ZOMBIE_EAT_INTERVAL_TICKS;
                        board.cells[plant.row][plant.col] = None;
                        commands.entity(plant_e).try_despawn();
                        continue;
                    }
                    if plant.kind == PlantKind::Garlic {
                        plant.hp -= ZOMBIE_EAT_DAMAGE;
                        zombie.eat_cooldown_remaining = ZOMBIE_EAT_INTERVAL_TICKS;
                        let mut rng = rand::rng();
                        let new_row = if rng.random_bool(0.5) {
                            zombie.row.saturating_sub(1)
                        } else {
                            (zombie.row + 1).min(LAWN_ROWS - 1)
                        };
                        let new_row = if new_row == zombie.row {
                            if zombie.row == 0 { 1 } else { zombie.row - 1 }
                        } else {
                            new_row
                        };
                        zombie.row = new_row;
                        zpos.y = row_center_y(new_row);
                        if plant.hp <= 0 {
                            board.cells[plant.row][plant.col] = None;
                            commands.entity(plant_e).try_despawn();
                        }
                        continue;
                    }
                    plant.hp -= ZOMBIE_EAT_DAMAGE;
                    zombie.eat_cooldown_remaining = if chilled_now {
                        ZOMBIE_EAT_INTERVAL_TICKS * 2
                    } else {
                        ZOMBIE_EAT_INTERVAL_TICKS
                    };
                    if plant.hp <= 0 {
                        board.cells[plant.row][plant.col] = None;
                        commands.entity(plant_e).try_despawn();
                    }
                }
            }
        } else {
            let speed_mult = if chilled_now {
                CHILLED_SPEED_MULTIPLIER
            } else {
                1.0
            };
            zombie.eat_cooldown_remaining = 0;
            zpos.x -= zombie.speed_pps * speed_mult * dt;
        }
    }
}

pub fn resolve_hypno_combat(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut zombies: Query<(Entity, &mut Zombie, &Pos), Without<Dying>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    struct Snap {
        e: Entity,
        hyp: bool,
        row: usize,
        x: f32,
    }
    let snap: Vec<Snap> = zombies
        .iter()
        .map(|(e, z, pos)| Snap {
            e,
            hyp: z.hypnotized,
            row: z.row,
            x: pos.x,
        })
        .collect();
    let mut pairs: Vec<(Entity, Entity)> = Vec::new();
    for h in &snap {
        if !h.hyp {
            continue;
        }
        let mut best: Option<(Entity, f32)> = None;
        for v in &snap {
            if v.hyp || v.row != h.row {
                continue;
            }
            let d = (v.x - h.x).abs();
            if d <= ZOMBIE_EAT_REACH && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((v.e, d));
            }
        }
        if let Some((v_e, _)) = best {
            pairs.push((h.e, v_e));
        }
    }
    for (h_e, v_e) in pairs {
        hypno_bite(&mut zombies, &mut commands, h_e, v_e, frame_ticks.0);
        hypno_bite(&mut zombies, &mut commands, v_e, h_e, frame_ticks.0);
    }
}

fn hypno_bite(
    zombies: &mut Query<(Entity, &mut Zombie, &Pos), Without<Dying>>,
    commands: &mut Commands,
    att_e: Entity,
    vic_e: Entity,
    frame_ticks: i32,
) {
    let Ok((_, mut att, _)) = zombies.get_mut(att_e) else {
        return;
    };
    att.eat_cooldown_remaining = (att.eat_cooldown_remaining - frame_ticks).max(0);
    if att.eat_cooldown_remaining > 0 {
        return;
    }
    att.eat_cooldown_remaining = ZOMBIE_EAT_INTERVAL_TICKS;
    let Ok((_, mut vic, _)) = zombies.get_mut(vic_e) else {
        return;
    };
    if vic.take_damage(ZOMBIE_EAT_DAMAGE) {
        kill_zombie(commands, vic_e);
    }
}

pub fn spawn_mowers(world: &mut World) {
    for row in 0..LAWN_ROWS {
        world.spawn((
            GameplayCleanup,
            LawnMower { row, active: false },
            Pos {
                x: MOWER_X_LOGIC,
                y: row_center_y(row),
            },
        ));
    }
}

pub fn run_mowers(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut mowers: Query<(Entity, &mut LawnMower, &mut Pos), Without<Zombie>>,
    zombies: Query<(Entity, &Zombie, &Pos, Has<Dying>), Without<Plant>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    let dt = frame_ticks.0 as f32 / TICK_HZ;
    for (mower_e, mut mower, mut mpos) in &mut mowers {
        if !mower.active {
            let should_trigger = zombies.iter().any(|(_, z, zpos, dying)| {
                !dying && z.row == mower.row && zpos.x <= mpos.x + MOWER_TRIGGER_SLACK
            });
            if !should_trigger {
                continue;
            }
            mower.active = true;
        }
        mpos.x += MOWER_SPEED_PPS * dt;
        // Kill sweep (corpses excluded via Dying filter).
        for (z_e, z, zpos, dying) in &zombies {
            if dying || z.row != mower.row {
                continue;
            }
            if (zpos.x - mpos.x).abs() <= MOWER_KILL_HALF_WIDTH {
                let (hx, hy) = (zpos.x, zpos.y);
                kill_zombie(&mut commands, z_e);
                repame_fx::burst(
                    &mut commands,
                    hx,
                    hy,
                    &super::fx::thump(),
                    8,
                    &mut rand::rng(),
                );
            }
        }
        if mpos.x > BOARD_WIDTH + 40.0 {
            commands.entity(mower_e).try_despawn();
        }
    }
}

pub fn lose_on_house_reach(
    zombies: Query<(&Zombie, &Pos), Without<Dying>>,
    mowers: Query<&LawnMower>,
    mut flow: ResMut<FlowControl>,
    mut trauma: ResMut<repame_fx::Trauma>,
    mut flash: ResMut<repame_fx::Flash>,
) {
    use super::state::Overlay;
    if flow.overlay != Overlay::None {
        return;
    }
    for (zombie, pos) in &zombies {
        if pos.x <= HOUSE_X_LOGIC {
            let row_protected = mowers.iter().any(|m| m.row == zombie.row);
            if !row_protected {
                flow.overlay = Overlay::GameOver;
                flow.paused = true;
                trauma.add(1.0);
                flash.trigger([0.8, 0.1, 0.1, 0.6], 30);
                break;
            }
        }
    }
}

/// Corpse countdown: fall one-shot duration, then despawn (rig plays the
/// same transition via the `die` trigger; see rigs).
pub fn step_dying(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut corpses: Query<(Entity, &mut Dying)>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    for (e, mut dying) in &mut corpses {
        dying.remaining_ticks -= frame_ticks.0;
        if dying.remaining_ticks <= 0 {
            commands.entity(e).try_despawn();
        }
    }
}
