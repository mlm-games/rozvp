//! All chrome art is placeholder art (wood panels, seed packets, meters) using
//! `Box`/`Row`/`Column`/`ZStack` + `Modifier` + `on_click`; Material widgets are
//! only kept for the settings language dropdown. Clicks flow exclusively
//! through `UiAction` -> `UiBridge`.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use repose_core::View;
use repose_core::prelude::{
    AlignItems, AnimationSpec, Color as RColor, Easing, JustifyContent, Modifier, remember,
};
use repose_core::{CursorIcon, FontWeight, TextAlign};
use repose_material::material3::{
    ButtonConfig, DropdownMenu, DropdownMenuConfig, DropdownMenuEntry, DropdownMenuItem,
    FilledTonalButton, MenuState,
};
use repose_ui::anim_ext::{
    AnimatedVisibility, AnimatedVisibilityConfig, EnterTransition, ExitTransition,
};
use repose_ui::overlay::OverlayHandle;
use repose_ui::{Box, Center, Column, Grid, Row, Text as RText, TextStyle, ViewExt, ZStack};

use crate::app::{AppState, CHOOSER_SEEDS, OverlayMenu, SeedSlotUi, SharedUi};

// Actions + bridge

#[derive(Clone, Debug)]
#[allow(dead_code)] // future-phase actions are wired but not yet reachable
pub enum UiAction {
    // selector / modes
    OpenAdventure,
    OpenMiniGames,
    OpenPuzzle,
    OpenSurvival,
    OpenZenGarden,
    OpenAlmanac,
    OpenStore,
    OpenPause,

    // seed chooser / HUD
    ChooserPick(String),
    ChooserRemove(usize),
    ConfirmSeedChooser,
    SelectSeedSlot(usize),
    ToggleShovel,
    ClearCursor,
    RestartLevel,
    DialogOk,

    // template/global
    OpenSettings,
    OpenCredits,
    CloseOverlay,
    Resume,
    QuitToTitle,
    QuitApp,
    SetMasterVol(f32),
    SetSfxVol(f32),
    SetMusicVol(f32),
    SaveSettings,
    NextLanguage,
    SetLanguage(String),
}

#[derive(bevy::prelude::Resource, Clone)]
pub struct UiBridge {
    pub shared: Arc<Mutex<SharedUi>>,
    pub actions: Arc<Mutex<Vec<UiAction>>>,
}

// Helpers

fn t(translations: &HashMap<String, String>, key: &str, fallback: &str) -> String {
    translations
        .get(key)
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

fn col(r: u8, g: u8, b: u8) -> RColor {
    RColor::from_rgba(r, g, b, 255)
}

fn cola(r: u8, g: u8, b: u8, a: u8) -> RColor {
    RColor::from_rgba(r, g, b, a)
}

fn push(actions: &Arc<Mutex<Vec<UiAction>>>, a: UiAction) {
    if let Ok(mut q) = actions.lock() {
        q.push(a);
    }
}

fn spacer(h: f32) -> View {
    Box(Modifier::new().width(1.0).height(h))
}

fn empty() -> View {
    Box(Modifier::new().width(0.0).height(0.0))
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
    Box(Modifier::new()
        .fill_max_size()
        .background(cola(0, 0, 0, 170))
        .input_blocker()
        .render_z_index(100.0))
}

/// PvZ wood panel.
fn wood_panel(w: f32, h: Option<f32>, children: impl IntoIterator<Item = View>) -> View {
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
    .with_children(children.into_iter().collect())
}

fn title_text(label: impl Into<String>) -> View {
    RText(label.into())
        .size(34.0)
        .color(col(255, 222, 74))
        .font_weight(FontWeight::BOLD)
        .text_align(TextAlign::Center)
}

fn body_text(label: impl Into<String>) -> View {
    RText(label.into())
        .size(16.0)
        .color(RColor::WHITE)
        .text_align(TextAlign::Center)
}

fn dim_text(label: impl Into<String>) -> View {
    RText(label.into())
        .size(13.0)
        .color(cola(255, 255, 255, 190))
        .text_align(TextAlign::Center)
}

/// Custom PvZ-styled button (no Material look).
fn menu_btn(label: impl Into<String>, w: f32, h: f32, on: impl Fn() + 'static) -> View {
    let label = label.into();
    ZStack(
        Modifier::new()
            .width(w)
            .height(h)
            .background(col(104, 72, 34))
            .border(2.0, col(222, 183, 86), 7.0)
            .clip_rounded(7.0)
            .on_click(on)
            .cursor(CursorIcon::Pointer),
    )
    .child(
        Center(Modifier::new().fill_max_size()).child(
            RText(label)
                .size(18.0)
                .color(RColor::WHITE)
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

// Layout constants (800x600 design space).
const SEED_PACKET_W: f32 = 50.0;
const SEED_PACKET_H: f32 = 70.0;
const SUN_BOX_W: f32 = 72.0;
const SEED_BANK_X: f32 = 88.0;
const SEED_BANK_Y: f32 = 8.0;
const SHOVEL_X: f32 = 622.0;

// compose_root

pub fn compose_root(
    overlay: OverlayHandle,
    st: SharedUi,
    actions: Arc<Mutex<Vec<UiAction>>>,
) -> View {
    let settings_view = settings_ui(overlay, &st, actions.clone());

    let content = match st.phase {
        AppState::Splash => splash_ui(),
        AppState::Loading => loading_ui(&st),
        AppState::Title => ZStack(Modifier::new().fill_max_size()).child((
            selector_ui(&st, actions.clone()),
            overlay_layer_title(&st, settings_view.clone(), actions.clone()),
        )),
        AppState::InGame => ZStack(Modifier::new().fill_max_size()).child((
            ingame_hud(&st, actions.clone()),
            overlay_layer_ingame(&st, settings_view.clone(), actions.clone()),
        )),
    };

    let root = ZStack(Modifier::new().fill_max_size());
    if st.transition_alpha > 0.001 || st.flash_alpha > 0.001 {
        let fade_a = (st.transition_alpha.clamp(0.0, 1.0) * 255.0) as u8;
        let flash_a = (st.flash_alpha.clamp(0.0, 1.0) * 255.0) as u8;
        root.child((
            content,
            Box(Modifier::new()
                .fill_max_size()
                .background(cola(0, 0, 0, fade_a))),
            Box(Modifier::new()
                .fill_max_size()
                .background(cola(flash_a, flash_a, flash_a, flash_a))),
        ))
    } else {
        root.child(content)
    }
}

fn overlay_layer_title(
    st: &SharedUi,
    settings_view: View,
    actions: Arc<Mutex<Vec<UiAction>>>,
) -> View {
    ZStack(Modifier::new().fill_max_size().hit_passthrough()).child((
        AnimatedVisibility(
            st.overlay == OverlayMenu::SeedChooser,
            seed_chooser_ui(st, actions.clone()),
            popup_anim_config("seeds"),
        ),
        AnimatedVisibility(
            st.overlay == OverlayMenu::Settings,
            settings_view.clone(),
            popup_anim_config("title_settings"),
        ),
        AnimatedVisibility(
            st.overlay == OverlayMenu::Credits,
            credits_ui(st, actions.clone()),
            popup_anim_config("title_credits"),
        ),
    ))
}

fn overlay_layer_ingame(
    st: &SharedUi,
    settings_view: View,
    actions: Arc<Mutex<Vec<UiAction>>>,
) -> View {
    ZStack(Modifier::new().fill_max_size().hit_passthrough()).child((
        AnimatedVisibility(
            st.overlay == OverlayMenu::Pause,
            pause_ui(st, actions.clone()),
            popup_anim_config("pause"),
        ),
        AnimatedVisibility(
            st.overlay == OverlayMenu::Settings,
            settings_view.clone(),
            popup_anim_config("ingame_settings"),
        ),
        AnimatedVisibility(
            st.overlay == OverlayMenu::Credits,
            credits_ui(st, actions.clone()),
            popup_anim_config("ingame_credits"),
        ),
        AnimatedVisibility(
            st.overlay == OverlayMenu::NotEnoughSun,
            not_enough_sun_ui(actions.clone()),
            popup_anim_config("not_enough_sun"),
        ),
        AnimatedVisibility(
            st.overlay == OverlayMenu::LevelComplete,
            level_complete_ui(st, actions.clone()),
            popup_anim_config("level_complete"),
        ),
        AnimatedVisibility(
            st.overlay == OverlayMenu::Award,
            award_ui(st, actions.clone()),
            popup_anim_config("award"),
        ),
        AnimatedVisibility(
            st.overlay == OverlayMenu::GameOver,
            game_over_ui(st, actions.clone()),
            popup_anim_config("game_over"),
        ),
    ))
}

// Splash / loading

fn splash_ui() -> View {
    ZStack(Modifier::new().fill_max_size()).child((
        Box(Modifier::new().fill_max_size().background(col(18, 40, 14))),
        Center(Modifier::new().fill_max_size()).child(
            Column(Modifier::new().align_items(AlignItems::CENTER).gap(8.0)).child((
                RText("RoZVP")
                    .size(56.0)
                    .color(col(255, 222, 70))
                    .font_weight(FontWeight::BOLD),
                dim_text("A Repose + Bevy PvZ recreation"),
            )),
        ),
    ))
}

fn loading_ui(st: &SharedUi) -> View {
    let pct = st.loading_progress.clamp(0.0, 1.0);
    ZStack(Modifier::new().fill_max_size()).child((
        Box(Modifier::new().fill_max_size().background(col(18, 40, 14))),
        Center(Modifier::new().fill_max_size()).child(
            Column(Modifier::new().align_items(AlignItems::CENTER)).child((
                title_text(t(&st.translations, "loading", "Loading...")),
                spacer(16.0),
                progress_bar(330.0, 18.0, pct, col(120, 185, 45)),
                spacer(8.0),
                body_text(format!("{:.0}%", pct * 100.0)),
            )),
        ),
    ))
}

fn progress_bar(w: f32, h: f32, pct: f32, fill: RColor) -> View {
    ZStack(
        Modifier::new()
            .width(w)
            .height(h)
            .background(col(40, 28, 16))
            .clip_rounded(h * 0.5),
    )
    .child(Box(Modifier::new()
        .width((w * pct.clamp(0.0, 1.0)).max(1.0))
        .height(h)
        .background(fill)
        .clip_rounded(h * 0.5)))
}

// Game selector (title hub)

fn selector_ui(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;

    let adventure_label = format!(
        "{} {}",
        t(tr, "adventure", "Adventure"),
        crate::game::level_flow::level_label(st.adventure_level),
    );

    let locked_btn = |label: String| {
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
                RText(format!("{label} 🔒"))
                    .size(17.0)
                    .color(cola(255, 255, 255, 140))
                    .font_weight(FontWeight::BOLD),
            ),
        )
    };

    let btn = |label: String, sub: &'static str, a: Arc<Mutex<Vec<UiAction>>>, action: UiAction| {
        ZStack(
            Modifier::new()
                .width(286.0)
                .height(50.0)
                .background(col(98, 70, 34))
                .border(2.0, col(222, 183, 86), 8.0)
                .clip_rounded(8.0)
                .on_click(move || push(&a, action.clone()))
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
                RText(label)
                    .size(19.0)
                    .color(RColor::WHITE)
                    .font_weight(FontWeight::BOLD),
                RText(sub).size(10.0).color(cola(255, 255, 255, 190)),
            )),
        )
    };

    let menu = wood_panel(
        330.0,
        Some(520.0),
        [
            RText("RoZVP")
                .size(40.0)
                .color(col(255, 222, 70))
                .font_weight(FontWeight::BOLD),
            spacer(6.0),
            btn(
                adventure_label,
                "Continue the main adventure",
                actions.clone(),
                UiAction::OpenAdventure,
            ),
            btn(
                t(tr, "mini-games", "Mini-Games"),
                "Quick weird challenges",
                actions.clone(),
                UiAction::OpenMiniGames,
            ),
            btn(
                t(tr, "puzzle", "Puzzle"),
                "Vasebreaker and I, Zombie",
                actions.clone(),
                UiAction::OpenPuzzle,
            ),
            btn(
                t(tr, "survival", "Survival"),
                "How long can you last?",
                actions.clone(),
                UiAction::OpenSurvival,
            ),
            locked_btn(t(tr, "zen-garden", "Zen Garden")),
            locked_btn(t(tr, "almanac", "Almanac")),
            locked_btn(t(tr, "store", "Store")),
            spacer(4.0),
            Row(Modifier::new().gap(6.0)).child((
                menu_btn(t(tr, "settings", "Options"), 88.0, 34.0, {
                    let a = actions.clone();
                    move || push(&a, UiAction::OpenSettings)
                }),
                menu_btn(t(tr, "help", "Help"), 88.0, 34.0, {
                    let a = actions.clone();
                    move || push(&a, UiAction::OpenCredits)
                }),
                menu_btn(t(tr, "quit", "Quit"), 88.0, 34.0, {
                    let a = actions.clone();
                    move || push(&a, UiAction::QuitApp)
                }),
            )),
        ],
    );

    ZStack(Modifier::new().fill_max_size()).child((
        fake_lawn_bg(),
        Box(Modifier::new()
            .absolute()
            .offset_left(28.0)
            .offset_top(28.0)
            .render_z_index(5.0))
        .child(menu),
        Box(Modifier::new()
            .absolute()
            .offset_right(20.0)
            .offset_bottom(16.0)
            .render_z_index(5.0))
        .child(menu_btn("User: Player", 200.0, 40.0, || {})),
    ))
}

fn fake_lawn_bg() -> View {
    ZStack(Modifier::new().fill_max_size()).child((
        Box(Modifier::new().fill_max_size().background(col(86, 145, 43))),
        Box(Modifier::new()
            .absolute()
            .offset_left(40.0)
            .offset_top(80.0)
            .width(720.0)
            .height(500.0)
            .background(col(96, 160, 48))
            .clip_rounded(3.0)
            .hit_passthrough()),
        Box(Modifier::new()
            .absolute()
            .offset_left(400.0)
            .offset_top(80.0)
            .width(360.0)
            .height(500.0)
            .background(cola(50, 110, 25, 60))
            .hit_passthrough()),
    ))
}

// Seed chooser

fn seed_chooser_ui(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;

    let mut chosen_row = Row(Modifier::new()
        .height(SEED_PACKET_H + 8.0)
        .padding(4.0)
        .gap(4.0)
        .background(col(38, 26, 13))
        .border(2.0, col(25, 16, 7), 6.0)
        .clip_rounded(6.0));
    if st.chooser_picks.is_empty() {
        chosen_row = chosen_row.child(dim_text("Pick some seeds!"));
    }
    for (i, name) in st.chooser_picks.iter().enumerate() {
        let cost = crate::game::defs::seed_def(name)
            .map(|d| d.cost)
            .unwrap_or(0);
        let color = crate::game::defs::seed_def(name)
            .map(|d| d.color)
            .map(packet_color)
            .unwrap_or(col(80, 120, 40));
        let a = actions.clone();
        chosen_row = chosen_row.child(packet_tile(name.clone(), cost, color, false, move || {
            push(&a, UiAction::ChooserRemove(i));
        }));
    }

    let mut tiles: Vec<View> = Vec::new();
    for name in CHOOSER_SEEDS {
        // Locked seeds stay hidden until awarded (FLOW-004).
        if !st.unlocked_seed_names.iter().any(|u| u == name) {
            continue;
        }
        let picked = st.chooser_picks.iter().any(|p| p == name);
        let def = crate::game::defs::seed_def(name);
        let cost = def.map(|d| d.cost).unwrap_or(0);
        let color = def
            .map(|d| d.color)
            .map(packet_color)
            .unwrap_or(col(95, 150, 75));
        let name_string = (*name).to_string();
        let a = actions.clone();
        tiles.push(packet_tile(
            name_string.clone(),
            cost,
            color,
            picked,
            move || {
                push(&a, UiAction::ChooserPick(name_string.clone()));
            },
        ));
    }

    let available_grid = Grid(8, Modifier::new(), tiles, 8.0, 8.0);

    center_modal(wood_panel(
        720.0,
        Some(530.0),
        [
            title_text(t(tr, "choose-your-seeds", "Choose Your Seeds")),
            spacer(8.0),
            body_text(t(tr, "your-bank", "Your bank:")),
            spacer(6.0),
            chosen_row,
            spacer(14.0),
            body_text(t(tr, "available-packets", "Available packets:")),
            spacer(6.0),
            available_grid,
            spacer(16.0),
            Row(Modifier::new().gap(12.0)).child((
                menu_btn(t(tr, "back", "Back"), 130.0, 44.0, {
                    let a = actions.clone();
                    move || push(&a, UiAction::CloseOverlay)
                }),
                menu_btn(t(tr, "lets-rock", "Let's Rock!"), 180.0, 50.0, {
                    let a = actions.clone();
                    move || push(&a, UiAction::ConfirmSeedChooser)
                }),
            )),
        ],
    ))
}

fn packet_color(c: bevy::prelude::Color) -> RColor {
    let s = c.to_srgba();
    RColor::from_rgba(
        (s.red * 255.0).round() as u8,
        (s.green * 255.0).round() as u8,
        (s.blue * 255.0).round() as u8,
        (s.alpha * 255.0).round() as u8,
    )
}

/// 50x70 seed packet tile with portrait swatch + cost label.
fn packet_tile(
    label: String,
    cost: i32,
    color: RColor,
    selected: bool,
    on_click: impl Fn() + 'static,
) -> View {
    let short: String = label.chars().take(4).collect();
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
            .on_click(on_click)
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
            RText(short)
                .size(10.0)
                .color(col(35, 24, 10))
                .font_weight(FontWeight::BOLD)
                .single_line(),
            Box(Modifier::new()
                .width(40.0)
                .height(30.0)
                .background(color)
                .clip_rounded(2.0)),
            RText(cost.to_string())
                .size(12.0)
                .color(col(35, 24, 10))
                .font_weight(FontWeight::BOLD),
        )),
        if selected {
            Box(Modifier::new()
                .size(SEED_PACKET_W, SEED_PACKET_H)
                .background(cola(255, 230, 60, 45)))
        } else {
            empty()
        },
    ))
}

// In-game HUD

fn ingame_hud(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;

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
        Box(Modifier::new()
            .width(26.0)
            .height(26.0)
            .background(col(255, 220, 40))
            .border(2.0, col(220, 150, 20), 13.0)
            .clip_rounded(13.0)),
        RText(st.sun.to_string())
            .size(22.0)
            .color(RColor::WHITE)
            .font_weight(FontWeight::BOLD),
    ));

    // Seed bank.
    let mut bank = Row(Modifier::new()
        .absolute()
        .offset_left(SEED_BANK_X)
        .offset_top(SEED_BANK_Y)
        .height(SEED_PACKET_H + 8.0)
        .padding(3.0)
        .gap(3.0)
        .background(col(48, 32, 16))
        .border(2.0, col(30, 18, 8), 6.0)
        .clip_rounded(6.0)
        .render_z_index(10.0));
    for (i, s) in st.seed_bank.iter().take(10).enumerate() {
        bank = bank.child(seed_packet_hud(i, s, actions.clone()));
    }

    // Shovel.
    let shovel = ZStack(
        Modifier::new()
            .absolute()
            .offset_left(SHOVEL_X)
            .offset_top(SEED_BANK_Y)
            .width(58.0)
            .height(SEED_PACKET_H)
            .background(if st.shovel_selected {
                col(145, 100, 40)
            } else {
                col(70, 50, 28)
            })
            .border(2.0, col(35, 22, 9), 5.0)
            .clip_rounded(5.0)
            .on_click({
                let a = actions.clone();
                move || push(&a, UiAction::ToggleShovel)
            })
            .cursor(CursorIcon::Pointer)
            .render_z_index(10.0),
    )
    .child(
        Center(Modifier::new().fill_max_size()).child(
            RText("Shovel")
                .size(12.0)
                .color(RColor::WHITE)
                .font_weight(FontWeight::BOLD),
        ),
    );

    // Pause button (top-right).
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
            .on_click({
                let a = actions.clone();
                move || push(&a, UiAction::OpenPause)
            })
            .cursor(CursorIcon::Pointer)
            .render_z_index(10.0),
    )
    .child(
        Center(Modifier::new().fill_max_size()).child(
            RText("||")
                .size(18.0)
                .color(RColor::WHITE)
                .font_weight(FontWeight::BOLD),
        ),
    );

    // Progress meter (bottom-right).
    let meter = Column(
        Modifier::new()
            .absolute()
            .offset_right(16.0)
            .offset_bottom(14.0)
            .align_items(AlignItems::CENTER)
            .render_z_index(10.0),
    )
    .child((
        RText(format!("Flags {}/{}", st.flags_done, st.flags_total))
            .size(11.0)
            .color(RColor::WHITE)
            .font_weight(FontWeight::BOLD),
        spacer(3.0),
        progress_bar(112.0, 16.0, st.progress, col(190, 35, 35)),
    ));

    // Advice banner (bottom-center).
    let advice_layer = if st.advice.visible {
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
            Box(Modifier::new()
                .width(430.0)
                .padding(10.0)
                .background(cola(15, 12, 8, 225))
                .border(2.0, col(190, 150, 70), 8.0)
                .clip_rounded(8.0))
            .child(
                Center(Modifier::new().fill_max_width()).child(
                    RText(st.advice.text.clone())
                        .size(15.0)
                        .color(col(255, 240, 180))
                        .text_align(TextAlign::Center),
                ),
            ),
        )
    } else {
        empty()
    };

    let _ = tr;
    ZStack(Modifier::new().fill_max_size().hit_passthrough()).child((
        sun_badge,
        bank,
        shovel,
        pause_btn,
        meter,
        advice_layer,
    ))
}

/// Interactive HUD seed packet: recharge veil drains from the top,
/// unaffordable dims grey, selection highlights gold.
fn seed_packet_hud(i: usize, s: &SeedSlotUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let ready = s.ready.clamp(0.0, 1.0);
    let unready = 1.0 - ready;
    let affordable = s.affordable;
    let selected = s.selected;
    let short: String = s.seed_name.chars().take(4).collect();
    let cost = s.cost;
    let a = actions.clone();

    let swatch_color = crate::game::defs::seed_def(&s.seed_name)
        .map(|d| d.color)
        .map(packet_color)
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
            .on_click(move || push(&a, UiAction::SelectSeedSlot(i)))
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
            RText(short)
                .size(10.0)
                .color(col(40, 25, 10))
                .font_weight(FontWeight::BOLD)
                .single_line(),
            Box(Modifier::new()
                .width(38.0)
                .height(30.0)
                .background(swatch_color)
                .border(1.0, col(50, 40, 20), 2.0)
                .clip_rounded(2.0)),
            RText(cost.to_string())
                .size(12.0)
                .color(col(30, 20, 8))
                .font_weight(FontWeight::BOLD),
        )),
        // Recharge veil anchored to the top.
        if unready > 0.001 {
            Box(Modifier::new()
                .width(SEED_PACKET_W)
                .height(SEED_PACKET_H * unready)
                .background(cola(0, 0, 0, 150))
                .hit_passthrough()
                .render_z_index(2.0))
        } else {
            empty()
        },
        // Unaffordable grey-out.
        if !affordable {
            Box(Modifier::new()
                .size(SEED_PACKET_W, SEED_PACKET_H)
                .background(cola(0, 0, 0, 105))
                .hit_passthrough()
                .render_z_index(3.0))
        } else {
            empty()
        },
    ))
}

// Overlays: pause / settings / credits / end screens

fn pause_ui(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;
    center_modal(wood_panel(
        340.0,
        None,
        [
            title_text(t(tr, "paused", "Paused")),
            spacer(16.0),
            menu_btn(t(tr, "resume", "Resume"), 260.0, 48.0, {
                let a = actions.clone();
                move || push(&a, UiAction::Resume)
            }),
            spacer(8.0),
            menu_btn(t(tr, "restart", "Restart"), 260.0, 48.0, {
                let a = actions.clone();
                move || push(&a, UiAction::RestartLevel)
            }),
            spacer(8.0),
            menu_btn(t(tr, "settings", "Options"), 260.0, 48.0, {
                let a = actions.clone();
                move || push(&a, UiAction::OpenSettings)
            }),
            spacer(8.0),
            menu_btn(t(tr, "main-menu", "Main Menu"), 260.0, 48.0, {
                let a = actions.clone();
                move || push(&a, UiAction::QuitToTitle)
            }),
        ],
    ))
}

fn level_complete_ui(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;
    center_modal(wood_panel(
        420.0,
        None,
        [
            title_text(t(tr, "level-complete", "LEVEL COMPLETE!")),
            spacer(10.0),
            body_text(format!("Finished level {}", st.level_name)),
            spacer(16.0),
            menu_btn(t(tr, "continue", "Continue"), 170.0, 46.0, {
                let a = actions.clone();
                move || push(&a, UiAction::DialogOk)
            }),
        ],
    ))
}

/// Award screen shown after LevelComplete when a seed unlock is pending.
fn award_ui(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;
    let seed = st.pending_award_seed.clone().unwrap_or_default();
    let def = crate::game::defs::seed_def(&seed);
    let color = def
        .map(|d| d.color)
        .map(packet_color)
        .unwrap_or(col(95, 150, 75));

    center_modal(wood_panel(
        440.0,
        None,
        [
            title_text(t(tr, "award-title", "You got a new plant!")),
            spacer(12.0),
            Box(Modifier::new()
                .width(100.0)
                .height(80.0)
                .background(color)
                .border(2.0, col(220, 180, 80), 8.0)
                .clip_rounded(8.0)),
            spacer(10.0),
            body_text(seed),
            spacer(16.0),
            menu_btn(t(tr, "awesome", "Awesome!"), 170.0, 46.0, {
                let a = actions.clone();
                move || push(&a, UiAction::DialogOk)
            }),
        ],
    ))
}

fn game_over_ui(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;
    center_modal(wood_panel(
        480.0,
        None,
        [
            RText(t(
                tr,
                "zombies-ate-your-brains",
                "THE ZOMBIES ATE YOUR BRAINS!",
            ))
            .size(23.0)
            .color(col(220, 40, 40))
            .font_weight(FontWeight::BOLD)
            .text_align(TextAlign::Center),
            spacer(16.0),
            Row(Modifier::new().gap(12.0)).child((
                menu_btn(t(tr, "main-menu", "Menu"), 150.0, 46.0, {
                    let a = actions.clone();
                    move || push(&a, UiAction::QuitToTitle)
                }),
                menu_btn(t(tr, "try-again", "Try Again"), 150.0, 46.0, {
                    let a = actions.clone();
                    move || push(&a, UiAction::RestartLevel)
                }),
            )),
        ],
    ))
}

fn not_enough_sun_ui(actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    center_modal(wood_panel(
        360.0,
        None,
        [
            title_text("Not enough sun!"),
            spacer(14.0),
            menu_btn("OK", 130.0, 44.0, move || {
                push(&actions, UiAction::CloseOverlay)
            }),
        ],
    ))
}

fn credits_ui(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;
    center_modal(wood_panel(
        500.0,
        None,
        [
            title_text(t(tr, "help", "Credits / Help")),
            spacer(12.0),
            body_text("RoZVP — faithful open reimplementation."),
            dim_text("Original game © PopCap. Assets are user supplied."),
            dim_text("Engine: Bevy. UI: Repose."),
            spacer(8.0),
            body_text("Click suns. Plant Sunflowers. Stop the zombies."),
            spacer(16.0),
            menu_btn(t(tr, "back", "Back"), 150.0, 42.0, move || {
                push(&actions, UiAction::CloseOverlay)
            }),
        ],
    ))
}

fn settings_ui(overlay: OverlayHandle, st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let a_m_down = actions.clone();
    let a_m_up = actions.clone();
    let a_s_down = actions.clone();
    let a_s_up = actions.clone();
    let a_mu_down = actions.clone();
    let a_mu_up = actions.clone();
    let a_save = actions.clone();
    let a_back = actions.clone();
    let master = st.master_vol;
    let sfx = st.sfx_vol;
    let music = st.music_vol;
    let tr = &st.translations;
    let lang = &st.language;
    let langs = &st.available_languages;
    let overlay_clone = overlay.clone();
    let actions_clone = actions.clone();

    let menu_state: Rc<MenuState> = remember(MenuState::new);
    let lang_items: Vec<DropdownMenuEntry> = langs
        .iter()
        .map(|l| {
            let a = actions_clone.clone();
            let code = l.clone();
            let mut item = DropdownMenuItem::new(l.clone(), move || {
                push(&a, UiAction::SetLanguage(code.clone()))
            });
            if l == lang {
                item = item.disabled();
            }
            DropdownMenuEntry::Item(item)
        })
        .collect();
    let menu_trigger = menu_state.clone();
    let lang_label = st.language.clone();
    let trigger = FilledTonalButton(
        Modifier::new().width(100.0).height(40.0),
        move || menu_trigger.open(),
        ButtonConfig::default(),
        move || RText(lang_label.clone()).size(20.0),
    );

    let lang_dropdown = DropdownMenu(
        menu_state,
        overlay_clone,
        Modifier::new(),
        trigger,
        lang_items,
        DropdownMenuConfig {
            min_width: 100.0,
            ..Default::default()
        },
    );

    let inner = Column(
        Modifier::new()
            .width(360.0)
            .padding(24.0)
            .background(col(72, 48, 24))
            .border(3.0, col(35, 22, 9), 8.0)
            .clip_rounded(12.0)
            .align_items(AlignItems::CENTER)
            .render_z_index(110.0),
    )
    .child(title_text(t(tr, "settings", "Options")))
    .child(spacer(12.0))
    .child(vol_row(
        &t(tr, "master-volume", "Master"),
        master,
        move |v| push(&a_m_down, UiAction::SetMasterVol(v)),
        move |v| push(&a_m_up, UiAction::SetMasterVol(v)),
    ))
    .child(spacer(8.0))
    .child(vol_row(
        &t(tr, "sfx-volume", "SFX"),
        sfx,
        move |v| push(&a_s_down, UiAction::SetSfxVol(v)),
        move |v| push(&a_s_up, UiAction::SetSfxVol(v)),
    ))
    .child(spacer(8.0))
    .child(vol_row(
        &t(tr, "music-volume", "Music"),
        music,
        move |v| push(&a_mu_down, UiAction::SetMusicVol(v)),
        move |v| push(&a_mu_up, UiAction::SetMusicVol(v)),
    ))
    .child(spacer(8.0))
    .child(
        RText(format!("{}:", t(tr, "language", "Language")))
            .size(18.0)
            .color(RColor::WHITE),
    )
    .child(Row(Modifier::new().gap(6.0)).child(lang_dropdown))
    .child(spacer(16.0))
    .child(Row(Modifier::new().gap(10.0)).child((
        menu_btn(t(tr, "save", "Save"), 130.0, 42.0, move || {
            push(&a_save, UiAction::SaveSettings)
        }),
        menu_btn(t(tr, "back", "Back"), 130.0, 42.0, move || {
            push(&a_back, UiAction::CloseOverlay)
        }),
    )));

    ZStack(Modifier::new().fill_max_size()).child((
        scrim(),
        Center(Modifier::new().fill_max_size()).child(inner),
    ))
}

fn vol_row(
    label: &str,
    value: f32,
    down: impl Fn(f32) + 'static,
    up: impl Fn(f32) + 'static,
) -> View {
    Row(Modifier::new().gap(8.0).align_items(AlignItems::CENTER)).child((
        RText(format!("{label}: {:.0}%", value * 100.0))
            .size(16.0)
            .color(RColor::WHITE)
            .font_weight(FontWeight::BOLD),
        menu_btn("-", 42.0, 34.0, move || down(value - 0.1)),
        menu_btn("+", 42.0, 34.0, move || up(value + 0.1)),
    ))
}
