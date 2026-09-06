//! Game feel wiring: effect presets plus the per-tick fx step.
//! The retired bevy build never spawned juice, so puff/sparkle tuning
//! is new (small, fast, canvas-native). Trauma decay (1.5/s) and the
//! transition halves (0.4 s) mirror the old `ScreenEffectsConfig` and
//! `Transition` numbers.

use repame_fx::{EaseKind, EffectDef, Gradient, Jittered, SpawnerDef};
use repame_sim::bevy_ecs::prelude::*;

use super::state::FrameTicks;

/// Pea impact: 6 green-white puffs, no gravity, high drag.
pub fn pea_puff() -> EffectDef {
    EffectDef {
        spawner: SpawnerDef {
            rate_per_sec: 0.0,
            max_alive: 32,
        },
        speed_pps: Jittered {
            base: 70.0,
            range: 30.0,
        },
        lifetime_ticks: Jittered {
            base: 25.0,
            range: 8.0,
        },
        size_px: Jittered {
            base: 6.0,
            range: 2.0,
        },
        gravity_pps2: 0.0,
        drag_per_sec: 3.0,
        gradient: Gradient::fade_out([0.75, 0.95, 0.25, 1.0]),
        ease: EaseKind::QuadOut,
    }
}

/// Explosions: 24 orange-red sparks with gravity and drag.
pub fn explosion() -> EffectDef {
    EffectDef {
        spawner: SpawnerDef {
            rate_per_sec: 0.0,
            max_alive: 96,
        },
        speed_pps: Jittered {
            base: 160.0,
            range: 80.0,
        },
        lifetime_ticks: Jittered {
            base: 45.0,
            range: 15.0,
        },
        size_px: Jittered {
            base: 10.0,
            range: 4.0,
        },
        gravity_pps2: 160.0,
        drag_per_sec: 1.5,
        gradient: Gradient {
            keys: vec![
                (0.0, [1.0, 0.85, 0.30, 1.0]),
                (0.5, [0.95, 0.45, 0.12, 1.0]),
                (1.0, [0.45, 0.12, 0.08, 0.0]),
            ],
        },
        ease: EaseKind::CubicOut,
    }
}

/// Sun collect: 8 gold sparkles drifting up (negative gravity).
pub fn sparkle() -> EffectDef {
    EffectDef {
        spawner: SpawnerDef {
            rate_per_sec: 0.0,
            max_alive: 32,
        },
        speed_pps: Jittered {
            base: 50.0,
            range: 20.0,
        },
        lifetime_ticks: Jittered {
            base: 35.0,
            range: 10.0,
        },
        size_px: Jittered {
            base: 5.0,
            range: 2.0,
        },
        gravity_pps2: -60.0,
        drag_per_sec: 2.0,
        gradient: Gradient::fade_out([1.0, 0.90, 0.25, 1.0]),
        ease: EaseKind::QuadOut,
    }
}

/// Chomp/mower thump: 10 brown dust motes, heavy drag.
pub fn thump() -> EffectDef {
    EffectDef {
        spawner: SpawnerDef {
            rate_per_sec: 0.0,
            max_alive: 32,
        },
        speed_pps: Jittered {
            base: 90.0,
            range: 40.0,
        },
        lifetime_ticks: Jittered {
            base: 30.0,
            range: 10.0,
        },
        size_px: Jittered {
            base: 8.0,
            range: 3.0,
        },
        gravity_pps2: 60.0,
        drag_per_sec: 4.0,
        gradient: Gradient::fade_out([0.55, 0.42, 0.28, 1.0]),
        ease: EaseKind::QuadOut,
    }
}

/// Per-tick fx: particles, floaters, trauma decay, flash, transition.
/// Runs on sim time (frozen while paused); the transition is stepped
/// with real ticks separately (see `sim.rs`), so it can unblock.
pub fn step_fx(
    mut commands: Commands,
    frame: Res<FrameTicks>,
    mut particles: Query<(Entity, &mut repame_fx::Particle)>,
    mut numbers: Query<(Entity, &mut repame_fx::DamageNumber)>,
    mut trauma: ResMut<repame_fx::Trauma>,
    mut flash: ResMut<repame_fx::Flash>,
) {
    repame_fx::step_particles(&mut commands, &mut particles, frame.0);
    repame_fx::step_numbers(&mut commands, &mut numbers, frame.0);
    trauma.decay(frame.0);
    flash.tick(frame.0);
}

#[cfg(test)]
mod tests {
    use super::super::comps::{CherryBombFuse, GameplayCleanup, Plant, PlantKind, Pos, SunDrop};
    use super::super::render::frame_input;
    use super::super::sim::PilotApp;
    use super::super::state::{Board, ClickQueue, GameTime};

    #[test]
    fn transition_blocks_ticks_then_releases() {
        let mut app = PilotApp::new();
        app.enter_level(0);
        app.sim
            .world
            .resource_mut::<repame_fx::TransitionFx>()
            .begin();
        let before = app.sim.world.resource::<GameTime>().ticks;
        assert_eq!(app.advance(0.5), 0);
        assert_eq!(app.sim.world.resource::<GameTime>().ticks, before);
        // Both 0.4 s halves at max 10 ticks per advance call.
        for _ in 0..9 {
            app.advance(0.1);
        }
        assert!(app.advance(0.1) > 0);
        assert!(app.sim.world.resource::<GameTime>().ticks > before);
    }

    #[test]
    fn cherry_detonation_spawns_juice() {
        let mut app = PilotApp::new();
        app.enter_level(0);
        app.sim.world.spawn((
            GameplayCleanup,
            Plant {
                kind: PlantKind::CherryBomb,
                row: 2,
                col: 3,
                hp: 300,
                attack_cooldown_remaining: 0,
                attack_cooldown_max: 0,
            },
            Pos { x: 0.0, y: 0.0 },
            CherryBombFuse { remaining: 1 },
        ));
        app.advance(0.02);
        let particles = app
            .sim
            .world
            .query::<&repame_fx::Particle>()
            .iter(&app.sim.world)
            .count();
        assert!(particles > 0, "detonation bursts particles");
        assert!(
            app.sim.world.resource::<repame_fx::Trauma>().amount > 0.0,
            "detonation adds trauma"
        );
        assert!(
            app.sim.world.resource::<repame_fx::Flash>().active(),
            "detonation flashes"
        );
        // Trauma reaches the camera (rendering only).
        let input = frame_input(&mut app.sim.world, [800.0, 600.0]);
        assert!(
            (input.cam.center.x - 400.0).abs() > 1e-6 || (input.cam.center.y - 300.0).abs() > 1e-6,
            "shake offsets the camera"
        );
    }

    #[test]
    fn sun_collect_sparkles_and_floats() {
        use super::super::state::SunStats;
        let mut app = PilotApp::new();
        app.enter_level(0);
        app.sim.world.spawn((
            GameplayCleanup,
            SunDrop {
                born_tick: 0,
                value: 25,
            },
            Pos { x: 200.0, y: 200.0 },
        ));
        app.sim
            .world
            .resource_mut::<ClickQueue>()
            .clicks
            .push((200.0, 200.0));
        app.advance(0.02);
        assert_eq!(app.sim.world.resource::<Board>().sun, 75);
        let numbers = app
            .sim
            .world
            .query::<&repame_fx::DamageNumber>()
            .iter(&app.sim.world)
            .count();
        assert_eq!(numbers, 1);
        let _ = app.sim.world.resource::<SunStats>();
    }
}
