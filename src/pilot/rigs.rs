//! Live-rig sync: one renamite host per zombie, machine inputs driven
//! from sim markers (`Eating` -> `eating` bool, fresh `Dying` -> `die`
//! trigger), playback ticked per frame. Presentational only: never
//! affects sim outcomes.

use repame_sim::bevy_ecs::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use super::comps::{Dying, Eating, Zombie};
use super::sim::PilotApp;
use super::state::RigEntry;

const ZOMBIE_REN: &str = include_str!("../../assets/anims/zombie.ren");

/// Reconcile rig hosts with sim zombies after a tick batch:
/// drop hosts for despawned entities, spawn hosts for newcomers,
/// push marker state into machine inputs, tick playback.
///
/// The `&Zombie` bound is load-bearing: bare `Has` filters match every
/// entity, which used to mint (and per-frame tick) a full `.ren` host for
/// each pea, sun, plant, and particle — a parse plus a machine eval each.
pub fn sync_rigs(app: &mut PilotApp) {
    let live: Vec<(Entity, bool, bool)> = app
        .sim
        .world
        .query::<(Entity, &Zombie, Has<Eating>, Has<Dying>)>()
        .iter(&app.sim.world)
        .map(|(e, _, eating, dying)| (e, eating, dying))
        .collect();
    let live_set: std::collections::HashSet<Entity> = live.iter().map(|(e, _, _)| *e).collect();
    app.rigs.hosts.retain(|e, _| live_set.contains(e));
    for (e, eating, dying) in live {
        let entry = app.rigs.hosts.entry(e).or_insert_with(|| {
            let host = repame_actors::host_from_str(ZOMBIE_REN).expect("zombie.ren must parse");
            RigEntry {
                host,
                die_fired: false,
            }
        });
        let mut host = entry.host.borrow_mut();
        host.player.set_bool("eating", eating);
        if dying && !entry.die_fired {
            entry.die_fired = true;
            host.player.fire("die");
        }
        host.tick_playback();
    }
}

/// Headless rig smoke test helper: load + tick once.
#[allow(dead_code)]
pub fn smoke_host() -> Rc<RefCell<repame_actors::PlayerHost>> {
    repame_actors::host_from_str(ZOMBIE_REN).expect("zombie.ren must parse")
}
