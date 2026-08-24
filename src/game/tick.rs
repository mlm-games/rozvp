//! Hybrid tick model: original integer countdowns advanced by an accumulator.

use bevy::prelude::*;

use crate::game::constants::TICK_HZ;

/// Absolute simulation time in ticks since level start.
#[derive(Resource, Debug, Default)]
pub struct GameTime {
    pub ticks: i64,
    pub accumulator: f32,
}

/// Whole ticks to advance this frame (0 when paused / blocked / between steps).
#[derive(Resource, Debug, Default)]
pub struct FrameTicks(pub i32);

pub fn advance_game_time(
    time: Res<Time>,
    mut game_time: ResMut<GameTime>,
    mut frame_ticks: ResMut<FrameTicks>,
) {
    game_time.accumulator += time.delta_secs() * TICK_HZ;
    let whole = game_time.accumulator.floor() as i32;
    game_time.accumulator -= whole as f32;
    game_time.ticks += whole as i64;
    frame_ticks.0 = whole;
}
