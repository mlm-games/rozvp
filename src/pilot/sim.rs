//! Pilot sim assembly + tick driver. Systems register in the bevy chain
//! order (Economy, Combat, Combat2); the driver advances integer 100 Hz
//! ticks through `Sim::tick()` and enforces pause/overlay blocking.

use std::time::Duration;

use repame_shell::Sim;
use repame_sim::bevy_ecs::prelude::*;

use super::audio::PilotAudio;
use super::combat;
use super::economy;
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

        // Economy chain (mirrors GamePlugin order).
        sim.add_system(economy::reset_click_consumed);
        sim.add_system(economy::tick_seed_recharge);
        sim.add_system(economy::spawn_sky_sun);
        sim.add_system(economy::fall_and_expire_suns);
        sim.add_system(economy::tick_sun_producers);
        sim.add_system(economy::collect_sun_clicks);
        sim.add_system(economy::handle_board_clicks);
        sim.add_system(levels::start_wave_if_needed);
        sim.add_system(levels::spawn_wave_zombies);
        // Combat chain.
        sim.add_system(combat::tick_plants_and_fire);
        sim.add_system(combat::move_peas_and_hit);
        sim.add_system(combat::tick_cherry_bombs);
        sim.add_system(combat::tick_potato_mines);
        sim.add_system(combat::tick_chompers);
        sim.add_system(combat::tick_squash);
        sim.add_system(combat::tick_jalapenos);
        sim.add_system(combat::tick_spikeweeds);
        sim.add_system(combat::tick_ice_shrooms);
        sim.add_system(combat::tick_doom_shrooms);
        sim.add_system(combat::move_and_eat_zombies);
        sim.add_system(combat::run_mowers);
        sim.add_system(combat::lose_on_house_reach);
        sim.add_system(levels::advance_or_complete_level);
        sim.add_system(levels::tick_advice);
        sim.add_system(economy::sync_ui);
        // Combat2 chain.
        sim.add_system(combat::tick_craters);
        sim.add_system(combat::resolve_hypno_combat);
        sim.add_system(combat::step_dying);

        Self {
            sim,
            rigs: RigMap::default(),
            audio: PilotAudio::new(),
            i18n: Localizer::new(),
            accumulator: 0.0,
        }
    }

    /// Advance wall-clock time (seconds) into whole 100 Hz ticks.
    /// Blocked while paused or any overlay is up (mirrors the bevy
    /// run-conditions). Returns ticks run.
    pub fn advance(&mut self, dt_secs: f32) -> i32 {
        let blocked = self.sim.world.resource::<FlowControl>().sim_blocked();
        if blocked {
            self.sim.world.resource_mut::<FrameTicks>().0 = 0;
            return 0;
        }
        self.accumulator += dt_secs * 100.0;
        let mut whole = self.accumulator.floor() as i32;
        self.accumulator -= whole as f32;
        // Clamp spiral-of-death: max 10 ticks per frame at 100 Hz.
        whole = whole.min(10);
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
        self.rigs.hosts.clear();
        self.sim.world.resource_mut::<Board>().reset();
        self.sim.world.resource_mut::<SunStats>().collected_total = 0;
        self.sim
            .world
            .resource_mut::<SeedBankRuntime>()
            .recharge_remaining
            .clear();
        self.sim.world.resource_mut::<ClickConsumedThisFrame>().0 = false;
        *self.sim.world.resource_mut::<AdviceState>() = AdviceState::default();
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
