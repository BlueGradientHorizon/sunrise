//! The `left_click_empty` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// The local player left-clicks empty space, with any main-hand stack.
    /// Client-side only; the server does not receive this event.
    #[hook(
        target = "sunrise_client_gameplay::game::native_hooks::left_click_empty::ClientGame<'_>::native_left_click_empty",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "PlayerInteractEvent.LeftClickEmpty",
        cancellable = false
    )]
    fn left_click_empty();
}
