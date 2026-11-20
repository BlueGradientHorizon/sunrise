//! The `menu_button_valid` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Whether this menu's owner accepts a button id before `menu_button` runs.
    /// The owner chooses the ids for its own menu; there is no shared numeric
    /// range. Leave `accepted` false for ids the active menu does not support.
    #[hook(
        side = "server",
        contract = "cancel",
        out = "accepted",
        scope = "menu",
        key = "menu",
        host = "read",
        neoforge = "none (vanilla menu button ids are handled by their menu implementation)"
    )]
    fn menu_button_valid();
}
