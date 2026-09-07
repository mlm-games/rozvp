//! Gameplay actions: Escape / pad-Start pauses, gated by phase contexts.
//! Pointer clicks keep flowing as positions through `ClickQueue`; this
//! covers button-like intents with edges (`just_pressed`).

use repame_input::{ActionMap, Binding, Chord};
use repose_core::input::{GamepadButton, Key, Modifiers};

/// Gameplay intents driven by hardware input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PilotAction {
    /// Toggle pause while in game (Escape, pad Start).
    PauseToggle,
}

/// Active while playing a level (no modal overlay).
pub const CTX_GAMEPLAY: &str = "gameplay";
/// Active in menus, title, and modal overlays.
pub const CTX_MENU: &str = "menu";

/// Escape on keyboard, Start on pad, live in both contexts (pause and
/// unpause take the same key). Pad feed needs a repose runtime hook
/// (hardware polling here would drain repose's own backend queue);
/// until then the key binding carries it.
pub fn action_map() -> ActionMap<PilotAction> {
    let mut map = ActionMap::new();
    map.bind(
        PilotAction::PauseToggle,
        Binding::Key(Chord::new(Key::Escape, Modifiers::default())),
    );
    map.bind(PilotAction::PauseToggle, Binding::Pad(GamepadButton::Start));
    map.in_context(CTX_GAMEPLAY, PilotAction::PauseToggle);
    map.in_context(CTX_MENU, PilotAction::PauseToggle);
    map
}
