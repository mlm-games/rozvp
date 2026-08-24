//! Shooter firing + pea projectiles (normal and snow).

use bevy::prelude::*;

use crate::game::board::Board;
use crate::game::constants::{
    BOARD_WIDTH, FIRE_PEA_DAMAGE, PEA_DAMAGE, PEA_HIT_RADIUS, PEA_SPEED_PPS, PLANT_Z,
    SNOW_PEA_CHILL_TICKS, TICK_HZ,
};
use crate::game::defs::PlantKind;
use crate::game::systems::{GameplayCleanup, Plant};
use crate::game::tick::FrameTicks;
use crate::game::zombie::{Chilled, Zombie};
use crate::game::zombie_anim::Dying;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeaKind {
    Normal,
    Snow,
    /// Ignited by Torchwood: double damage, no chill.
    Fire,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct PeaProjectile {
    pub kind: PeaKind,
    pub row: usize,
    pub damage: i32,
    pub speed_pps: f32,
}

/// Fires only when a zombie is ahead (to the right) in the same row.
/// `Without<>` filters are required: both queries touch `Transform` and Bevy
/// has no way to know Plant/Zombie entities are disjoint without them.
pub fn tick_plants_and_fire(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut plants: Query<(&mut Plant, &Transform), Without<Zombie>>,
    zombies: Query<(&Zombie, &Transform), Without<Plant>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }

    for (mut plant, tf) in &mut plants {
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

        // Threepeater covers its own lane plus the adjacent ones.
        let is_threepeater = plant.kind == PlantKind::Threepeater;
        let has_target_ahead = zombies.iter().any(|(zombie, ztf)| {
            let lane_ok = if is_threepeater {
                (zombie.row as i32 - plant.row as i32).abs() <= 1
            } else {
                zombie.row == plant.row
            };
            lane_ok && ztf.translation.x > tf.translation.x + 12.0
        });
        if !has_target_ahead {
            continue;
        }

        plant.attack_cooldown_remaining = plant.attack_cooldown_max;

        let is_snow = plant.kind == PlantKind::SnowPea;
        let spawn = tf.translation + Vec3::new(26.0, 8.0, 0.0);
        commands.spawn((
            GameplayCleanup,
            PeaProjectile {
                kind: if is_snow { PeaKind::Snow } else { PeaKind::Normal },
                row: plant.row,
                damage: PEA_DAMAGE,
                speed_pps: PEA_SPEED_PPS,
            },
            Sprite {
                color: if is_snow {
                    Color::srgb(0.70, 0.90, 1.00)
                } else {
                    Color::srgb(0.75, 0.95, 0.25)
                },
                custom_size: Some(Vec2::new(16.0, 10.0)),
                ..default()
            },
            Transform::from_xyz(spawn.x, spawn.y, PLANT_Z + 4.0),
        ));

        // Repeater: second normal pea trailing the first.
        if plant.kind == PlantKind::Repeater {
            let back = tf.translation + Vec3::new(8.0, 8.0, 0.0);
            commands.spawn((
                GameplayCleanup,
                PeaProjectile {
                    kind: PeaKind::Normal,
                    row: plant.row,
                    damage: PEA_DAMAGE,
                    speed_pps: PEA_SPEED_PPS,
                },
                Sprite {
                    color: Color::srgb(0.75, 0.95, 0.25),
                    custom_size: Some(Vec2::new(16.0, 10.0)),
                    ..default()
                },
                Transform::from_xyz(back.x, back.y, PLANT_Z + 4.0),
            ));
        }

        // Threepeater: side-lane peas (center lane covered by the main pea).
        if is_threepeater {
            for target_row in [plant.row.wrapping_sub(1), plant.row + 1] {
                if target_row >= crate::game::constants::LAWN_ROWS {
                    continue;
                }
                let row_y =
                    Board::logic_to_world(Vec2::new(0.0, Board::row_center_y(target_row)), 0.0)
                        .translation
                        .y;
                commands.spawn((
                    GameplayCleanup,
                    PeaProjectile {
                        kind: PeaKind::Normal,
                        row: target_row,
                        damage: PEA_DAMAGE,
                        speed_pps: PEA_SPEED_PPS,
                    },
                    Sprite {
                        color: Color::srgb(0.75, 0.95, 0.25),
                        custom_size: Some(Vec2::new(16.0, 10.0)),
                        ..default()
                    },
                    Transform::from_xyz(spawn.x, row_y + 8.0, PLANT_Z + 4.0),
                ));
            }
        }
    }
}

/// Moves peas right; hits the frontmost (lowest-x) zombie within radius.
/// Deterministic regardless of ECS iteration order — matches the original's
/// "pea hits the front zombie it overlaps" behavior.
pub fn move_peas_and_hit(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut peas: Query<(Entity, &mut Transform, &mut PeaProjectile), Without<Zombie>>,
    mut zombies: Query<(Entity, &mut Zombie, &Transform, Has<Dying>), Without<PeaProjectile>>,
    torchwoods: Query<(&Plant, &Transform), (Without<Zombie>, Without<PeaProjectile>)>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    let dt = frame_ticks.0 as f32 / TICK_HZ;

    for (pea_e, mut pea_tf, mut pea) in &mut peas {
        pea_tf.translation.x += pea.speed_pps * dt;

        // Torchwood: normal peas passing through ignite (2x damage, no chill).
        // Snow peas pass through unchanged.
        if pea.kind == PeaKind::Normal {
            let lit = torchwoods.iter().any(|(tp, ttf)| {
                tp.kind == PlantKind::Torchwood
                    && tp.row == pea.row
                    && (pea_tf.translation.x - ttf.translation.x).abs() < 20.0
            });
            if lit {
                pea.kind = PeaKind::Fire;
                pea.damage = FIRE_PEA_DAMAGE;
            }
        }

        // Frontmost zombie in range.
        let mut best: Option<(Entity, f32)> = None;
        for (z_e, zombie, z_tf, dying) in &zombies {
            if dying
                || zombie.row != pea.row
                || pea_tf
                    .translation
                    .truncate()
                    .distance(z_tf.translation.truncate())
                    > PEA_HIT_RADIUS
            {
                continue;
            }
            match best {
                Some((_, best_x)) if z_tf.translation.x >= best_x => {}
                _ => best = Some((z_e, z_tf.translation.x)),
            }
        }

        let mut despawn_pea = false;
        if let Some((target, _)) = best {
            if let Ok((_, mut zombie, _, _)) = zombies.get_mut(target) {
                // Armor first, overflow to body (see Zombie::take_damage).
                if zombie.take_damage(pea.damage) {
                    crate::game::zombie_anim::kill_zombie(&mut commands, target);
                } else if pea.kind == PeaKind::Snow {
                    // Insert or refresh chill duration.
                    commands.entity(target).insert(Chilled {
                        remaining_ticks: SNOW_PEA_CHILL_TICKS,
                    });
                }
            }
            despawn_pea = true;
        }

        let logic = Board::world_to_logic(pea_tf.translation.truncate());
        if despawn_pea || logic.x > BOARD_WIDTH + 40.0 {
            commands.entity(pea_e).try_despawn();
        }
    }
}
