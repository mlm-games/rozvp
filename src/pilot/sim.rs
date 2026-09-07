//! Pilot sim assembly + tick driver. Systems register in the bevy chain
//! order (Economy, Combat, Combat2); the driver advances integer 100 Hz
//! ticks through `Sim::tick()` and enforces pause/overlay blocking.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use repame_shell::Sim;
use repame_sim::bevy_ecs::prelude::*;
use repame_sim::bevy_ecs::schedule::IntoScheduleConfigs;

use super::audio::PilotAudio;
use super::combat;
use super::economy;
use super::fx;
use super::i18n::Localizer;
use super::levels;
use super::state::{
    AdviceState, Board, ClickConsumedThisFrame, ClickQueue, FlowControl, FrameTicks, GameTime,
    RigMap, SeedBankRuntime, SunStats, UiShare, WaveState,
};

/// Everything the pilot owns: headless sim + presentational rig hosts,
/// audio engine, and the string catalog. Audio/i18n live next to the
/// `Sim` (not in the world): views drive them, systems stay pure.
pub struct PilotApp {
    pub sim: Sim,
    pub rigs: RigMap,
    pub audio: PilotAudio,
    pub i18n: Localizer,
    accumulator: f32,
    /// Board frame geometry for the current window: dp contain-fit plus
    /// look-point shift. Written by `Viewport2d` at paint time (it alone
    /// owns the px/dp bridge), read at compose time for rig layout.
    /// Lags a resize by at most one frame.
    pub board_fit: Rc<Cell<repame_sprite::FrameGeom>>,
}

impl PilotApp {
    pub fn new() -> Self {
        let mut sim = Sim::new(Duration::from_secs_f64(0.01));
        sim.world.init_resource::<GameTime>();
        sim.world.init_resource::<FrameTicks>();
        sim.world.init_resource::<Board>();
        sim.world.init_resource::<SeedBankRuntime>();
        sim.world.init_resource::<ClickConsumedThisFrame>();
        sim.world.init_resource::<SunStats>();
        sim.world.init_resource::<WaveState>();
        sim.world.init_resource::<AdviceState>();
        sim.world.init_resource::<ClickQueue>();
        sim.world.init_resource::<FlowControl>();
        sim.world.init_resource::<UiShare>();
        repame_fx::init_resources(&mut sim.world);

        // One chained schedule: a bare `Schedule` does not preserve
        // insertion order for conflicting systems (probed: planting ran
        // before sun pickup), so the bevy chain order (Economy, Combat,
        // Combat2) is enforced with nested `.chain()` groups (bevy tuples
        // cap at 20 systems).
        sim.add_chained_systems(
            (
                (
                    // Economy chain (mirrors GamePlugin order).
                    economy::reset_click_consumed,
                    economy::tick_seed_recharge,
                    economy::spawn_sky_sun,
                    economy::fall_and_expire_suns,
                    economy::tick_sun_producers,
                    economy::collect_sun_clicks,
                    economy::handle_board_clicks,
                    levels::start_wave_if_needed,
                    levels::spawn_wave_zombies,
                )
                    .chain(),
                (
                    // Combat chain.
                    combat::tick_plants_and_fire,
                    combat::move_peas_and_hit,
                    combat::tick_cherry_bombs,
                    combat::tick_potato_mines,
                    combat::tick_chompers,
                    combat::tick_squash,
                    combat::tick_jalapenos,
                    combat::tick_spikeweeds,
                    combat::tick_ice_shrooms,
                    combat::tick_doom_shrooms,
                    combat::move_and_eat_zombies,
                    combat::run_mowers,
                    combat::lose_on_house_reach,
                    levels::advance_or_complete_level,
                    levels::tick_advice,
                    economy::sync_ui,
                )
                    .chain(),
                (
                    // Combat2 chain.
                    combat::tick_craters,
                    combat::resolve_hypno_combat,
                    combat::step_dying,
                    // Juice runs on sim time (frozen while paused).
                    fx::step_fx,
                )
                    .chain(),
            )
                .chain(),
        );

        Self {
            sim,
            rigs: RigMap::default(),
            audio: PilotAudio::new(),
            i18n: Localizer::new(),
            accumulator: 0.0,
            board_fit: Rc::new(Cell::new(repame_sprite::FrameGeom::default())),
        }
    }

    /// Advance wall-clock time (seconds) into whole 100 Hz ticks.
    /// Blocked while paused, overlayed, or mid-transition (mirrors the
    /// bevy run-conditions plus transition input blocking). The
    /// transition itself steps on real ticks so it can always unblock.
    /// Returns ticks run.
    pub fn advance(&mut self, dt_secs: f32) -> i32 {
        self.accumulator += dt_secs * 100.0;
        let mut whole = self.accumulator.floor() as i32;
        self.accumulator -= whole as f32;
        // Clamp spiral-of-death: max 10 ticks per frame at 100 Hz.
        whole = whole.min(10);
        self.sim
            .world
            .resource_mut::<repame_fx::TransitionFx>()
            .step(whole);
        let blocked = self.sim.world.resource::<FlowControl>().sim_blocked()
            || self
                .sim
                .world
                .resource::<repame_fx::TransitionFx>()
                .blocking();
        if blocked {
            self.sim.world.resource_mut::<FrameTicks>().0 = 0;
            return 0;
        }
        for _ in 0..whole {
            {
                let mut time = self.sim.world.resource_mut::<GameTime>();
                time.ticks += 1;
                self.sim.world.resource_mut::<FrameTicks>().0 = 1;
            }
            self.sim.tick();
        }
        // Rig + presentational sync happens in rigs/views after this.
        crate::pilot::rigs::sync_rigs(self);
        whole
    }

    /// Start (or restart) a level: teardown, reset, mowers, waves, UI.
    pub fn enter_level(&mut self, adventure_level: u32) {
        // Despawn all level entities.
        let owned: Vec<Entity> = self
            .sim
            .world
            .query_filtered::<Entity, With<super::comps::GameplayCleanup>>()
            .iter(&self.sim.world)
            .collect();
        for e in owned {
            self.sim.world.despawn(e);
        }
        // Juice owns no cleanup tags: clear fx entities explicitly.
        let fx_left: Vec<Entity> = {
            let mut q = self.sim.world.query_filtered::<Entity, Or<(
                With<repame_fx::Particle>,
                With<repame_fx::Spawner>,
                With<repame_fx::DamageNumber>,
            )>>();
            q.iter(&self.sim.world).collect()
        };
        for e in fx_left {
            self.sim.world.despawn(e);
        }
        self.rigs.hosts.clear();
        self.sim.world.resource_mut::<Board>().reset();
        self.sim.world.resource_mut::<SunStats>().collected_total = 0;
        self.sim
            .world
            .resource_mut::<SeedBankRuntime>()
            .recharge_remaining
            .clear();
        self.sim.world.resource_mut::<ClickConsumedThisFrame>().0 = false;
        self.sim.world.resource_mut::<ClickQueue>().clicks.clear();
        *self.sim.world.resource_mut::<AdviceState>() = AdviceState::default();
        *self.sim.world.resource_mut::<repame_fx::Trauma>() = repame_fx::Trauma::new();
        *self.sim.world.resource_mut::<repame_fx::Flash>() = repame_fx::Flash::default();
        *self.sim.world.resource_mut::<repame_fx::TransitionFx>() = repame_fx::TransitionFx::new();
        {
            let mut flow = self.sim.world.resource_mut::<FlowControl>();
            flow.paused = false;
            flow.overlay = super::state::Overlay::None;
            flow.pending_restart = false;
        }
        combat::spawn_mowers(&mut self.sim.world);
        levels::load_level(&mut self.sim.world, adventure_level);
        self.accumulator = 0.0;
    }
}

impl Default for PilotApp {
    fn default() -> Self {
        Self::new()
    }
}
