//! The `menu_button` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0031

pub trait Hooks {
    /// A player pressed a mod menu's button (`clickMenuButton`), on the
    /// server. `entity` is the block entity backing the menu when there is
    /// one, out of the world during the call; it is None for a blockless
    /// menu. Mods may change it and the clicking player's carried stack.
    #[hook(
        side = "server",
        contract = "listen",
        scope = "menu",
        key = "menu",
        neoforge = "none (NeoForge has no such event; vanilla AbstractContainerMenu.clickMenuButton)"
    )]
    fn menu_button();
}
