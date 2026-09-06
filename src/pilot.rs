//! Pilot shell: rozvp on repame (sim) + live renamite rigs (actors).
//!
//! Strangler pattern: the bevy path (`run()`) is untouched. Game state
//! lives in a headless [`Sim`] world stepped in integer 100 Hz ticks;
//! `.ren` rigs play live instead of baked sprite atlases; views are
//! pure repose. Audio + save IO are staged after playability.

pub mod audio;
pub mod combat;
pub mod comps;
pub mod constants;
pub mod economy;
pub mod fx;
pub mod i18n;
pub mod levels;
pub mod render;
pub mod rigs;
pub mod runner;
pub mod save;
pub mod sim;
pub mod state;
pub mod views;

pub use sim::PilotApp;

/// Headless demo: enter level 0, step `ticks` 10 ms ticks.
/// Returns `(sun, zombies, peas, mowers)` for smoke assertions.
pub fn run_headless(ticks: u32) -> (i32, usize, usize, usize) {
    use repame_sim::bevy_ecs::prelude::*;
    let mut app = PilotApp::new();
    app.enter_level(0);
    for _ in 0..ticks {
        app.advance(0.01);
    }
    let sun = app.sim.world.resource::<state::Board>().sun;
    let zombies = app
        .sim
        .world
        .query::<&comps::Zombie>()
        .iter(&app.sim.world)
        .count();
    let peas = app
        .sim
        .world
        .query::<&comps::PeaProjectile>()
        .iter(&app.sim.world)
        .count();
    let mowers = app
        .sim
        .world
        .query::<&comps::LawnMower>()
        .iter(&app.sim.world)
        .count();
    (sun, zombies, peas, mowers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_zero_enters_clean() {
        let mut app = PilotApp::new();
        app.enter_level(0);
        let mowers = app
            .sim
            .world
            .query::<&comps::LawnMower>()
            .iter(&app.sim.world)
            .count();
        assert_eq!(mowers, 5);
        assert_eq!(app.sim.world.resource::<state::Board>().sun, 50);
        assert!(
            !app.sim
                .world
                .resource::<state::WaveState>()
                .wave_plans
                .is_empty()
        );
    }

    #[test]
    fn click_collects_sun_across_frames() {
        // Regression: views push clicks during compose, *after* the frame's
        // ticks ran. The queue must carry them into the next frame's ticks —
        // `reset_click_consumed` used to clear the queue first, so sun
        // clicks (and planting clicks) never reached any system.
        let mut app = PilotApp::new();
        app.enter_level(0);
        app.sim.world.spawn((
            comps::GameplayCleanup,
            comps::SunDrop {
                born_tick: 0,
                value: 25,
            },
            comps::Pos { x: 400.0, y: 300.0 },
        ));
        let before = app.sim.world.resource::<state::Board>().sun;
        // Like `board_layer`'s on_pointer_down, after this frame's pump.
        app.sim
            .world
            .resource_mut::<state::ClickQueue>()
            .clicks
            .push((400.0, 300.0));
        // Next frame's pump.
        app.advance(0.01);
        assert_eq!(app.sim.world.resource::<state::Board>().sun, before + 25);
        assert!(
            app.sim
                .world
                .query::<&comps::SunDrop>()
                .iter(&app.sim.world)
                .count()
                == 0
        );
        assert!(
            app.sim
                .world
                .resource::<state::ClickQueue>()
                .clicks
                .is_empty()
        );
    }

    #[test]
    fn near_miss_sun_click_still_collects() {
        // Forgiving grab: a click 40 logic px off the sun's center (outside
        // the 28x28 visual, inside the grab radius) collects it. Falling
        // suns keep moving while the player aims, so edge clicks must not
        // silently drop.
        let mut app = PilotApp::new();
        app.enter_level(0);
        app.sim.world.spawn((
            comps::GameplayCleanup,
            comps::SunDrop {
                born_tick: 0,
                value: 25,
            },
            comps::Pos { x: 400.0, y: 300.0 },
        ));
        let before = app.sim.world.resource::<state::Board>().sun;
        app.sim
            .world
            .resource_mut::<state::ClickQueue>()
            .clicks
            .push((440.0, 300.0));
        app.advance(0.01);
        assert_eq!(app.sim.world.resource::<state::Board>().sun, before + 25);
        assert!(
            app.sim
                .world
                .query::<&comps::SunDrop>()
                .iter(&app.sim.world)
                .count()
                == 0
        );
    }

    #[test]
    fn first_wave_spawns_after_delay() {
        // 18 s start delay + stagger: 2000 ticks must show a zombie.
        let (_, zombies, _, mowers) = run_headless(2000);
        assert!(zombies >= 1, "zombies = {zombies}");
        assert_eq!(mowers, 5);
    }
}
