//! Lawn mowers: one per row, single-use last line of defense.

use bevy::prelude::*;

use crate::game::board::Board;
use crate::game::constants::{
    BOARD_WIDTH, LAWN_ROWS, MOWER_KILL_HALF_WIDTH, MOWER_SPEED_PPS, MOWER_TRIGGER_SLACK,
    MOWER_X_LOGIC, MOWER_Z, TICK_HZ,
};
use crate::game::systems::GameplayCleanup;
use crate::game::tick::FrameTicks;
use crate::game::zombie::Zombie;
use crate::game::zombie_anim::{self, Dying};

#[derive(Component, Clone, Copy, Debug)]
pub struct LawnMower {
    pub row: usize,
    pub active: bool,
}

/// Spawns one mower per lawn row.
pub fn spawn_mowers(mut commands: Commands) {
    for row in 0..LAWN_ROWS {
        let logic_y = Board::row_center_y(row);
        commands.spawn((
            GameplayCleanup,
            LawnMower { row, active: false },
            Sprite {
                color: Color::srgb(0.75, 0.75, 0.78),
                custom_size: Some(Vec2::new(36.0, 30.0)),
                ..default()
            },
            Board::logic_to_world(Vec2::new(MOWER_X_LOGIC, logic_y), MOWER_Z),
        ));
    }
}

/// PvZ behavior seam:
/// - an inactive mower triggers when a zombie in its row reaches it;
/// - on the SAME tick it activates it also moves and can kill the triggerer
///   (no `continue` after activation — this closes the game-over race);
/// - an active mower kills every zombie in its row by contact;
/// - leaving the board to the right despawns it.
pub fn run_mowers(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut mowers: Query<(Entity, &mut LawnMower, &mut Transform), Without<Zombie>>,
    zombies: Query<(Entity, &Zombie, &Transform, Has<Dying>), Without<crate::game::systems::Plant>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    let dt = frame_ticks.0 as f32 / TICK_HZ;

    for (mower_e, mut mower, mut mower_tf) in &mut mowers {
        if !mower.active {
            let should_trigger = zombies.iter().any(|(_, z, z_tf, dying)| {
                !dying
                    && z.row == mower.row
                    && z_tf.translation.x <= mower_tf.translation.x + MOWER_TRIGGER_SLACK
            });
            if !should_trigger {
                continue;
            }
            mower.active = true;
        }

        mower_tf.translation.x += MOWER_SPEED_PPS * dt;

        for (z_e, z, z_tf, dying) in &zombies {
            if !dying
                && z.row == mower.row
                && (z_tf.translation.x - mower_tf.translation.x).abs() <= MOWER_KILL_HALF_WIDTH
            {
                zombie_anim::kill_zombie(&mut commands, z_e);
            }
        }

        if Board::world_to_logic(mower_tf.translation.truncate()).x > BOARD_WIDTH + 40.0 {
            commands.entity(mower_e).try_despawn();
        }
    }
}
