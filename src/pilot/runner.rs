//! Platform entries: desktop, web, and android shells around the sim.
//! Same three-entry shape as the renamite/resims apps: each builds a
//! `PilotApp`, boots save/i18n/audio, then pumps wall-clock dt -> sim
//! ticks -> rig sync -> root view every frame. Boot loads `save.ron`
//! (progress + settings) before the first frame; splash/loading phases
//! run on wall-clock timers. Gamepad input flows through the platform
//! runner (`gamepad` feature); pointer input arrives as view events.

use std::time::{Duration, Instant};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use super::audio::Cue;
use super::sim::PilotApp;
use super::state::{Overlay, PilotPhase};
use super::{save, views};

const SPLASH_SECS: f32 = 1.5;
const LOADING_SECS: f32 = 0.5;

/// Desktop entry: 1280x720 window, title hub on boot.
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub fn desktop_main() -> anyhow::Result<()> {
    let mut app = boot_app();
    let mut last = Instant::now();
    let boot_at = last;
    let mut last_overlay = Overlay::None;
    repame_shell::run_desktop("RoZVP", (1280, 720), move |sched, ctx| {
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32().min(0.25);
        last = now;
        pump(&mut app, dt, now.duration_since(boot_at), &mut last_overlay);
        views::root_view(sched, ctx, &mut app)
    })
}

/// Web entry: default canvas, prevent-default on (via the shell).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_start() -> Result<(), JsValue> {
    let mut app = boot_app();
    let boot_at = Instant::now();
    let mut last_overlay = Overlay::None;
    // Web clock: whole 100 Hz ticks from elapsed wall time each frame.
    let mut last_ticks = 0i64;
    repame_shell::run_web(move |sched, ctx| {
        let elapsed = boot_at.elapsed().as_secs_f32();
        let ticks = (elapsed * 100.0) as i64;
        let dt = ((ticks - last_ticks).max(0) as f32 / 100.0).min(0.25);
        last_ticks = ticks;
        pump(&mut app, dt, boot_at.elapsed(), &mut last_overlay);
        views::root_view(sched, ctx, &mut app)
    })
}

/// Android entry: logger + window insets, then the shared shell.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn android_main(android_app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    rlobkit_app_events::insets::set_on_insets(Box::new(|insets| {
        let r = repose_core::locals::WindowInsets {
            top: insets.top,
            bottom: insets.bottom,
            left: insets.left,
            right: insets.right,
            ime_bottom: insets.ime_bottom,
        };
        repose_core::locals::set_window_insets_default(r);
    }));

    let mut app = boot_app();
    let boot_at = Instant::now();
    let mut last = boot_at;
    let mut last_overlay = Overlay::None;
    let _ = repame_shell::run_android(android_app, move |sched, ctx| {
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32().min(0.25);
        last = now;
        pump(&mut app, dt, now.duration_since(boot_at), &mut last_overlay);
        views::root_view(sched, ctx, &mut app)
    });
}

/// Build the app and boot save/i18n/audio before the first frame.
fn boot_app() -> PilotApp {
    let mut app = PilotApp::new();
    boot_from_save(&mut app);
    app
}

/// One frame: boot phases, sim ticks, stingers, music intensity.
fn pump(app: &mut PilotApp, dt_secs: f32, elapsed: Duration, last_overlay: &mut Overlay) {
    advance_boot_phase(app, elapsed);
    app.advance(dt_secs);
    poll_overlay_stingers(app, last_overlay);
    app.audio.set_intensity(progress(app));
    app.audio.update(dt_secs);
}

/// Load save (if any) into the UI share, language, and audio channels.
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

fn progress(app: &PilotApp) -> f32 {
    app.sim
        .world
        .resource::<super::state::UiShare>()
        .ui
        .lock()
        .map(|ui| ui.progress)
        .unwrap_or(0.0)
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
