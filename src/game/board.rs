//! Board resource: grid occupancy + economy, plus logic<->world coordinate helpers.

use bevy::prelude::*;

use crate::game::constants::{
    BOARD_HEIGHT, BOARD_WIDTH, GRID_CELL_H, GRID_CELL_W, LAWN_COLS, LAWN_ROWS, LAWN_XMIN,
    LAWN_YMIN, SKY_SUN_INTERVAL_TICKS, STARTING_SUN,
};

/// Day/night stage for the current level (drives lawn tint + sky sun).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Stage {
    #[default]
    Day,
    Night,
}

#[derive(Resource, Debug)]
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
        self.cells = [[None; LAWN_COLS]; LAWN_ROWS];
        self.sun = STARTING_SUN;
        self.sun_collected_total = 0;
        self.sky_sun_timer = SKY_SUN_INTERVAL_TICKS;
        self.stage = Stage::Day;
    }

    /// Logic-space (top-left origin, y-down) -> grid cell.
    pub fn logic_to_grid(logic_x: f32, logic_y: f32) -> Option<(usize, usize)> {
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

    pub fn grid_center_logic(col: usize, row: usize) -> Vec2 {
        Vec2::new(
            LAWN_XMIN + col as f32 * GRID_CELL_W + GRID_CELL_W * 0.5,
            LAWN_YMIN + row as f32 * GRID_CELL_H + GRID_CELL_H * 0.5,
        )
    }

    pub fn row_center_y(row: usize) -> f32 {
        LAWN_YMIN + row as f32 * GRID_CELL_H + GRID_CELL_H * 0.5
    }

    /// Logic space (top-left origin, y-down) -> Bevy world space (center origin, y-up).
    pub fn logic_to_world(logic: Vec2, z: f32) -> Transform {
        Transform::from_xyz(logic.x - BOARD_WIDTH * 0.5, BOARD_HEIGHT * 0.5 - logic.y, z)
    }

    pub fn world_to_logic(world: Vec2) -> Vec2 {
        Vec2::new(world.x + BOARD_WIDTH * 0.5, BOARD_HEIGHT * 0.5 - world.y)
    }

    pub fn can_plant(&self, col: usize, row: usize) -> bool {
        row < LAWN_ROWS && col < LAWN_COLS && self.cells[row][col].is_none()
    }
}
