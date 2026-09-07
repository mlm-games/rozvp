//! Pilot views: board canvas, HUD, menus, overlays. Presentational only:
//! reads sim + UI share, writes UI share + click queue + flow control.
//!
//! Coordinate convention: 1 logic px == 1 dp. Unit discipline lives in
//! `repame-sprite` (`Viewport2d` owns the px/dp bridge); game code works
//! purely in logic units and never touches physical px.
//!
//! Chrome styling mirrors the previous repose-bevy UI (`menus/mod.rs`):
//! wood panels, parchment seed packets, sun badge, advice banner, and
//! centered scrim modals — adapted to the pilot's `&mut PilotApp` closure
//! style (raw-pointer `apply_act` dispatches).

use std::time::Duration;

use fluent_bundle::FluentArgs;
use repame_sprite::{ActorFrame, PickEvent, Viewport2d};
use repose_canvas::Canvas;
use repose_core::input::{KeyEvent, KeyEventType};
use repose_core::prelude::{AlignItems, AnimationSpec, Easing, JustifyContent, Modifier};
use repose_core::shortcuts::KeyChord;
use repose_core::{
    Color, CursorIcon, FocusRequester, FontWeight, Modifier as CoreModifier, Rect, RenderContext,
    Scheduler, TextAlign, View, remember, request_frame,
};
use repose_ui::anim_ext::{
    AnimatedVisibility, AnimatedVisibilityConfig, EnterTransition, ExitTransition,
};
use repose_ui::{Box as UiBox, Center, Column, Grid, Row, Text, TextStyle, ViewExt, ZStack};

use super::render::frame_input;
use super::sim::PilotApp;
use super::state::{Overlay, PilotPhase};
use crate::pilot::constants::{BOARD_HEIGHT, BOARD_WIDTH};

// Layout constants (800x600 design space, mirrors old menus/mod.rs).
const SEED_PACKET_W: f32 = 50.0;
const SEED_PACKET_H: f32 = 70.0;
const SUN_BOX_W: f32 = 72.0;
const SEED_BANK_X: f32 = 88.0;
const SEED_BANK_Y: f32 = 8.0;
const SHOVEL_X: f32 = 622.0;

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
    let view = match phase {
        PilotPhase::Splash => splash_view(app),
        PilotPhase::Loading => loading_view(app),
        PilotPhase::Title => title_view(app, ctx),
        PilotPhase::InGame => game_view(app, ctx),
    };
    // Focusable root so hardware keys reach the action feed (same shape
    // as the resims app root). Key events feed `ActionState`; the sim
    // consumes edges per tick, the runner per frame (pause toggle).
    let app_ptr = app as *mut PilotApp;
    let focus = remember(FocusRequester::new);
    let fr_positioned = (*focus).clone();
    ZStack(
        Modifier::new()
            .fill_max_size()
            .focusable(true)
            .focus_requester((*focus).clone())
            .on_globally_positioned(move |_| {
                fr_positioned.request_focus();
            })
            .on_key_event(move |ke: KeyEvent| {
                if ke.is_repeat {
                    return false;
                }
                let down = matches!(ke.event_type, KeyEventType::Down);
                // SAFETY: synchronous compose-time dispatch only.
                let app = unsafe { &mut *app_ptr };
                app.sim
                    .world
                    .resource_mut::<repame_input::ActionState<super::input::PilotAction>>()
                    .key(&KeyChord::new(ke.key.clone(), ke.modifiers), down);
                false
            }),
    )
    .child(view)
}

fn col(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}

fn cola(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba(r, g, b, a)
}

fn spacer(h: f32) -> View {
    UiBox(CoreModifier::new().width(1.0).height(h))
}

fn empty() -> View {
    UiBox(CoreModifier::new().width(0.0).height(0.0))
}

fn popup_anim_config(key: &str) -> AnimatedVisibilityConfig {
    AnimatedVisibilityConfig {
        key: key.into(),
        spec: AnimationSpec::tween(Duration::from_millis(180), Easing::EaseOut),
        enter: EnterTransition::ScaleIn { initial: 0.94 },
        exit: ExitTransition::ScaleOut { target: 0.94 },
    }
}

/// Full-screen dimmer that swallows clicks so lawn/HUD controls don't leak.
fn scrim() -> View {
    UiBox(
        Modifier::new()
            .fill_max_size()
            .background(cola(0, 0, 0, 170))
            .input_blocker()
            .render_z_index(100.0),
    )
}

/// PvZ wood panel.
fn wood_panel(w: f32, h: Option<f32>, children: Vec<View>) -> View {
    let mut m = Modifier::new()
        .width(w)
        .padding(16.0)
        .background(col(72, 48, 24))
        .border(3.0, col(35, 22, 9), 8.0)
        .clip_rounded(8.0)
        .shadow(10.0, 4.0)
        .render_z_index(110.0);
    if let Some(h) = h {
        m = m.height(h);
    }
    Column(
        m.align_items(AlignItems::CENTER)
            .justify_content(JustifyContent::CENTER),
    )
    .child(children)
}

fn title_text(label: impl Into<String>) -> View {
    Text(label.into())
        .size(34.0)
        .color(col(255, 222, 74))
        .font_weight(FontWeight::BOLD)
        .text_align(TextAlign::Center)
}

fn body_text(label: impl Into<String>) -> View {
    Text(label.into())
        .size(16.0)
        .color(Color::WHITE)
        .text_align(TextAlign::Center)
}

fn dim_text(label: impl Into<String>) -> View {
    Text(label.into())
        .size(13.0)
        .color(cola(255, 255, 255, 190))
        .text_align(TextAlign::Center)
}

/// Custom PvZ-styled button (no flat green look).
fn menu_btn(app: &mut PilotApp, label: impl Into<String>, w: f32, h: f32, act: UiAct) -> View {
    let label = label.into();
    let app_ptr = app as *mut PilotApp;
    ZStack(
        Modifier::new()
            .width(w)
            .height(h)
            .background(col(104, 72, 34))
            .border(2.0, col(222, 183, 86), 7.0)
            .clip_rounded(7.0)
            .on_click(move || {
                // SAFETY: synchronous compose-time dispatch only.
                let app = unsafe { &mut *app_ptr };
                apply_act(app, act.clone());
            })
            .cursor(CursorIcon::Pointer),
    )
    .child(
        Center(Modifier::new().fill_max_size()).child(
            Text(label)
                .size(18.0)
                .color(Color::WHITE)
                .font_weight(FontWeight::BOLD)
                .text_align(TextAlign::Center),
        ),
    )
}

fn center_modal(child: View) -> View {
    ZStack(Modifier::new().fill_max_size()).child((
        scrim(),
        Center(Modifier::new().fill_max_size()).child(child),
    ))
}

fn progress_bar(w: f32, h: f32, pct: f32, fill: Color) -> View {
    ZStack(
        Modifier::new()
            .width(w)
            .height(h)
            .background(col(40, 28, 16))
            .clip_rounded(h * 0.5),
    )
    .child(UiBox(
        Modifier::new()
            .width((w * pct.clamp(0.0, 1.0)).max(1.0))
            .height(h)
            .background(fill)
            .clip_rounded(h * 0.5),
    ))
}

fn fake_lawn_bg() -> View {
    ZStack(Modifier::new().fill_max_size()).child((
        UiBox(Modifier::new().fill_max_size().background(col(86, 145, 43))),
        UiBox(
            Modifier::new()
                .absolute()
                .offset_left(40.0)
                .offset_top(80.0)
                .width(720.0)
                .height(500.0)
                .background(col(96, 160, 48))
                .clip_rounded(3.0)
                .hit_passthrough(),
        ),
        UiBox(
            Modifier::new()
                .absolute()
                .offset_left(400.0)
                .offset_top(80.0)
                .width(360.0)
                .height(500.0)
                .background(cola(50, 110, 25, 60))
                .hit_passthrough(),
        ),
    ))
}

/// Seed-def `[f32; 4]` tint -> UI color.
fn packet_color(c: [f32; 4]) -> Color {
    rgba(c)
}

/// 50x70 seed packet tile with portrait swatch + cost label.
/// `label` is the already-short display text (see `short_seed`).
fn packet_tile(
    app: &mut PilotApp,
    label: String,
    cost: i32,
    color: Color,
    selected: bool,
    act: UiAct,
) -> View {
    let app_ptr = app as *mut PilotApp;
    ZStack(
        Modifier::new()
            .width(SEED_PACKET_W)
            .height(SEED_PACKET_H)
            .background(col(210, 175, 85))
            .border(
                if selected { 3.0 } else { 2.0 },
                if selected {
                    col(255, 230, 60)
                } else {
                    col(90, 60, 25)
                },
                3.0,
            )
            .clip_rounded(3.0)
            .on_click(move || {
                let app = unsafe { &mut *app_ptr };
                apply_act(app, act.clone());
            })
            .cursor(CursorIcon::Pointer),
    )
    .child((
        Column(
            Modifier::new()
                .size(SEED_PACKET_W - 6.0, SEED_PACKET_H - 6.0)
                .padding(3.0)
                .align_items(AlignItems::CENTER)
                .justify_content(JustifyContent::SPACE_BETWEEN),
        )
        .child((
            Text(label)
                .size(10.0)
                .color(col(35, 24, 10))
                .font_weight(FontWeight::BOLD)
                .single_line(),
            UiBox(
                Modifier::new()
                    .width(40.0)
                    .height(30.0)
                    .background(color)
                    .clip_rounded(2.0),
            ),
            Text(cost.to_string())
                .size(12.0)
                .color(col(35, 24, 10))
                .font_weight(FontWeight::BOLD),
        )),
        if selected {
            UiBox(
                Modifier::new()
                    .size(SEED_PACKET_W, SEED_PACKET_H)
                    .background(cola(255, 230, 60, 45)),
            )
        } else {
            empty()
        },
    ))
}

fn splash_view(app: &mut PilotApp) -> View {
    let _ = app;
    ZStack(Modifier::new().fill_max_size()).child((
        UiBox(Modifier::new().fill_max_size().background(col(18, 40, 14))),
        Center(Modifier::new().fill_max_size()).child(
            Column(Modifier::new().align_items(AlignItems::CENTER).gap(8.0)).child((
                Text("RoZVP".to_string())
                    .size(56.0)
                    .color(col(255, 222, 70))
                    .font_weight(FontWeight::BOLD),
                dim_text(t(app, "tagline")),
            )),
        ),
    ))
}

fn loading_view(app: &mut PilotApp) -> View {
    ZStack(Modifier::new().fill_max_size()).child((
        UiBox(Modifier::new().fill_max_size().background(col(18, 40, 14))),
        Center(Modifier::new().fill_max_size()).child(
            Column(Modifier::new().align_items(AlignItems::CENTER)).child((
                title_text(t(app, "loading")),
                spacer(16.0),
                progress_bar(330.0, 18.0, 0.6, col(120, 185, 45)),
                spacer(8.0),
                body_text("60%"),
            )),
        ),
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
    let base = selector_ui(app);
    // Popups animate in over the hub like the old build.
    let layer = match overlay {
        Overlay::SeedChooser => {
            AnimatedVisibility(true, seed_chooser(app), popup_anim_config("seeds"))
        }
        Overlay::Settings => AnimatedVisibility(
            true,
            settings_view(app),
            popup_anim_config("title_settings"),
        ),
        Overlay::Credits => {
            AnimatedVisibility(true, credits_view(app), popup_anim_config("title_credits"))
        }
        _ => empty(),
    };
    ZStack(Modifier::new().fill_max_size()).child((base, layer))
}

/// Wood mode button with subtitle, 286x50 like the old hub.
fn mode_button(app: &mut PilotApp, label: String, sub: String) -> View {
    let app_ptr = app as *mut PilotApp;
    ZStack(
        Modifier::new()
            .width(286.0)
            .height(50.0)
            .background(col(98, 70, 34))
            .border(2.0, col(222, 183, 86), 8.0)
            .clip_rounded(8.0)
            .on_click(move || {
                let app = unsafe { &mut *app_ptr };
                apply_act(app, UiAct::OpenAdventure);
            })
            .cursor(CursorIcon::Pointer),
    )
    .child(
        Column(
            Modifier::new()
                .fill_max_size()
                .padding(5.0)
                .align_items(AlignItems::CENTER)
                .justify_content(JustifyContent::CENTER),
        )
        .child((
            Text(label)
                .size(19.0)
                .color(Color::WHITE)
                .font_weight(FontWeight::BOLD),
            Text(sub).size(10.0).color(cola(255, 255, 255, 190)),
        )),
    )
}

fn locked_button(label: String) -> View {
    ZStack(
        Modifier::new()
            .width(286.0)
            .height(50.0)
            .background(col(60, 55, 50))
            .border(2.0, col(120, 110, 95), 8.0)
            .clip_rounded(8.0),
    )
    .child(
        Center(Modifier::new().fill_max_size()).child(
            Text(format!("{label} 🔒"))
                .size(17.0)
                .color(cola(255, 255, 255, 140))
                .font_weight(FontWeight::BOLD),
        ),
    )
}

fn selector_ui(app: &mut PilotApp) -> View {
    let adventure_label = {
        let ui = app.sim.world.resource::<super::state::UiShare>();
        let ui = ui.ui.lock().unwrap();
        format!(
            "{} {}",
            t(app, "adventure"),
            super::levels::level_label(ui.adventure_level)
        )
    };
    let adventure_sub = t(app, "adventure-sub");
    let minigames = t(app, "mini-games");
    let minigames_sub = t(app, "minigames-sub");
    let puzzle = t(app, "puzzle");
    let puzzle_sub = t(app, "puzzle-sub");
    let survival = t(app, "survival");
    let survival_sub = t(app, "survival-sub");
    let zen = t(app, "zen-garden");
    let almanac = t(app, "almanac");
    let store = t(app, "store");
    let settings = t(app, "settings");
    let help = t(app, "help");
    let quit = t(app, "quit");
    let user = t(app, "user-label");

    let menu = wood_panel(
        330.0,
        Some(520.0),
        vec![
            Text("RoZVP".to_string())
                .size(40.0)
                .color(col(255, 222, 70))
                .font_weight(FontWeight::BOLD),
            spacer(6.0),
            mode_button(app, adventure_label, adventure_sub),
            mode_button(app, minigames, minigames_sub),
            mode_button(app, puzzle, puzzle_sub),
            mode_button(app, survival, survival_sub),
            locked_button(zen),
            locked_button(almanac),
            locked_button(store),
            spacer(4.0),
            Row(Modifier::new().gap(6.0)).child((
                menu_btn(app, settings, 88.0, 34.0, UiAct::OpenSettings),
                menu_btn(app, help, 88.0, 34.0, UiAct::OpenCredits),
                menu_btn(app, quit, 88.0, 34.0, UiAct::QuitApp),
            )),
        ],
    );

    // User chip is display-only; keep it a non-clickable wood button look.
    let user_chip = ZStack(
        Modifier::new()
            .width(200.0)
            .height(40.0)
            .background(col(104, 72, 34))
            .border(2.0, col(222, 183, 86), 7.0)
            .clip_rounded(7.0),
    )
    .child(
        Center(Modifier::new().fill_max_size()).child(
            Text(user)
                .size(18.0)
                .color(Color::WHITE)
                .font_weight(FontWeight::BOLD)
                .text_align(TextAlign::Center),
        ),
    );

    ZStack(Modifier::new().fill_max_size()).child((
        fake_lawn_bg(),
        UiBox(
            Modifier::new()
                .absolute()
                .offset_left(28.0)
                .offset_top(28.0)
                .render_z_index(5.0),
        )
        .child(menu),
        UiBox(
            Modifier::new()
                .absolute()
                .offset_right(20.0)
                .offset_bottom(16.0)
                .render_z_index(5.0),
        )
        .child(user_chip),
    ))
}

fn game_view(app: &mut PilotApp, ctx: &RenderContext) -> View {
    // Board canvas fills the window (aspect-fit lawn); rigs overlay it in
    // the same space; HUD floats on top in window space; modals center over
    // everything — mirrors the old in-game look.
    ZStack(Modifier::new().fill_max_size()).child((
        board_layer(app),
        rigs_layer(app, ctx),
        ingame_hud(app),
        overlay_layer(app),
    ))
}

/// Lawn + entities through the framework viewport: sprites, world texts,
/// backdrop and flash tint all share the viewport's dp fit, and picks
/// arrive back in world coords — no game-side unit math. Unit discipline
/// lives in `repame-sprite` (`Viewport2d`); this layer only wires the sim
/// snapshot in and the click queue out.
///
/// The transition fade is transform-free (a fullscreen black rect), so it
/// stays a trivial overlay canvas game-side rather than growing the
/// framework snapshot.
fn board_layer(app: &mut PilotApp) -> View {
    let input = frame_input(&mut app.sim.world, [BOARD_WIDTH, BOARD_HEIGHT]);
    // Transition fade alpha (0 = no cover).
    let fade = app.sim.world.resource::<repame_fx::TransitionFx>().alpha();
    let fit_cell = app.board_fit.clone();
    let app_ptr = app as *mut PilotApp;
    let board = Viewport2d(input, fit_cell, move |ev| {
        let PickEvent::Click { world, .. } = ev else {
            return;
        };
        // SAFETY: synchronous compose-time dispatch only.
        let app = unsafe { &mut *app_ptr };
        app.sim
            .world
            .resource_mut::<super::state::ClickQueue>()
            .clicks
            .push((world.x, world.y));
    });
    if fade <= 0.001 {
        return board;
    }
    let alpha = (fade.clamp(0.0, 1.0) * 255.0) as u8;
    ZStack(Modifier::new().fill_max_size()).child((
        board,
        Canvas(
            Modifier::new().fill_max_size().hit_passthrough(),
            move |scope: &mut repose_canvas::DrawScope| {
                scope.draw_rect(
                    Rect {
                        x: 0.0,
                        y: 0.0,
                        w: scope.size.width,
                        h: scope.size.height,
                    },
                    Color::from_rgba(0, 0, 0, alpha),
                    0.0,
                );
            },
        ),
    ))
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
///
/// Each rig paints through [`zombie_actor_view`]: a transparent canvas with
/// no editor chrome (the old `RenamitePlayer` embed painted an opaque
/// background plus a checkerboard artboard, i.e. the black square).
/// Surfaces are positioned by the framework (`ActorFrame`) through the
/// viewport's shared geometry, so they track the sim `Pos` while moving —
/// glued to sprites and picks even under camera shake.
fn rigs_layer(app: &mut PilotApp, ctx: &RenderContext) -> View {
    use super::comps::{Pos, Zombie};
    // Entity-keyed pass (queries borrow world; hosts live outside it).
    let items: Vec<(f32, f32, bool, repame_actors::PlayerHostRef)> = {
        let mut q = app
            .sim
            .world
            .query::<(repame_sim::bevy_ecs::prelude::Entity, &Zombie, &Pos)>();
        let world = &app.sim.world;
        let mut out = Vec::new();
        for (e, zombie, pos) in q.iter(world) {
            if let Some(entry) = app.rigs.hosts.get(&e) {
                out.push((pos.x, pos.y, zombie.hypnotized, entry.host.clone()));
            }
        }
        out
    };
    let mut views = Vec::new();
    let geom = app.board_fit.clone();
    for (x, y, hypnotized, host) in items {
        // 64x80 surface, artboard 256x320 scaled by host.view (0.25).
        // The rig faces right; the horde walks left, so mirror around the
        // surface center exactly like the old `sprite.flip_x` did.
        views.push(ActorFrame(
            [x, y],
            [64.0, 80.0],
            geom.clone(),
            !hypnotized,
            zombie_actor_view(host, ctx.clone()),
        ));
    }
    ZStack(
        Modifier::new()
            .fill_max_size()
            .hit_passthrough()
            .render_z_index(5.0),
    )
    .child(views)
}

/// Transparent in-game rig surface: paints the live `Scene` with no editor
/// chrome (no opaque background, shadow, checkerboard, or border) so the
/// zombie reads like the old 64x80 baked-atlas sprite. Playback is ticked
/// in `rigs::sync_rigs`, not here, so paused games freeze and there is no
/// double-tick speedup.
fn zombie_actor_view(host: repame_actors::PlayerHostRef, ctx: RenderContext) -> View {
    Canvas(
        Modifier::new().fill_max_size().hit_passthrough(),
        move |scope| {
            let mut h = host.borrow_mut();
            let sw = scope.size.width as f64;
            let sh = scope.size.height as f64;
            if sw <= 1.0 || sh <= 1.0 {
                return;
            }
            let art = h.artboard();
            if art.x <= 0.0 || art.y <= 0.0 {
                return;
            }
            // Exact-fit the 256x320 artboard into the surface (no editor
            // margin): a 64x80 surface yields the classic 0.25 scale.
            let scale = (sw / art.x).min(sh / art.y);
            h.view.scale = scale;
            h.view.offset.x = (sw - art.x * scale) * 0.5;
            h.view.offset.y = (sh - art.y * scale) * 0.5;
            if h.dirty_images {
                let host = &mut *h;
                host.renderer
                    .sync_document_images(&host.player.project.document, &ctx);
                host.dirty_images = false;
            }
            let scene = h.player.scene().clone();
            let view = h.view;
            let prepared = h.renderer.prepare(&scene, &view);
            h.renderer.paint_prepared(&prepared, scope);
        },
    )
}

fn ingame_hud(app: &mut PilotApp) -> View {
    let (
        sun,
        slots,
        shovel_selected,
        progress,
        flags_done,
        flags_total,
        advice_text,
        advice_visible,
    ) = {
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

    // Sun counter badge.
    let sun_badge = Row(Modifier::new()
        .absolute()
        .offset_left(10.0)
        .offset_top(SEED_BANK_Y)
        .width(SUN_BOX_W)
        .height(SEED_PACKET_H + 4.0)
        .padding(6.0)
        .gap(6.0)
        .background(col(48, 32, 16))
        .border(2.0, col(30, 18, 8), 8.0)
        .clip_rounded(8.0)
        .align_items(AlignItems::CENTER)
        .render_z_index(10.0))
    .child((
        UiBox(
            Modifier::new()
                .width(26.0)
                .height(26.0)
                .background(col(255, 220, 40))
                .border(2.0, col(220, 150, 20), 13.0)
                .clip_rounded(13.0),
        ),
        Text(sun.to_string())
            .size(22.0)
            .color(Color::WHITE)
            .font_weight(FontWeight::BOLD),
    ));

    // Seed bank bar.
    let mut bank_children: Vec<View> = Vec::new();
    for (i, s) in slots.iter().take(10).enumerate() {
        bank_children.push(seed_packet_hud(app, i, s));
    }
    let bank = Row(Modifier::new()
        .absolute()
        .offset_left(SEED_BANK_X)
        .offset_top(SEED_BANK_Y)
        .height(SEED_PACKET_H + 8.0)
        .padding(3.0)
        .gap(3.0)
        .background(col(48, 32, 16))
        .border(2.0, col(30, 18, 8), 6.0)
        .clip_rounded(6.0)
        .render_z_index(10.0))
    .child(bank_children);

    // Shovel.
    let app_ptr = app as *mut PilotApp;
    let shovel_label = t(app, "shovel");
    let shovel = ZStack(
        Modifier::new()
            .absolute()
            .offset_left(SHOVEL_X)
            .offset_top(SEED_BANK_Y)
            .width(58.0)
            .height(SEED_PACKET_H)
            .background(if shovel_selected {
                col(145, 100, 40)
            } else {
                col(70, 50, 28)
            })
            .border(2.0, col(35, 22, 9), 5.0)
            .clip_rounded(5.0)
            .on_click(move || {
                let app = unsafe { &mut *app_ptr };
                toggle_shovel(app);
            })
            .cursor(CursorIcon::Pointer)
            .render_z_index(10.0),
    )
    .child(
        Center(Modifier::new().fill_max_size()).child(
            Text(shovel_label)
                .size(12.0)
                .color(Color::WHITE)
                .font_weight(FontWeight::BOLD),
        ),
    );

    // Pause button (top-right).
    let app_ptr = app as *mut PilotApp;
    let pause_btn = ZStack(
        Modifier::new()
            .absolute()
            .offset_right(12.0)
            .offset_top(SEED_BANK_Y)
            .width(46.0)
            .height(36.0)
            .background(col(70, 50, 28))
            .border(2.0, col(35, 22, 9), 5.0)
            .clip_rounded(5.0)
            .on_click(move || {
                let app = unsafe { &mut *app_ptr };
                pause_game(app);
            })
            .cursor(CursorIcon::Pointer)
            .render_z_index(10.0),
    )
    .child(
        Center(Modifier::new().fill_max_size()).child(
            Text("||".to_string())
                .size(18.0)
                .color(Color::WHITE)
                .font_weight(FontWeight::BOLD),
        ),
    );

    // Progress meter (bottom-right).
    let flags_label = {
        let mut args = FluentArgs::new();
        args.set("done", flags_done);
        args.set("total", flags_total);
        app.i18n.t_with_args("hud-flags-count", Some(&args))
    };
    let meter = Column(
        Modifier::new()
            .absolute()
            .offset_right(16.0)
            .offset_bottom(14.0)
            .align_items(AlignItems::CENTER)
            .render_z_index(10.0),
    )
    .child((
        Text(flags_label)
            .size(11.0)
            .color(Color::WHITE)
            .font_weight(FontWeight::BOLD),
        spacer(3.0),
        progress_bar(112.0, 16.0, progress, col(190, 35, 35)),
    ));

    // Advice banner (bottom-center).
    let advice_label = if advice_text.is_empty() {
        String::new()
    } else {
        t(app, &advice_text)
    };
    let advice_layer = if advice_visible {
        Column(
            Modifier::new()
                .absolute()
                .offset_bottom(64.0)
                .fill_max_width()
                .justify_content(JustifyContent::CENTER)
                .hit_passthrough()
                .render_z_index(12.0),
        )
        .child(
            UiBox(
                Modifier::new()
                    .width(430.0)
                    .padding(10.0)
                    .background(cola(15, 12, 8, 225))
                    .border(2.0, col(190, 150, 70), 8.0)
                    .clip_rounded(8.0),
            )
            .child(
                Center(Modifier::new().fill_max_width()).child(
                    Text(advice_label)
                        .size(15.0)
                        .color(col(255, 240, 180))
                        .text_align(TextAlign::Center),
                ),
            ),
        )
    } else {
        empty()
    };

    ZStack(Modifier::new().fill_max_size().hit_passthrough()).child((
        sun_badge,
        bank,
        shovel,
        pause_btn,
        meter,
        advice_layer,
    ))
}

fn short_name(name: &str) -> String {
    name.chars().take(4).collect()
}

/// Interactive HUD seed packet: recharge veil drains from the top,
/// unaffordable dims grey, selection highlights gold.
fn seed_packet_hud(app: &mut PilotApp, i: usize, s: &super::state::SeedSlot) -> View {
    let ready = s.ready.clamp(0.0, 1.0);
    let unready = 1.0 - ready;
    let affordable = s.affordable;
    let selected = s.selected;
    let short = short_seed(app, &s.seed_name);
    let cost = s.cost;
    let app_ptr = app as *mut PilotApp;

    let swatch_color = super::comps::seed_def(&s.seed_name)
        .map(|d| packet_color(d.color))
        .unwrap_or(col(80, 120, 40));

    ZStack(
        Modifier::new()
            .width(SEED_PACKET_W)
            .height(SEED_PACKET_H)
            .background(col(210, 175, 85))
            .border(
                if selected { 3.0 } else { 2.0 },
                if selected {
                    col(255, 230, 60)
                } else {
                    col(90, 60, 25)
                },
                3.0,
            )
            .clip_rounded(3.0)
            .on_click(move || {
                let app = unsafe { &mut *app_ptr };
                select_seed(app, i);
            })
            .cursor(CursorIcon::Pointer)
            .render_z_index(1.0),
    )
    .child((
        Column(
            Modifier::new()
                .size(SEED_PACKET_W - 6.0, SEED_PACKET_H - 6.0)
                .padding(3.0)
                .align_items(AlignItems::CENTER)
                .justify_content(JustifyContent::SPACE_BETWEEN),
        )
        .child((
            Text(short)
                .size(10.0)
                .color(col(40, 25, 10))
                .font_weight(FontWeight::BOLD)
                .single_line(),
            UiBox(
                Modifier::new()
                    .width(38.0)
                    .height(30.0)
                    .background(swatch_color)
                    .border(1.0, col(50, 40, 20), 2.0)
                    .clip_rounded(2.0),
            ),
            Text(cost.to_string())
                .size(12.0)
                .color(col(30, 20, 8))
                .font_weight(FontWeight::BOLD),
        )),
        // Recharge veil anchored to the top.
        if unready > 0.001 {
            UiBox(
                Modifier::new()
                    .width(SEED_PACKET_W)
                    .height(SEED_PACKET_H * unready)
                    .background(cola(0, 0, 0, 150))
                    .hit_passthrough()
                    .render_z_index(2.0),
            )
        } else {
            empty()
        },
        // Unaffordable grey-out.
        if !affordable {
            UiBox(
                Modifier::new()
                    .size(SEED_PACKET_W, SEED_PACKET_H)
                    .background(cola(0, 0, 0, 105))
                    .hit_passthrough()
                    .render_z_index(3.0),
            )
        } else {
            empty()
        },
    ))
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
    // Eagerly build only the visible modal; each is a wood panel on a
    // scrim with a scale pop like the old build.
    let modal = match overlay {
        Overlay::None => return empty(),
        Overlay::Pause => pause_ui(app),
        Overlay::GameOver => game_over_ui(app),
        Overlay::LevelComplete => level_complete_ui(app),
        Overlay::Award => award_ui(app),
        Overlay::NotEnoughSun => not_enough_sun_ui(app),
        Overlay::SeedChooser => seed_chooser(app),
        Overlay::Settings => settings_view(app),
        Overlay::Credits => credits_view(app),
    };
    let key = match overlay {
        Overlay::Pause => "pause",
        Overlay::GameOver => "game_over",
        Overlay::LevelComplete => "level_complete",
        Overlay::Award => "award",
        Overlay::NotEnoughSun => "not_enough_sun",
        Overlay::SeedChooser => "seeds",
        Overlay::Settings => "ingame_settings",
        Overlay::Credits => "ingame_credits",
        Overlay::None => "none",
    };
    ZStack(
        Modifier::new()
            .fill_max_size()
            .hit_passthrough()
            // Above rigs (5) and HUD (10-12): the scrim must dim every
            // zombie and panel, per `game_view` ("modals over everything").
            .render_z_index(20.0),
    )
    .child(AnimatedVisibility(true, modal, popup_anim_config(key)))
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
    ChooserPick(String),
    ChooserRemove(usize),
    ConfirmSeedChooser,
}

fn pause_ui(app: &mut PilotApp) -> View {
    let paused = t(app, "paused");
    let resume = t(app, "resume");
    let restart = t(app, "restart");
    let settings = t(app, "settings");
    let menu = t(app, "main-menu");
    center_modal(wood_panel(
        340.0,
        None,
        vec![
            title_text(paused),
            spacer(16.0),
            menu_btn(app, resume, 260.0, 48.0, UiAct::Resume),
            spacer(8.0),
            menu_btn(app, restart, 260.0, 48.0, UiAct::Restart),
            spacer(8.0),
            menu_btn(app, settings, 260.0, 48.0, UiAct::OpenSettings),
            spacer(8.0),
            menu_btn(app, menu, 260.0, 48.0, UiAct::QuitToTitle),
        ],
    ))
}

fn level_complete_ui(app: &mut PilotApp) -> View {
    let title = t(app, "level-complete");
    let finished = t(app, "finished-level");
    let level_name = app
        .sim
        .world
        .resource::<super::state::UiShare>()
        .ui
        .lock()
        .map(|ui| ui.level_name.clone())
        .unwrap_or_default();
    let cont = t(app, "continue");
    center_modal(wood_panel(
        420.0,
        None,
        vec![
            title_text(title),
            spacer(10.0),
            body_text(format!("{finished} {level_name}")),
            spacer(16.0),
            menu_btn(app, cont, 170.0, 46.0, UiAct::LevelOk),
        ],
    ))
}

fn award_ui(app: &mut PilotApp) -> View {
    let seed = app
        .sim
        .world
        .resource::<super::state::UiShare>()
        .ui
        .lock()
        .map(|ui| ui.pending_award_seed.clone())
        .unwrap_or(None);
    let award_title = t(app, "award-title");
    let title = match seed.as_deref() {
        Some(s) => {
            let mut args = FluentArgs::new();
            args.set("title", award_title);
            args.set("seed", t_seed(app, s));
            app.i18n.t_with_args("award-seed-is", Some(&args))
        }
        None => award_title,
    };
    let seed_name = seed.clone().unwrap_or_default();
    let color = super::comps::seed_def(&seed_name)
        .map(|d| packet_color(d.color))
        .unwrap_or(col(95, 150, 75));
    let awesome = t(app, "awesome");
    center_modal(wood_panel(
        440.0,
        None,
        vec![
            title_text(title),
            spacer(12.0),
            UiBox(
                Modifier::new()
                    .width(100.0)
                    .height(80.0)
                    .background(color)
                    .border(2.0, col(220, 180, 80), 8.0)
                    .clip_rounded(8.0),
            ),
            spacer(10.0),
            body_text(t_seed(app, &seed_name)),
            spacer(16.0),
            menu_btn(app, awesome, 170.0, 46.0, UiAct::AwardOk),
        ],
    ))
}

fn game_over_ui(app: &mut PilotApp) -> View {
    let title = t(app, "zombies-ate-your-brains");
    let menu = t(app, "main-menu");
    let retry = t(app, "try-again");
    center_modal(wood_panel(
        480.0,
        None,
        vec![
            Text(title)
                .size(23.0)
                .color(col(220, 40, 40))
                .font_weight(FontWeight::BOLD)
                .text_align(TextAlign::Center),
            spacer(16.0),
            Row(Modifier::new().gap(12.0)).child((
                menu_btn(app, menu, 150.0, 46.0, UiAct::QuitToTitle),
                menu_btn(app, retry, 150.0, 46.0, UiAct::Restart),
            )),
        ],
    ))
}

fn not_enough_sun_ui(app: &mut PilotApp) -> View {
    let title = t(app, "not-enough-sun");
    let ok = t(app, "ok");
    center_modal(wood_panel(
        360.0,
        None,
        vec![
            title_text(title),
            spacer(14.0),
            menu_btn(app, ok, 130.0, 44.0, UiAct::CloseOverlay),
        ],
    ))
}

/// Start the fade cover on phase switches (mirrors bevy
/// `begin_to_state` transitions; input blocks until uncover).
fn begin_transition(app: &mut PilotApp) {
    app.sim
        .world
        .resource_mut::<repame_fx::TransitionFx>()
        .begin();
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
            begin_transition(app);
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
            let award_is_none = award.is_none();
            let mut flow = app.sim.world.resource_mut::<super::state::FlowControl>();
            match award {
                Some(_) => flow.overlay = Overlay::Award,
                None => {
                    flow.overlay = Overlay::None;
                    flow.paused = false;
                }
            }
            if award_is_none {
                begin_transition(app);
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
            begin_transition(app);
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
        UiAct::ChooserPick(name) => {
            if let Ok(mut ui) = app.sim.world.resource::<super::state::UiShare>().ui.lock() {
                if !ui.chooser_picks.contains(&name) && ui.chooser_picks.len() < 10 {
                    ui.chooser_picks.push(name);
                }
            }
        }
        UiAct::ChooserRemove(idx) => {
            if let Ok(mut ui) = app.sim.world.resource::<super::state::UiShare>().ui.lock() {
                if idx < ui.chooser_picks.len() {
                    ui.chooser_picks.remove(idx);
                }
            }
        }
        UiAct::ConfirmSeedChooser => {
            confirm_chooser(app);
        }
    }
}

/// Settings dialog in a wood panel: three volume rows, language list,
/// Save/Back. Volumes apply immediately, Save persists to `save.ron`.
fn settings_view(app: &mut PilotApp) -> View {
    let (master, sfx, music, current_lang) = {
        let share = app.sim.world.resource::<super::state::UiShare>();
        let ui = share.ui.lock().unwrap();
        (ui.master_vol, ui.sfx_vol, ui.music_vol, ui.language.clone())
    };
    let settings_title = t(app, "settings");
    let master_label = t(app, "master-volume");
    let sfx_label = t(app, "sfx-volume");
    let music_label = t(app, "music-volume");
    let lang_label = t(app, "language");
    let save_label = t(app, "save");
    let back_label = t(app, "back");
    let codes = app.i18n.available();

    let mut children: Vec<View> = vec![title_text(settings_title), spacer(12.0)];
    for (label, value, channel) in [
        (master_label, master, VolChannel::Master),
        (sfx_label, sfx, VolChannel::Sfx),
        (music_label, music, VolChannel::Music),
    ] {
        let down = UiAct::BumpVol(channel, false);
        let up = UiAct::BumpVol(channel, true);
        children.push(vol_row(app, &label, value, down, up));
        children.push(spacer(8.0));
    }
    children.push(
        Text(format!("{lang_label}:"))
            .size(18.0)
            .color(Color::WHITE),
    );
    for code in codes {
        let selected = code == current_lang;
        let label = if selected {
            format!("[x] {code}")
        } else {
            format!("[ ] {code}")
        };
        children.push(lang_row(app, label, selected, UiAct::SetLanguage(code)));
    }
    children.push(spacer(16.0));
    let save = menu_btn(app, save_label, 130.0, 42.0, UiAct::SaveSettings);
    let back = menu_btn(app, back_label, 130.0, 42.0, UiAct::CloseOverlay);
    children.push(Row(Modifier::new().gap(10.0)).child((save, back)));

    center_modal(wood_panel(360.0, None, children))
}

fn vol_row(app: &mut PilotApp, label: &str, value: f32, down: UiAct, up: UiAct) -> View {
    let minus = menu_btn(app, "-".to_string(), 42.0, 34.0, down);
    let plus = menu_btn(app, "+".to_string(), 42.0, 34.0, up);
    Row(Modifier::new().gap(8.0).align_items(AlignItems::CENTER)).child((
        Text(format!("{label}: {:.0}%", value * 100.0))
            .size(16.0)
            .color(Color::WHITE)
            .font_weight(FontWeight::BOLD),
        minus,
        plus,
    ))
}

fn lang_row(app: &mut PilotApp, label: String, selected: bool, act: UiAct) -> View {
    let app_ptr = app as *mut PilotApp;
    ZStack(
        Modifier::new()
            .width(200.0)
            .height(36.0)
            .background(if selected {
                col(104, 72, 34)
            } else {
                col(60, 55, 50)
            })
            .border(
                2.0,
                if selected {
                    col(255, 230, 60)
                } else {
                    col(120, 110, 95)
                },
                7.0,
            )
            .clip_rounded(7.0)
            .on_click(move || {
                let app = unsafe { &mut *app_ptr };
                apply_act(app, act.clone());
            })
            .cursor(CursorIcon::Pointer),
    )
    .child(
        Center(Modifier::new().fill_max_size()).child(
            Text(label)
                .size(16.0)
                .color(Color::WHITE)
                .font_weight(FontWeight::BOLD),
        ),
    )
}

/// Credits dialog. Body copy goes through i18n like the old build; only
/// the Back button chrome is code-side.
fn credits_view(app: &mut PilotApp) -> View {
    let title = t(app, "help");
    let l1 = t(app, "credits-line-1");
    let l2 = t(app, "credits-line-2");
    let l3 = t(app, "credits-line-3");
    let l4 = t(app, "credits-line-4");
    let back = t(app, "back");
    center_modal(wood_panel(
        500.0,
        None,
        vec![
            title_text(title),
            spacer(12.0),
            body_text(l1),
            dim_text(l2),
            dim_text(l3),
            spacer(8.0),
            body_text(l4),
            spacer(16.0),
            menu_btn(app, back, 150.0, 42.0, UiAct::CloseOverlay),
        ],
    ))
}

fn seed_chooser(app: &mut PilotApp) -> View {
    let (picks, unlocked) = {
        let share = app.sim.world.resource::<super::state::UiShare>();
        let ui = share.ui.lock().unwrap();
        (ui.chooser_picks.clone(), ui.unlocked_seed_names.clone())
    };
    let title = t(app, "choose-your-seeds");
    let bank_label = t(app, "your-bank");
    let avail_label = t(app, "available-packets");
    let empty_label = t(app, "pick-seeds");
    let back_label = t(app, "back");
    let rock_label = t(app, "lets-rock");

    // Chosen bank strip (dark inset bar, parchment tiles).
    let mut chosen_children: Vec<View> = Vec::new();
    if picks.is_empty() {
        chosen_children.push(dim_text(empty_label));
    }
    for (i, name) in picks.iter().enumerate() {
        let cost = super::comps::seed_def(name).map(|d| d.cost).unwrap_or(0);
        let color = super::comps::seed_def(name)
            .map(|d| packet_color(d.color))
            .unwrap_or(col(95, 150, 75));
        let shown = short_seed(app, name);
        chosen_children.push(packet_tile(
            app,
            shown,
            cost,
            color,
            false,
            UiAct::ChooserRemove(i),
        ));
    }
    let chosen_row = Row(Modifier::new()
        .height(SEED_PACKET_H + 8.0)
        .padding(4.0)
        .gap(4.0)
        .background(col(38, 26, 13))
        .border(2.0, col(25, 16, 7), 6.0)
        .clip_rounded(6.0))
    .child(chosen_children);

    // Available packets grid (8 columns, locked seeds hidden).
    let mut tiles: Vec<View> = Vec::new();
    for def in super::comps::SEED_DEFS {
        let name = def.name;
        if !unlocked.iter().any(|u| u == name) {
            continue;
        }
        let picked = picks.iter().any(|p| p == name);
        let shown = short_seed(app, name);
        tiles.push(packet_tile(
            app,
            shown,
            def.cost,
            packet_color(def.color),
            picked,
            UiAct::ChooserPick(name.to_string()),
        ));
    }
    let available_grid = Grid(8, Modifier::new(), tiles, 8.0, 8.0);

    center_modal(wood_panel(
        720.0,
        Some(530.0),
        vec![
            title_text(title),
            spacer(8.0),
            body_text(bank_label),
            spacer(6.0),
            chosen_row,
            spacer(14.0),
            body_text(avail_label),
            spacer(6.0),
            available_grid,
            spacer(16.0),
            Row(Modifier::new().gap(12.0)).child((
                menu_btn(app, back_label, 130.0, 44.0, UiAct::CloseOverlay),
                menu_btn(app, rock_label, 180.0, 50.0, UiAct::ConfirmSeedChooser),
            )),
        ],
    ))
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
    begin_transition(app);
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

    /// Headless paint proof for the live zombie rig: run the surface
    /// painter into a real `Scene` and check it emits vector geometry
    /// centered on the sim `Pos` (no missing zombie, no lane offset).
    #[test]
    fn zombie_rig_paints_geometry_on_its_lane() {
        use super::super::comps::{GameplayCleanup, Pos, Zombie, ZombieKind};
        use super::super::state::row_center_y;

        let mut app = PilotApp::new();
        app.enter_level(0);
        let (zx, zy) = (700.0, row_center_y(0));
        let e = app
            .sim
            .world
            .spawn((
                GameplayCleanup,
                Zombie::new(ZombieKind::Normal, 0),
                Pos { x: zx, y: zy },
            ))
            .id();
        app.advance(0.05);
        assert!(
            app.rigs.hosts.contains_key(&e),
            "sync mints exactly one host per zombie"
        );
        assert_eq!(app.rigs.hosts.len(), 1, "no hosts for non-zombies");
        // The zombie walks left during the advance: read live pos.
        let (zx, zy) = {
            let mut q = app.sim.world.query::<(&Pos, &Zombie)>();
            let world = &app.sim.world;
            let (pos, _) = q.iter(world).next().expect("zombie alive");
            (pos.x, pos.y)
        };

        let ctx = RenderContext::new();
        // Headless: board canvas never painted, so fit is identity.
        let view = super::rigs_layer(&mut app, &ctx);
        assert_eq!(view.children.len(), 1);
        let surface = &view.children[0];
        // 64x80 box centered on the sim pos.
        assert_eq!(surface.modifier.offset_left, Some(zx - 32.0));
        assert_eq!(surface.modifier.offset_top, Some(zy - 40.0));
        let canvas = surface
            .children
            .iter()
            .find(|c| c.modifier.painter.is_some())
            .expect("surface wraps a paint canvas");
        let painter = canvas.modifier.painter.clone().unwrap();
        let mut scene = repose_core::Scene::default();
        let rect = repose_core::Rect {
            x: zx - 32.0,
            y: zy - 40.0,
            w: 64.0,
            h: 80.0,
        };
        painter(&mut scene, rect, 1.0);

        let mut count = 0;
        let (mut min_x, mut min_y) = (f32::INFINITY, f32::INFINITY);
        let (mut max_x, mut max_y) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for node in &scene.nodes {
            if let repose_core::SceneNode::VectorMesh {
                mesh, transform, ..
            } = node
            {
                let [a, b, c, d, tx, ty] = *transform;
                for v in mesh.vertices.iter() {
                    let x = a * v.pos[0] + c * v.pos[1] + tx;
                    let y = b * v.pos[0] + d * v.pos[1] + ty;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                    count += 1;
                }
            }
        }
        assert!(count > 100, "zombie must paint real geometry, got {count}");
        let (cx, cy) = ((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
        assert!(
            (cx - zx).abs() < 16.0,
            "paint centered horizontally on sim x: {cx} vs {zx}"
        );
        assert!(
            (cy - zy).abs() < 8.0,
            "paint centered vertically on sim lane: {cy} vs {zy}"
        );
    }
}
