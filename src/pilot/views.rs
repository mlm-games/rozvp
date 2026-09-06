//! Pilot views: board canvas, HUD, menus, overlays. Presentational only:
//! reads sim + UI share, writes UI share + click queue + flow control.
//!
//! Coordinate convention: 1 logic px == 1 dp. The board canvas is a fixed
//! 800x600 region; pointer positions (physical px, region-relative) map
//! through `px_to_dp` straight onto logic coords.

use std::rc::Rc;

use fluent_bundle::FluentArgs;
use renamite_player_ui::RenamitePlayer;
use repose_canvas::Canvas;
use repose_core::locals::px_to_dp;
use repose_core::{Color, Modifier, Rect, RenderContext, Scheduler, View, request_frame};
use repose_ui::{Box as UiBox, Column, Row, Spacer, Text, TextStyle, ViewExt, ZStack};

use super::render::frame_input;
use super::sim::PilotApp;
use super::state::{Overlay, PilotPhase};
use crate::pilot::constants::{BOARD_HEIGHT, BOARD_WIDTH};

/// Root view: splash, loading, title, or game, plus a standing frame
/// request (continuous game loop; sim time advances in the runner).
pub fn root_view(_sched: &mut Scheduler, ctx: &RenderContext, app: &mut PilotApp) -> View {
    request_frame();
    let phase = app
        .sim
        .world
        .resource::<super::state::UiShare>()
        .ui
        .lock()
        .map(|ui| ui.phase)
        .unwrap_or(PilotPhase::Title);
    match phase {
        PilotPhase::Splash => splash_view(app),
        PilotPhase::Loading => loading_view(app),
        PilotPhase::Title => title_view(app, ctx),
        PilotPhase::InGame => game_view(app, ctx),
    }
}

fn splash_view(app: &mut PilotApp) -> View {
    Column(Modifier::new().padding(48.0).gap(16.0)).child((
        Text(t(app, "app-title")).size(56.0),
        Text(t(app, "tagline")),
    ))
}

fn loading_view(app: &mut PilotApp) -> View {
    Column(Modifier::new().padding(48.0).gap(16.0)).child((
        Text(t(app, "app-title")).size(40.0),
        Text(t(app, "loading")).size(20.0),
    ))
}

/// Lookup a UI string in the current language.
fn t(app: &PilotApp, key: &str) -> String {
    app.i18n.t(key)
}

/// Translated seed display name. Falls back to the internal name when the
/// key itself leaks through (unknown seed or missing catalog entry).
fn t_seed(app: &PilotApp, name: &str) -> String {
    let got = app.i18n.t(&super::comps::seed_key(name));
    if got == super::comps::seed_key(name) {
        name.to_string()
    } else {
        got
    }
}

/// Packet-length seed label: translator short when the current locale
/// provides one, else the first four chars of the translated full name
/// (correct language, occasionally long — translators fix it with a
/// `seed-*-short` key, no code change).
fn short_seed(app: &PilotApp, name: &str) -> String {
    let short_key = format!("{}-short", super::comps::seed_key(name));
    if let Some(s) = app.i18n.t_local(&short_key) {
        return s;
    }
    short_name(&t_seed(app, name))
}

fn title_view(app: &mut PilotApp, _ctx: &RenderContext) -> View {
    let overlay = app
        .sim
        .world
        .resource::<super::state::FlowControl>()
        .overlay;
    match overlay {
        Overlay::SeedChooser => return seed_chooser(app),
        Overlay::Settings => return settings_view(app),
        Overlay::Credits => return credits_view(app),
        _ => {}
    }
    let adventure_label = {
        let ui = app.sim.world.resource::<super::state::UiShare>();
        let ui = ui.ui.lock().unwrap();
        format!("{} {}", t(app, "adventure"), ui.level_name)
    };
    Column(Modifier::new().padding(48.0).gap(16.0)).child((
        Text(t(app, "app-title")).size(40.0),
        Text(t(app, "tagline")),
        hub_button(app, adventure_label, UiAct::OpenAdventure),
        hub_button(app, t(app, "mini-games"), UiAct::OpenAdventure),
        hub_button(app, t(app, "puzzle"), UiAct::OpenAdventure),
        hub_button(app, t(app, "survival"), UiAct::OpenAdventure),
        Row(Modifier::new().gap(8.0)).child((
            hub_button(app, t(app, "settings"), UiAct::OpenSettings),
            hub_button(app, t(app, "help"), UiAct::OpenCredits),
            hub_button(app, t(app, "quit"), UiAct::QuitApp),
        )),
    ))
}

/// Title hub button. Mode buttons all open the seed chooser: the pilot
/// has one level pool (adventure levels), modes are presentational.
fn hub_button(app: &mut PilotApp, label: String, act: UiAct) -> View {
    let app_ptr = app as *mut PilotApp;
    UiBox(
        Modifier::new()
            .background(Color::from_rgba(60, 120, 60, 255))
            .padding(12.0)
            .on_click(move || {
                // SAFETY: invoked synchronously during compose, before any
                // other borrow of the app.
                let app = unsafe { &mut *app_ptr };
                apply_act(app, act.clone());
            }),
    )
    .child(Text(label).size(22.0))
}

fn game_view(app: &mut PilotApp, ctx: &RenderContext) -> View {
    let board = board_layer(app);
    let rigs = rigs_layer(app, ctx);
    let world = ZStack(Modifier::new().size(BOARD_WIDTH, BOARD_HEIGHT)).child((board, rigs));
    let hud = hud_bar(app);
    let overlays = overlay_layer(app);
    Column(Modifier::new().gap(4.0)).child((
        hud,
        ZStack(Modifier::new().size(BOARD_WIDTH, BOARD_HEIGHT)).child((world, overlays)),
    ))
}

/// Lawn + entities painted to canvas from the frame producers.
fn board_layer(app: &mut PilotApp) -> View {
    let input = frame_input(&mut app.sim.world, [BOARD_WIDTH, BOARD_HEIGHT]);
    let sprites = Rc::new(input.sprites);
    let app_ptr = app as *mut PilotApp;
    let modifier = Modifier::new()
        .size(BOARD_WIDTH, BOARD_HEIGHT)
        .on_pointer_down(move |ev: repose_core::input::PointerEvent| {
            let p = ev.position;
            let logic_x = px_to_dp(p.x);
            let logic_y = px_to_dp(p.y);
            // SAFETY: synchronous compose-time dispatch only.
            let app = unsafe { &mut *app_ptr };
            app.sim
                .world
                .resource_mut::<super::state::ClickQueue>()
                .clicks
                .push((logic_x, logic_y));
        });
    Canvas(modifier, move |scope: &mut repose_canvas::DrawScope| {
        for s in sprites.iter() {
            let w = s.size.x;
            let h = s.size.y;
            scope.draw_rect(
                Rect {
                    x: s.center.x - w * 0.5,
                    y: s.center.y - h * 0.5,
                    w,
                    h,
                },
                rgba(s.color),
                0.0,
            );
        }
    })
}

fn rgba(c: [f32; 4]) -> Color {
    Color::from_rgba(
        (c[0].clamp(0.0, 1.0) * 255.0) as u8,
        (c[1].clamp(0.0, 1.0) * 255.0) as u8,
        (c[2].clamp(0.0, 1.0) * 255.0) as u8,
        (c[3].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

/// One live-rig surface per zombie, offset to its logic position.
fn rigs_layer(app: &mut PilotApp, ctx: &RenderContext) -> View {
    use super::comps::{Pos, Zombie};
    // Entity-keyed pass (queries borrow world; hosts live outside it).
    let items: Vec<(f32, f32, repame_actors::PlayerHostRef)> = {
        let mut q = app
            .sim
            .world
            .query::<(repame_sim::bevy_ecs::prelude::Entity, &Zombie, &Pos)>();
        let world = &app.sim.world;
        let mut out = Vec::new();
        for (e, _, pos) in q.iter(world) {
            if let Some(entry) = app.rigs.hosts.get(&e) {
                out.push((pos.x, pos.y, entry.host.clone()));
            }
        }
        out
    };
    let mut views = Vec::new();
    for (x, y, host) in items {
        // 64x80 surface, artboard 256x320 scaled by host.view (0.25).
        let surface = UiBox(Modifier::new().size(64.0, 80.0).offset(
            Some(x - 32.0),
            Some(y - 40.0),
            None,
            None,
        ))
        .child(RenamitePlayer(host, ctx.clone()));
        views.push(surface);
    }
    ZStack(Modifier::new().size(BOARD_WIDTH, BOARD_HEIGHT)).child(views)
}

fn hud_bar(app: &mut PilotApp) -> View {
    let (sun, slots, shovel, progress, flags_done, flags_total, advice_text, advice_visible) = {
        let share = app.sim.world.resource::<super::state::UiShare>();
        let ui = share.ui.lock().unwrap();
        (
            ui.sun,
            ui.seed_bank.clone(),
            ui.shovel_selected,
            ui.progress,
            ui.flags_done,
            ui.flags_total,
            ui.advice.text.clone(),
            ui.advice.visible,
        )
    };
    let mut packets: Vec<View> = Vec::new();
    for (i, slot) in slots.iter().enumerate() {
        let label = format!(
            "{}{} {}",
            if slot.selected { "[x] " } else { "" },
            short_seed(app, &slot.seed_name),
            slot.cost
        );
        let app_ptr = app as *mut PilotApp;
        packets.push(
            UiBox(
                Modifier::new()
                    .background(if slot.affordable {
                        Color::from_rgba(50, 90, 50, 255)
                    } else {
                        Color::from_rgba(60, 60, 60, 255)
                    })
                    .padding(6.0)
                    .on_click(move || {
                        let app = unsafe { &mut *app_ptr };
                        select_seed(app, i);
                    }),
            )
            .child(Text(label).size(14.0)),
        );
    }
    let app_ptr = app as *mut PilotApp;
    let shovel = UiBox(
        Modifier::new()
            .background(if shovel {
                Color::from_rgba(145, 100, 40, 255)
            } else {
                Color::from_rgba(70, 50, 28, 255)
            })
            .padding(6.0)
            .on_click(move || {
                let app = unsafe { &mut *app_ptr };
                toggle_shovel(app);
            }),
    )
    .child(Text(t(app, "shovel")).size(14.0));
    let app_ptr = app as *mut PilotApp;
    let pause = UiBox(
        Modifier::new()
            .background(Color::from_rgba(70, 70, 90, 255))
            .padding(6.0)
            .on_click(move || {
                let app = unsafe { &mut *app_ptr };
                pause_game(app);
            }),
    )
    .child(Text("II".to_string()).size(14.0));
    let sun_label = {
        let mut args = FluentArgs::new();
        args.set("count", sun);
        app.i18n.t_with_args("hud-sun-count", Some(&args))
    };
    let flags_label = {
        let mut args = FluentArgs::new();
        args.set("done", flags_done);
        args.set("total", flags_total);
        app.i18n.t_with_args("hud-flags-count", Some(&args))
    };
    let mut row_children = vec![
        Text(sun_label).size(18.0),
        shovel,
        pause,
        Text(flags_label).size(14.0),
    ];
    row_children.extend(packets);
    let bar = Row(Modifier::new().gap(6.0)).child(row_children);
    // Advice is stored as an FTL key by `tick_advice`; translate here.
    let advice_label = if advice_text.is_empty() {
        String::new()
    } else {
        t(app, &advice_text)
    };
    if advice_visible {
        Column(Modifier::new().gap(2.0)).child((
            bar,
            Text(format!("{}  ({:.0}%)", advice_label, progress * 100.0)).size(14.0),
        ))
    } else {
        let label = {
            let mut args = FluentArgs::new();
            args.set("pct", format!("{:.0}", progress * 100.0));
            app.i18n.t_with_args("hud-progress-pct", Some(&args))
        };
        Column(Modifier::new().gap(2.0)).child((bar, Text(label).size(14.0)))
    }
}

fn short_name(name: &str) -> String {
    name.chars().take(4).collect()
}

fn select_seed(app: &mut PilotApp, idx: usize) {
    use super::audio::Cue;
    app.audio.cue(Cue::UiClick);
    enum Sel {
        Select,
        Poor,
        None,
    }
    let act = {
        let world = &app.sim.world;
        let Ok(mut ui) = world.resource::<super::state::UiShare>().ui.lock() else {
            return;
        };
        let Some(slot) = ui.seed_bank.get(idx).cloned() else {
            return;
        };
        if slot.ready >= 1.0 && slot.affordable {
            for (i, s) in ui.seed_bank.iter_mut().enumerate() {
                s.selected = i == idx;
            }
            ui.shovel_selected = false;
            Sel::Select
        } else if !slot.affordable {
            for s in ui.seed_bank.iter_mut() {
                s.selected = false;
            }
            Sel::Poor
        } else {
            Sel::None
        }
    };
    if matches!(act, Sel::Poor) {
        app.sim
            .world
            .resource_mut::<super::state::FlowControl>()
            .overlay = Overlay::NotEnoughSun;
    }
}

fn toggle_shovel(app: &mut PilotApp) {
    use super::audio::Cue;
    app.audio.cue(Cue::UiClick);
    if let Ok(mut ui) = app.sim.world.resource::<super::state::UiShare>().ui.lock() {
        ui.shovel_selected = !ui.shovel_selected;
        if ui.shovel_selected {
            for s in ui.seed_bank.iter_mut() {
                s.selected = false;
            }
        }
    }
}

fn pause_game(app: &mut PilotApp) {
    use super::audio::Cue;
    app.audio.cue(Cue::UiClick);
    let mut flow = app.sim.world.resource_mut::<super::state::FlowControl>();
    flow.paused = true;
    flow.overlay = Overlay::Pause;
}

fn overlay_layer(app: &mut PilotApp) -> View {
    let overlay = app
        .sim
        .world
        .resource::<super::state::FlowControl>()
        .overlay;
    match overlay {
        Overlay::None => Spacer(),
        Overlay::Pause => dialog(
            app,
            &t(app, "paused"),
            vec![
                (t(app, "resume"), UiAct::Resume),
                (t(app, "restart"), UiAct::Restart),
                (t(app, "settings"), UiAct::OpenSettings),
                (t(app, "quit-to-title"), UiAct::QuitToTitle),
            ],
        ),
        Overlay::GameOver => dialog(
            app,
            &t(app, "zombies-ate-your-brains"),
            vec![
                (t(app, "try-again"), UiAct::Restart),
                (t(app, "quit-to-title"), UiAct::QuitToTitle),
            ],
        ),
        Overlay::LevelComplete => {
            let name = app
                .sim
                .world
                .resource::<super::state::UiShare>()
                .ui
                .lock()
                .map(|ui| ui.level_name.clone())
                .unwrap_or_default();
            dialog(
                app,
                &format!("{} {name}", t(app, "level-complete")),
                vec![(t(app, "continue"), UiAct::LevelOk)],
            )
        }
        Overlay::Award => {
            let seed = app
                .sim
                .world
                .resource::<super::state::UiShare>()
                .ui
                .lock()
                .map(|ui| ui.pending_award_seed.clone())
                .unwrap_or(None);
            let title = match seed {
                Some(s) => {
                    let mut args = FluentArgs::new();
                    args.set("title", t(app, "award-title"));
                    args.set("seed", t_seed(app, &s));
                    app.i18n.t_with_args("award-seed-is", Some(&args))
                }
                None => t(app, "award-title"),
            };
            dialog(app, &title, vec![(t(app, "awesome"), UiAct::AwardOk)])
        }
        Overlay::NotEnoughSun => dialog(
            app,
            &t(app, "not-enough-sun"),
            vec![(t(app, "continue"), UiAct::CloseOverlay)],
        ),
        Overlay::SeedChooser => seed_chooser(app),
        Overlay::Settings => settings_view(app),
        Overlay::Credits => credits_view(app),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VolChannel {
    Master,
    Sfx,
    Music,
}

#[derive(Clone)]
enum UiAct {
    Resume,
    Restart,
    QuitToTitle,
    LevelOk,
    AwardOk,
    CloseOverlay,
    OpenAdventure,
    OpenSettings,
    OpenCredits,
    QuitApp,
    BumpVol(VolChannel, bool),
    SetLanguage(String),
    SaveSettings,
}

fn dialog(app: &mut PilotApp, title: &str, buttons: Vec<(String, UiAct)>) -> View {
    let app_ptr = app as *mut PilotApp;
    // Capture per-button actions via index into a shared vec.
    let acts: Rc<Vec<UiAct>> = Rc::new(buttons.iter().map(|(_, a)| a.clone()).collect());
    let mut children: Vec<View> = vec![Text(title.to_string()).size(24.0)];
    for (i, (label, _)) in buttons.iter().enumerate() {
        let acts = acts.clone();
        let label = label.clone();
        children.push(
            UiBox(
                Modifier::new()
                    .background(Color::from_rgba(60, 90, 60, 255))
                    .padding(8.0)
                    .on_click(move || {
                        let app = unsafe { &mut *app_ptr };
                        apply_act(app, acts[i].clone());
                    }),
            )
            .child(Text(label).size(18.0)),
        );
    }
    UiBox(
        Modifier::new()
            .background(Color::from_rgba(20, 20, 24, 230))
            .padding(24.0),
    )
    .child(Column(Modifier::new().gap(10.0)).child(children))
}

fn apply_act(app: &mut PilotApp, act: UiAct) {
    use super::audio::Cue;
    // Every button press ticks (mirrors bevy UI click feedback).
    app.audio.cue(Cue::UiClick);
    match act {
        UiAct::Resume => {
            let mut flow = app.sim.world.resource_mut::<super::state::FlowControl>();
            flow.overlay = Overlay::None;
            flow.paused = false;
        }
        UiAct::Restart => {
            let level = app
                .sim
                .world
                .resource::<super::state::UiShare>()
                .ui
                .lock()
                .map(|ui| ui.adventure_level)
                .unwrap_or(0);
            app.enter_level(level);
        }
        UiAct::QuitToTitle => {
            {
                let world = &app.sim.world;
                if let Ok(mut ui) = world.resource::<super::state::UiShare>().ui.lock() {
                    ui.phase = PilotPhase::Title;
                }
            }
            let mut flow = app.sim.world.resource_mut::<super::state::FlowControl>();
            flow.overlay = Overlay::None;
            flow.paused = false;
        }
        UiAct::LevelOk => {
            let award = {
                let adv = app
                    .sim
                    .world
                    .resource::<super::state::UiShare>()
                    .ui
                    .lock()
                    .map(|ui| ui.adventure_level)
                    .unwrap_or(0);
                super::levels::award_for_completed_level(adv)
            };
            {
                let world = &app.sim.world;
                if let Ok(mut ui) = world.resource::<super::state::UiShare>().ui.lock() {
                    match award {
                        Some(seed) => {
                            ui.pending_award_seed = Some(seed.to_string());
                        }
                        None => {
                            ui.adventure_level += 1;
                            ui.phase = PilotPhase::Title;
                            super::save::persist_ui(&ui);
                        }
                    }
                }
            }
            let mut flow = app.sim.world.resource_mut::<super::state::FlowControl>();
            match award {
                Some(_) => flow.overlay = Overlay::Award,
                None => {
                    flow.overlay = Overlay::None;
                    flow.paused = false;
                }
            }
            app.audio.cue(Cue::Win);
        }
        UiAct::AwardOk => {
            {
                let world = &app.sim.world;
                if let Ok(mut ui) = world.resource::<super::state::UiShare>().ui.lock() {
                    if let Some(seed) = ui.pending_award_seed.take()
                        && !ui.unlocked_seed_names.contains(&seed)
                    {
                        ui.unlocked_seed_names.push(seed);
                    }
                    ui.adventure_level += 1;
                    ui.phase = PilotPhase::Title;
                    super::save::persist_ui(&ui);
                }
            }
            let mut flow = app.sim.world.resource_mut::<super::state::FlowControl>();
            flow.overlay = Overlay::None;
            flow.paused = false;
        }
        UiAct::CloseOverlay => {
            // Back buttons: paused games return to Pause (matches bevy
            // Esc handling), title screens clear the overlay.
            let paused = app.sim.world.resource::<super::state::FlowControl>().paused;
            let mut flow = app.sim.world.resource_mut::<super::state::FlowControl>();
            flow.overlay = if paused {
                Overlay::Pause
            } else {
                Overlay::None
            };
        }
        UiAct::OpenAdventure => {
            open_chooser(app);
        }
        UiAct::OpenSettings => {
            app.sim
                .world
                .resource_mut::<super::state::FlowControl>()
                .overlay = Overlay::Settings;
        }
        UiAct::OpenCredits => {
            app.sim
                .world
                .resource_mut::<super::state::FlowControl>()
                .overlay = Overlay::Credits;
        }
        UiAct::QuitApp => {
            std::process::exit(0);
        }
        UiAct::BumpVol(channel, up) => {
            let world = &app.sim.world;
            if let Ok(mut ui) = world.resource::<super::state::UiShare>().ui.lock() {
                let slot = match channel {
                    VolChannel::Master => &mut ui.master_vol,
                    VolChannel::Sfx => &mut ui.sfx_vol,
                    VolChannel::Music => &mut ui.music_vol,
                };
                *slot = (*slot + if up { 0.1 } else { -0.1 }).clamp(0.0, 1.0);
                app.audio
                    .apply_volumes(ui.master_vol, ui.sfx_vol, ui.music_vol);
            }
        }
        UiAct::SetLanguage(code) => {
            if app.i18n.set_language(&code)
                && let Ok(mut ui) = app.sim.world.resource::<super::state::UiShare>().ui.lock()
            {
                ui.language = code;
            }
        }
        UiAct::SaveSettings => {
            if let Ok(ui) = app.sim.world.resource::<super::state::UiShare>().ui.lock() {
                super::save::persist_ui(&ui);
            }
            let paused = app.sim.world.resource::<super::state::FlowControl>().paused;
            let mut flow = app.sim.world.resource_mut::<super::state::FlowControl>();
            flow.overlay = if paused {
                Overlay::Pause
            } else {
                Overlay::None
            };
        }
    }
}

/// Settings dialog: three volume rows, language list, Save/Back.
/// Mirrors the bevy `settings_ui` (volumes apply immediately, Save
/// persists to `save.ron`).
fn settings_view(app: &mut PilotApp) -> View {
    let (master, sfx, music, current_lang) = {
        let share = app.sim.world.resource::<super::state::UiShare>();
        let ui = share.ui.lock().unwrap();
        (ui.master_vol, ui.sfx_vol, ui.music_vol, ui.language.clone())
    };
    let mut rows: Vec<View> = vec![Text(t(app, "settings")).size(24.0)];
    for (label, value, channel) in [
        (t(app, "master-volume"), master, VolChannel::Master),
        (t(app, "sfx-volume"), sfx, VolChannel::Sfx),
        (t(app, "music-volume"), music, VolChannel::Music),
    ] {
        let app_ptr = app as *mut PilotApp;
        let down = UiAct::BumpVol(channel, false);
        let app_ptr2 = app as *mut PilotApp;
        let up = UiAct::BumpVol(channel, true);
        rows.push(
            Row(Modifier::new().gap(6.0)).child((
                Text(format!("{label} {:3.0}%", value * 100.0)).size(16.0),
                UiBox(
                    Modifier::new()
                        .background(Color::from_rgba(70, 70, 90, 255))
                        .padding(6.0)
                        .on_click(move || {
                            let app = unsafe { &mut *app_ptr };
                            apply_act(app, down.clone());
                        }),
                )
                .child(Text("-".to_string()).size(16.0)),
                UiBox(
                    Modifier::new()
                        .background(Color::from_rgba(70, 70, 90, 255))
                        .padding(6.0)
                        .on_click(move || {
                            let app = unsafe { &mut *app_ptr2 };
                            apply_act(app, up.clone());
                        }),
                )
                .child(Text("+".to_string()).size(16.0)),
            )),
        );
        let _ = app_ptr;
    }
    rows.push(Text(t(app, "language")).size(18.0));
    for code in app.i18n.available() {
        let app_ptr = app as *mut PilotApp;
        let label = if code == current_lang {
            format!("[x] {code}")
        } else {
            format!("[ ] {code}")
        };
        rows.push(
            UiBox(
                Modifier::new()
                    .background(Color::from_rgba(50, 70, 50, 255))
                    .padding(6.0)
                    .on_click(move || {
                        let app = unsafe { &mut *app_ptr };
                        apply_act(app, UiAct::SetLanguage(code.clone()));
                    }),
            )
            .child(Text(label).size(16.0)),
        );
    }
    let app_ptr = app as *mut PilotApp;
    let save_label = t(app, "save");
    let app_ptr2 = app as *mut PilotApp;
    let back_label = t(app, "back");
    rows.push(
        Row(Modifier::new().gap(8.0)).child((
            UiBox(
                Modifier::new()
                    .background(Color::from_rgba(60, 120, 60, 255))
                    .padding(8.0)
                    .on_click(move || {
                        let app = unsafe { &mut *app_ptr };
                        apply_act(app, UiAct::SaveSettings);
                    }),
            )
            .child(Text(save_label).size(18.0)),
            UiBox(
                Modifier::new()
                    .background(Color::from_rgba(70, 70, 90, 255))
                    .padding(8.0)
                    .on_click(move || {
                        let app = unsafe { &mut *app_ptr2 };
                        apply_act(app, UiAct::CloseOverlay);
                    }),
            )
            .child(Text(back_label).size(18.0)),
        )),
    );
    Column(Modifier::new().gap(6.0)).child(rows)
}

/// Credits dialog. Body copy is hardcoded like the bevy build; only
/// the title/Back button go through i18n.
fn credits_view(app: &mut PilotApp) -> View {
    let app_ptr = app as *mut PilotApp;
    let back_label = t(app, "back");
    Column(Modifier::new().gap(10.0)).child((
        Text(t(app, "help")).size(24.0),
        Text(t(app, "credits-line-1")).size(16.0),
        Text(t(app, "credits-line-2")).size(16.0),
        Text(t(app, "credits-line-3")).size(16.0),
        Text(t(app, "credits-line-4")).size(16.0),
        UiBox(
            Modifier::new()
                .background(Color::from_rgba(70, 70, 90, 255))
                .padding(8.0)
                .on_click(move || {
                    let app = unsafe { &mut *app_ptr };
                    apply_act(app, UiAct::CloseOverlay);
                }),
        )
        .child(Text(back_label).size(18.0)),
    ))
}

fn seed_chooser(app: &mut PilotApp) -> View {
    // Simplified chooser: toggle unlocked seeds (max 10), confirm builds
    // the bank and enters the level. Mirrors ConfirmSeedChooser.
    let (picks, unlocked) = {
        let share = app.sim.world.resource::<super::state::UiShare>();
        let ui = share.ui.lock().unwrap();
        (ui.chooser_picks.clone(), ui.unlocked_seed_names.clone())
    };
    let mut rows: Vec<View> = vec![Text(t(app, "choose-your-seeds")).size(24.0)];
    for name in unlocked {
        let picked = picks.contains(&name);
        let app_ptr = app as *mut PilotApp;
        let label = format!(
            "{} {}",
            if picked { "[x]" } else { "[ ]" },
            t_seed(app, &name)
        );
        let toggle_name = name.clone();
        rows.push(
            UiBox(
                Modifier::new()
                    .background(Color::from_rgba(50, 70, 50, 255))
                    .padding(6.0)
                    .on_click(move || {
                        let app = unsafe { &mut *app_ptr };
                        if let Ok(mut ui) =
                            app.sim.world.resource::<super::state::UiShare>().ui.lock()
                        {
                            if let Some(pos) =
                                ui.chooser_picks.iter().position(|p| *p == toggle_name)
                            {
                                ui.chooser_picks.remove(pos);
                            } else if ui.chooser_picks.len() < 10 {
                                ui.chooser_picks.push(toggle_name.clone());
                            }
                        }
                    }),
            )
            .child(Text(label).size(16.0)),
        );
    }
    let app_ptr = app as *mut PilotApp;
    let rock_label = t(app, "lets-rock");
    let app_ptr2 = app as *mut PilotApp;
    let back_label = t(app, "back");
    rows.push(
        Row(Modifier::new().gap(8.0)).child((
            UiBox(
                Modifier::new()
                    .background(Color::from_rgba(60, 120, 60, 255))
                    .padding(8.0)
                    .on_click(move || {
                        let app = unsafe { &mut *app_ptr };
                        confirm_chooser(app);
                    }),
            )
            .child(Text(rock_label).size(18.0)),
            UiBox(
                Modifier::new()
                    .background(Color::from_rgba(70, 70, 90, 255))
                    .padding(8.0)
                    .on_click(move || {
                        let app = unsafe { &mut *app_ptr2 };
                        apply_act(app, UiAct::CloseOverlay);
                    }),
            )
            .child(Text(back_label).size(18.0)),
        )),
    );
    Column(Modifier::new().gap(6.0)).child(rows)
}

fn confirm_chooser(app: &mut PilotApp) {
    use super::audio::Cue;
    app.audio.cue(Cue::UiClick);
    if let Ok(mut ui) = app.sim.world.resource::<super::state::UiShare>().ui.lock() {
        ui.seed_bank.clear();
        for name in ui.chooser_picks.clone() {
            if !ui.unlocked_seed_names.contains(&name) {
                continue;
            }
            let cost = super::comps::seed_def(&name).map(|d| d.cost).unwrap_or(100);
            ui.seed_bank.push(super::state::SeedSlot {
                seed_name: name,
                cost,
                ready: 1.0,
                affordable: true,
                selected: false,
            });
        }
        ui.sun = super::super::pilot::constants::STARTING_SUN;
        ui.progress = 0.0;
        ui.flags_done = 0;
        ui.shovel_selected = false;
        super::save::persist_ui(&ui);
    }
    {
        let mut flow = app.sim.world.resource_mut::<super::state::FlowControl>();
        flow.overlay = Overlay::None;
        flow.paused = false;
    }
    let level = app
        .sim
        .world
        .resource::<super::state::UiShare>()
        .ui
        .lock()
        .map(|ui| ui.adventure_level)
        .unwrap_or(0);
    app.enter_level(level);
    if let Ok(mut ui) = app.sim.world.resource::<super::state::UiShare>().ui.lock() {
        ui.phase = PilotPhase::InGame;
    }
}

/// Seed chooser entry from title: copies bank-eligible picks then opens.
pub fn open_chooser(app: &mut PilotApp) {
    if let Ok(mut ui) = app.sim.world.resource::<super::state::UiShare>().ui.lock() {
        ui.chooser_picks = ui.seed_bank.iter().map(|s| s.seed_name.clone()).collect();
    }
    app.sim
        .world
        .resource_mut::<super::state::FlowControl>()
        .overlay = Overlay::SeedChooser;
}

#[cfg(test)]
mod tests {
    use super::super::save;
    use super::super::sim::PilotApp;
    use super::super::state::{FlowControl, Overlay, PilotPhase, UiShare};
    use super::*;

    /// Persist-writing tests must not clobber a real `save.ron`:
    /// back the file up, restore (or delete) afterwards.
    fn with_saved_save(f: impl FnOnce()) {
        let path = save::save_path();
        let backup = path.as_ref().and_then(|p| std::fs::read(p).ok());
        f();
        match (path, backup) {
            (Some(p), Some(bytes)) => {
                let _ = std::fs::write(p, bytes);
            }
            (Some(p), None) => {
                let _ = std::fs::remove_file(p);
            }
            _ => {}
        }
    }

    #[test]
    fn award_chain_unlocks_and_advances() {
        with_saved_save(|| {
            let mut app = PilotApp::new();
            {
                let mut flow = app.sim.world.resource_mut::<FlowControl>();
                flow.overlay = Overlay::LevelComplete;
            }
            // Level 0 completes into the Wall-nut award.
            apply_act(&mut app, UiAct::LevelOk);
            {
                let flow = app.sim.world.resource::<FlowControl>();
                assert_eq!(flow.overlay, Overlay::Award);
                let ui = app.sim.world.resource::<UiShare>();
                let ui = ui.ui.lock().unwrap();
                assert_eq!(ui.pending_award_seed.as_deref(), Some("Wall-nut"));
            }
            // Accepting unlocks the seed, advances, persists, returns to title.
            apply_act(&mut app, UiAct::AwardOk);
            {
                let flow = app.sim.world.resource::<FlowControl>();
                assert_eq!(flow.overlay, Overlay::None);
                let ui = app.sim.world.resource::<UiShare>();
                let ui = ui.ui.lock().unwrap();
                assert_eq!(ui.adventure_level, 1);
                assert_eq!(ui.phase, PilotPhase::Title);
                assert!(ui.unlocked_seed_names.contains(&"Wall-nut".to_string()));
            }
            // Persist actually fired: the file carries the new progress.
            let path = save::save_path().expect("save path exists on desktop");
            let data = save::load_from(&path).expect("save written");
            assert_eq!(data.adventure_level, 1);
            assert!(data.unlocked_seed_names.contains(&"Wall-nut".to_string()));
        });
    }

    #[test]
    fn volumes_clamp_without_persist() {
        let mut app = PilotApp::new();
        for _ in 0..20 {
            apply_act(&mut app, UiAct::BumpVol(VolChannel::Master, true));
        }
        for _ in 0..30 {
            apply_act(&mut app, UiAct::BumpVol(VolChannel::Music, false));
        }
        let ui = app.sim.world.resource::<UiShare>();
        let ui = ui.ui.lock().unwrap();
        assert_eq!(ui.master_vol, 1.0);
        assert_eq!(ui.music_vol, 0.0);
    }

    #[test]
    fn seed_shorts_prefer_translator_labels() {
        let mut app = PilotApp::new();
        // English ships `seed-*-short` keys.
        assert_eq!(super::short_seed(&app, "Sunflower"), "Sunf");
        assert_eq!(super::short_seed(&app, "Wall-nut"), "Wall");
        // French has none yet: truncated translated name, not English.
        assert!(app.i18n.set_language("fr"));
        assert_eq!(super::short_seed(&app, "Sunflower"), "Tour");
    }
}
