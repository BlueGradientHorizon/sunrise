//! The `client_services` declarations reconciled with the fork API.

pub trait Hooks {
    /// Before an actual connection is discarded, not on a dimension change.
    #[notes = r#"## Foreground GUI and client lifecycle

`Hooks::menu_background` can adjust the requested blur radius or set it to
`None` before the game validates it.

`Hooks::entity_outline` receives the entity id, kind, and mutable optional RGB
outline for the current client render. Leave the existing value unchanged to
preserve vanilla glowing; `None` disables it. These read-only client hooks
receive `&dyn ClientHost`. They do not grant server writes.

`Hooks::client_disconnecting` runs before an actual connection is discarded,
not for a dimension change. Initialization and final shutdown still use the
existing `client_setup` and `client_stopping` hooks, with no parallel aliases."#]
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "ClientPlayerNetworkEvent.LoggingOut"
    )]
    fn client_disconnecting();
    /// The normal menu blur request, before finite/range validation. None disables it.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "none (NeoForge has no menu blur event)"
    )]
    fn menu_background(radius: &mut Option<f32>);
}
