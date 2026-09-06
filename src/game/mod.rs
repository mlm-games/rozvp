//! RoZVP gameplay: board, sun economy, plants, zombies, mowers — on a 100 Hz tick.

pub mod advice;
pub mod board;
pub mod constants;
pub mod defs;
pub mod level_flow;
pub mod mower;
pub mod projectile;
pub mod specials;
pub mod sunflower;
pub mod systems;
pub mod tick;
pub mod visuals;
pub mod zombie;
pub mod zombie_anim;

use bevy::prelude::*;
use game_utils_bevy::transitions::Transition;

use crate::app::{AppState, Paused};

/// Sim setup + economy + waves. Ordered before [`Combat`].
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct Economy;

/// Combat sim + win/lose + HUD sync. Ordered after [`Economy`].
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct Combat;

/// Late combat: crater lifecycle + hypno brawls + batch-2 visual overrides.
/// Ordered after [`Combat`] so it sees this tick's resolved state.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct Combat2;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<tick::GameTime>()
            .init_resource::<tick::FrameTicks>()
            .init_resource::<board::Board>()
            .init_resource::<systems::SeedBankRuntime>()
            .init_resource::<systems::ClickConsumedThisFrame>()
            .init_resource::<systems::SunStats>()
            .init_resource::<zombie::WaveState>()
            .init_resource::<advice::AdviceState>()
            .add_plugins(zombie_anim::ZombieAnimPlugin)
            .add_systems(
                OnEnter(AppState::InGame),
                (
                    systems::enter_level,
                    // enter_level already resets WaveState; advice + mowers follow.
                    advice::reset_advice,
                    mower::spawn_mowers,
                )
                    .chain(),
            )
            .add_systems(OnExit(AppState::InGame), systems::exit_level)
            // Runs even while paused: consumes UI restart requests from
            // Pause/GameOver overlays, swaps the level and unpauses.
            .add_systems(
                Update,
                systems::apply_restart.run_if(in_state(AppState::InGame)),
            )
            // The gameplay chain exceeds Bevy's 20-system tuple limit, so it is
            // split into chained sets; total order is preserved.
            .configure_sets(
                Update,
                (Economy, Combat, Combat2)
                    .chain()
                    .run_if(in_state(AppState::InGame))
                    .run_if(|p: Res<Paused>| !p.0)
                    .run_if(|t: Res<Transition<AppState>>| !t.block_input),
            )
            .add_systems(
                Update,
                (
                    tick::advance_game_time,
                    systems::reset_click_consumed,
                    systems::tick_seed_recharge,
                    systems::spawn_sky_sun,
                    systems::fall_and_expire_suns,
                    sunflower::tick_sun_producers,
                    systems::collect_sun_clicks,
                    systems::handle_board_clicks,
                    zombie::start_wave_if_needed,
                    zombie::spawn_wave_zombies,
                )
                    .chain()
                    .in_set(Economy),
            )
            .add_systems(
                Update,
                (
                    projectile::tick_plants_and_fire,
                    projectile::move_peas_and_hit,
                    specials::tick_cherry_bombs,
                    specials::tick_potato_mines,
                    specials::tick_chompers,
                    specials::tick_squash,
                    specials::tick_jalapenos,
                    specials::tick_spikeweeds,
                    specials::tick_ice_shrooms,
                    specials::tick_doom_shrooms,
                    zombie::move_and_eat_zombies,
                    mower::run_mowers,
                    zombie::lose_on_house_reach,
                    zombie::advance_or_complete_level,
                    advice::tick_advice,
                    systems::sync_ui,
                    visuals::update_visuals,
                )
                    .chain()
                    .in_set(Combat),
            )
            .add_systems(
                Update,
                (
                    specials::tick_craters,
                    zombie::resolve_hypno_combat,
                    visuals::apply_batch2_visuals,
                    zombie_anim::apply_zombie_sheet,
                    zombie_anim::sync_zombie_anim,
                    zombie_anim::step_zombie_anim,
                )
                    .chain()
                    .in_set(Combat2),
            );
    }
}
