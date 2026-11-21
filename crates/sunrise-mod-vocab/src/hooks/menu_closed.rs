//! The `menu_closed` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0035

pub trait Hooks {
    /// A menu a mod opened closed: the player closed it, the server or a mod
    /// opened another, or the player left.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_menu_closed",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        neoforge = "PlayerContainerEvent.Close"
    )]
    fn menu_closed(uuid: u128, window: i32);
}
