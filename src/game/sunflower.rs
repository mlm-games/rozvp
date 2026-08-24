//! Sunflower sun production.

use bevy::prelude::*;
use rand::RngExt;

use crate::game::board::Board;
use crate::game::constants::{
    SUNFLOWER_FIRST_SUN_TICKS, SUNFLOWER_SUN_INTERVAL_TICKS, SUNFLOWER_SUN_VALUE, SUN_Z,
};
use crate::game::defs::PlantKind;
use crate::game::systems::{GameplayCleanup, Plant, SunDrop};
use crate::game::tick::{FrameTicks, GameTime};

#[derive(Component, Clone, Copy, Debug)]
pub struct SunProducer {
    pub countdown: i32,
}

impl Default for SunProducer {
    fn default() -> Self {
        Self {
            countdown: SUNFLOWER_FIRST_SUN_TICKS,
        }
    }
}

pub fn tick_sun_producers(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    game_time: Res<GameTime>,
    mut producers: Query<(&Plant, &Transform, &mut SunProducer)>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }

    let mut rng = rand::rng();

    for (plant, tf, mut producer) in &mut producers {
        if plant.kind != PlantKind::Sunflower {
            continue;
        }
        producer.countdown -= frame_ticks.0;
        if producer.countdown > 0 {
            continue;
        }
        producer.countdown += SUNFLOWER_SUN_INTERVAL_TICKS;

        let logic = Board::world_to_logic(tf.translation.truncate());
        let offset = Vec2::new(rng.random_range(-14.0..14.0), 26.0);

        commands.spawn((
            GameplayCleanup,
            SunDrop {
                born_tick: game_time.ticks,
                value: SUNFLOWER_SUN_VALUE,
            },
            Sprite {
                color: Color::srgb(1.0, 0.9, 0.2),
                custom_size: Some(Vec2::splat(28.0)),
                ..default()
            },
            Board::logic_to_world(logic + offset, SUN_Z),
        ));
    }
}
