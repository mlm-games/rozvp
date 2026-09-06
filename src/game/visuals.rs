//! Presentational-only visual states: armor strip, chill tint, wall-nut
//! cracks, cherry fuse flash. Writes `Sprite.color` exclusively — no gameplay.

use bevy::prelude::*;

use crate::game::constants::{
    CHERRY_FLASH_TICKS, WALLNUT_CRACK1_FRAC, WALLNUT_CRACK2_FRAC, WALLNUT_HP,
};
use crate::game::defs::PlantKind;
use crate::game::specials::{CherryBombFuse, ChomperState, PotatoMineState};
use crate::game::systems::Plant;
use crate::game::zombie::{Chilled, Frozen, Zombie, ZombieKind};

/// Runs last in the gameplay chain so colors reflect this tick's state.
pub fn update_visuals(
    mut zombies: Query<(&Zombie, Option<&Chilled>, Option<&Frozen>, &mut Sprite), Without<Plant>>,
    mut plants: Query<
        (
            &Plant,
            Option<&CherryBombFuse>,
            Option<&PotatoMineState>,
            Option<&ChomperState>,
            &mut Sprite,
        ),
        Without<Zombie>,
    >,
) {
    // Zombies: armor-strip feedback + chill/freeze tints.
    for (zombie, chilled, frozen, mut sprite) in &mut zombies {
        let (r, g, b) = zombie_base_rgb(zombie);

        sprite.color = if frozen.is_some() {
            // Frozen solid: near-white ice.
            tint_rgb((r, g, b), (0.85, 0.95, 1.00), 0.75)
        } else if chilled.is_some() {
            tint_rgb((r, g, b), (0.55, 0.80, 1.00), 0.45)
        } else {
            Color::srgb(r, g, b)
        };
    }

    // Plants: wall-nut cracks + cherry flash + potato/chomper states.
    for (plant, fuse, potato, chomper, mut sprite) in &mut plants {
        match plant.kind {
            PlantKind::WallNut => {
                sprite.color = wallnut_color(plant.hp);
            }
            PlantKind::CherryBomb => {
                if let Some(fuse) = fuse {
                    sprite.color = cherry_color(fuse.remaining);
                }
            }
            PlantKind::PotatoMine => {
                if let Some(state) = potato {
                    sprite.color = if state.armed {
                        Color::srgb(0.85, 0.30, 0.15)
                    } else {
                        Color::srgb(0.60, 0.45, 0.25)
                    };
                }
            }
            PlantKind::Chomper => {
                if let Some(state) = chomper {
                    sprite.color = if state.chewing_timer > 0 {
                        Color::srgb(0.40, 0.15, 0.50)
                    } else {
                        Color::srgb(0.65, 0.25, 0.80)
                    };
                }
            }
            _ => {}
        }
    }
}

fn zombie_base_rgb(z: &Zombie) -> (f32, f32, f32) {
    // Once armor is gone, show bare-body green regardless of cone/bucket variant.
    if z.armor_hp <= 0 {
        return (0.42, 0.62, 0.36);
    }

    match z.kind {
        ZombieKind::Normal => (0.42, 0.62, 0.36),
        ZombieKind::Flag => (0.80, 0.20, 0.20),
        ZombieKind::Conehead => (0.90, 0.50, 0.10),
        ZombieKind::Buckethead => (0.60, 0.60, 0.70),
    }
}

fn tint_rgb(base: (f32, f32, f32), target: (f32, f32, f32), t: f32) -> Color {
    let (r1, g1, b1) = base;
    let (r2, g2, b2) = target;
    Color::srgb(r1 + (r2 - r1) * t, g1 + (g2 - g1) * t, b1 + (b2 - b1) * t)
}

fn wallnut_color(hp: i32) -> Color {
    let frac = (hp as f32 / WALLNUT_HP as f32).clamp(0.0, 1.0);

    if frac > WALLNUT_CRACK1_FRAC {
        Color::srgb(0.58, 0.39, 0.20)
    } else if frac > WALLNUT_CRACK2_FRAC {
        Color::srgb(0.50, 0.33, 0.17)
    } else {
        Color::srgb(0.40, 0.26, 0.13)
    }
}

fn cherry_color(remaining: i32) -> Color {
    let base = Color::srgb(0.78, 0.16, 0.16);

    if remaining > CHERRY_FLASH_TICKS {
        return base;
    }

    // Blink driven purely by remaining fuse: 5-tick cadence in the final window.
    let phase = (remaining.max(0) / 5) % 2;
    if phase == 0 {
        Color::srgb(1.0, 0.95, 0.55)
    } else {
        base
    }
}

/// Isolated override pass for batch-2 states. Runs in `Combat2`, i.e. after
/// `update_visuals`, so hypno-pink and fire-pea orange win over base colors.
pub fn apply_batch2_visuals(
    mut zombies: Query<
        (&Zombie, &mut Sprite),
        (
            Without<Plant>,
            Without<crate::game::projectile::PeaProjectile>,
        ),
    >,
    mut peas: Query<
        (&crate::game::projectile::PeaProjectile, &mut Sprite),
        (Without<Plant>, Without<Zombie>),
    >,
) {
    // Hypnotized: pink ally tint.
    for (z, mut sprite) in &mut zombies {
        if z.hypnotized {
            sprite.color = Color::srgb(0.90, 0.55, 0.75);
        }
    }
    // Ignited peas: orange.
    for (pea, mut sprite) in &mut peas {
        if pea.kind == crate::game::projectile::PeaKind::Fire {
            sprite.color = Color::srgb(1.00, 0.55, 0.15);
        }
    }
}
