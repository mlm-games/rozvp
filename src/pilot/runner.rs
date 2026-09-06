//! Pilot runner: boots the repose desktop shell around the sim.
//! Each frame: wall-clock dt -> sim ticks -> rig sync -> root view.
//! Boot loads `save.ron` (progress + settings) before the first frame;
//! splash/loading phases run on wall-clock timers mirroring the bevy
//! `SplashTimer` (1.5 s) + `LoadingTimer` (0.5 s). Gamepad input flows
//! through the platform runner (`gamepad` feature); pointer input arrives
//! as view events (see views).

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use std::time::{Duration, Instant};

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use super::audio::Cue;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use super::sim::PilotApp;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use super::state::{Overlay, PilotPhase};
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use super::{save, views};

const SPLASH_SECS: f32 = 1.5;
const LOADING_SECS: f32 = 0.5;

/// Run the pilot shell (splash phase; levels start from the title hub).
/// Desktop only: `repame_shell::run_desktop` has no mobile/web backend,
/// and the bevy entry (`lib::run`) serves those targets.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub fn run() -> anyhow::Result<()> {
    let mut app = PilotApp::new();
    boot_from_save(&mut app);
    let mut last = Instant::now();
    let boot_at = last;
    let mut last_overlay = Overlay::None;
    repame_shell::run_desktop("RoZVP pilot", (1280, 720), move |sched, ctx| {
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32().min(0.25);
        last = now;
        advance_boot_phase(&mut app, now.duration_since(boot_at));
        app.advance(dt);
        poll_overlay_stingers(&mut app, &mut last_overlay);
        app.audio.set_intensity(progress(&app));
        app.audio.update(dt);
        views::root_view(sched, ctx, &mut app)
    })
}

/// Load save (if any) into the UI share, language, and audio channels.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn boot_from_save(app: &mut PilotApp) {
    let data = save::load();
    if let Ok(mut ui) = app.sim.world.resource::<super::state::UiShare>().ui.lock() {
        save::apply_to_ui(&data, &mut ui);
    }
    app.i18n.set_language(&data.settings.language);
    app.audio.apply_volumes(
        data.settings.master_volume,
        data.settings.sfx_volume,
        data.settings.music_volume,
    );
}

/// Splash -> Loading -> Title on wall-clock timers. Only moves forward
/// (never yanks an in-progress game back to title).
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn advance_boot_phase(app: &mut PilotApp, elapsed: Duration) {
    let Ok(mut ui) = app.sim.world.resource::<super::state::UiShare>().ui.lock() else {
        return;
    };
    let secs = elapsed.as_secs_f32();
    ui.phase = match ui.phase {
        PilotPhase::Splash if secs >= SPLASH_SECS + LOADING_SECS => PilotPhase::Title,
        PilotPhase::Splash if secs >= SPLASH_SECS => PilotPhase::Loading,
        PilotPhase::Loading if secs >= SPLASH_SECS + LOADING_SECS => PilotPhase::Title,
        phase => phase,
    };
}

/// One-shot stingers on overlay transitions (win/lose moments).
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn poll_overlay_stingers(app: &mut PilotApp, last_overlay: &mut Overlay) {
    let overlay = app
        .sim
        .world
        .resource::<super::state::FlowControl>()
        .overlay;
    if overlay != *last_overlay {
        match overlay {
            Overlay::LevelComplete | Overlay::Award => app.audio.cue(Cue::Win),
            Overlay::GameOver => app.audio.cue(Cue::Lose),
            _ => {}
        }
        *last_overlay = overlay;
    }
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn progress(app: &PilotApp) -> f32 {
    app.sim
        .world
        .resource::<super::state::UiShare>()
        .ui
        .lock()
        .map(|ui| ui.progress)
        .unwrap_or(0.0)
}

/// Mobile/web fallback: the pilot shell has no runner there.
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
pub fn run() -> anyhow::Result<()> {
    anyhow::bail!("rozvp-repose pilot shell is desktop-only")
}

#[cfg(test)]
mod tests {
    use super::super::state::{PilotPhase, UiShare};
    use super::*;

    fn phase_after(start: PilotPhase, secs: f32) -> PilotPhase {
        let mut app = PilotApp::new();
        if let Ok(mut ui) = app.sim.world.resource::<UiShare>().ui.lock() {
            ui.phase = start;
        }
        advance_boot_phase(&mut app, Duration::from_secs_f32(secs));
        app.sim
            .world
            .resource::<UiShare>()
            .ui
            .lock()
            .map(|ui| ui.phase)
            .unwrap()
    }

    #[test]
    fn splash_holds_then_loading_then_title() {
        assert_eq!(phase_after(PilotPhase::Splash, 0.1), PilotPhase::Splash);
        assert_eq!(phase_after(PilotPhase::Splash, 1.6), PilotPhase::Loading);
        assert_eq!(phase_after(PilotPhase::Splash, 2.1), PilotPhase::Title);
        assert_eq!(phase_after(PilotPhase::Loading, 2.1), PilotPhase::Title);
    }

    #[test]
    fn ingame_never_regresses_to_boot() {
        assert_eq!(phase_after(PilotPhase::InGame, 99.0), PilotPhase::InGame);
        assert_eq!(phase_after(PilotPhase::Title, 99.0), PilotPhase::Title);
    }
}
