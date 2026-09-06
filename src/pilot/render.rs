//! Frame producers: sim state -> `FrameInput` sprite batches + tint logic.
//! Ports `game::visuals` (armor strip, chill/freeze, wall-nut cracks,
//! cherry flash, potato/chomper states, hypno pink, fire-pea orange) and
//! lawn tiles from `spawn_level_entities`. Zombies render via live rigs
//! (`rigs`), not batches.

use repame_sim::bevy_ecs::prelude::*;
use repame_sprite::{Camera2d, FrameInput, SpriteInstance};

use super::comps::*;
use super::state::*;
use crate::pilot::constants::*;

fn quad(x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) -> SpriteInstance {
    SpriteInstance {
        center: glam::Vec2::new(x, y),
        rotation: 0.0,
        size: glam::Vec2::new(w, h),
        uv_min: glam::Vec2::ZERO,
        uv_max: glam::Vec2::ONE,
        color,
        page: 0,
    }
}

/// Fixed board camera: 800x600 logic space fills the viewport.
pub fn board_camera() -> Camera2d {
    Camera2d {
        center: glam::Vec2::new(400.0, 300.0),
        units_per_pixel: 1.0,
        zoom: 1.0,
    }
}

pub fn frame_input(world: &mut World, viewport_px: [f32; 2]) -> FrameInput {
    let mut sprites = Vec::new();
    let board = world.resource::<Board>();
    // Lawn checkerboard + porch (stage-tinted, mirrors bevy spawn).
    let (tile_a, tile_b, porch) = match board.stage {
        Stage::Day => (
            [0.385, 0.635, 0.235, 1.0],
            [0.345, 0.585, 0.215, 1.0],
            [0.55, 0.42, 0.26, 1.0],
        ),
        Stage::Night => (
            [0.20, 0.32, 0.24, 1.0],
            [0.16, 0.27, 0.20, 1.0],
            [0.36, 0.31, 0.28, 1.0],
        ),
    };
    for row in 0..LAWN_ROWS {
        for col in 0..LAWN_COLS {
            let shade = if (row + col) % 2 == 0 { tile_a } else { tile_b };
            let (cx, cy) = grid_center_logic(col, row);
            sprites.push(quad(cx, cy, GRID_CELL_W, GRID_CELL_H, shade));
        }
    }
    sprites.push(quad(
        LAWN_XMIN * 0.5,
        LAWN_YMIN + GRID_CELL_H * LAWN_ROWS as f32 * 0.5,
        LAWN_XMIN,
        GRID_CELL_H * LAWN_ROWS as f32,
        porch,
    ));
    // Plants.
    let mut plants = world.query::<(
        &Plant,
        &Pos,
        Option<&CherryBombFuse>,
        Option<&PotatoMineState>,
        Option<&ChomperState>,
    )>();
    for (plant, pos, fuse, potato, chomper) in plants.iter(world) {
        let Some(def) = seed_def_by_kind(plant.kind) else {
            continue;
        };
        let mut color = def.color;
        match plant.kind {
            PlantKind::WallNut => {
                color = wallnut_color(plant.hp);
            }
            PlantKind::CherryBomb => {
                if let Some(f) = fuse {
                    color = cherry_color(f.remaining);
                }
            }
            PlantKind::PotatoMine => {
                if let Some(s) = potato {
                    color = if s.armed {
                        [0.85, 0.30, 0.15, 1.0]
                    } else {
                        [0.60, 0.45, 0.25, 1.0]
                    };
                }
            }
            PlantKind::Chomper => {
                if let Some(s) = chomper {
                    color = if s.chewing_timer > 0 {
                        [0.40, 0.15, 0.50, 1.0]
                    } else {
                        [0.65, 0.25, 0.80, 1.0]
                    };
                }
            }
            _ => {}
        }
        sprites.push(quad(pos.x, pos.y, def.size.0, def.size.1, color));
    }
    // Peas.
    let mut peas = world.query::<(&PeaProjectile, &Pos)>();
    for (pea, pos) in peas.iter(world) {
        let color = match pea.kind {
            PeaKind::Snow => [0.70, 0.90, 1.00, 1.0],
            PeaKind::Fire => [1.00, 0.55, 0.15, 1.0],
            PeaKind::Normal => [0.75, 0.95, 0.25, 1.0],
        };
        sprites.push(quad(pos.x, pos.y, 16.0, 10.0, color));
    }
    // Suns.
    let mut suns = world.query::<(&SunDrop, &Pos)>();
    for (_, pos) in suns.iter(world) {
        sprites.push(quad(pos.x, pos.y, 28.0, 28.0, [1.0, 0.9, 0.2, 1.0]));
    }
    // Mowers.
    let mut mowers = world.query::<(&LawnMower, &Pos)>();
    for (_, pos) in mowers.iter(world) {
        sprites.push(quad(pos.x, pos.y, 36.0, 30.0, [0.75, 0.75, 0.78, 1.0]));
    }
    // Craters.
    let mut craters = world.query::<(&Crater, &Pos)>();
    for (_, pos) in craters.iter(world) {
        sprites.push(quad(pos.x, pos.y, 70.0, 60.0, [0.15, 0.12, 0.10, 1.0]));
    }
    FrameInput {
        cam: board_camera(),
        viewport_px,
        sprites,
        overlay_color: None,
    }
}

fn wallnut_color(hp: i32) -> [f32; 4] {
    let frac = (hp as f32 / WALLNUT_HP as f32).clamp(0.0, 1.0);
    if frac > WALLNUT_CRACK1_FRAC {
        [0.58, 0.39, 0.20, 1.0]
    } else if frac > WALLNUT_CRACK2_FRAC {
        [0.50, 0.33, 0.17, 1.0]
    } else {
        [0.40, 0.26, 0.13, 1.0]
    }
}

fn cherry_color(remaining: i32) -> [f32; 4] {
    let base = [0.78, 0.16, 0.16, 1.0];
    if remaining > CHERRY_FLASH_TICKS {
        return base;
    }
    if (remaining.max(0) / 5) % 2 == 0 {
        [1.0, 0.95, 0.55, 1.0]
    } else {
        base
    }
}
