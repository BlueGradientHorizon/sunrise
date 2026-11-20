//! The `menu_clicked` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0034

pub trait Hooks {
    /// A click in a mod menu, before its default action (`AbstractContainerMenu.clicked`).
    /// `slot` is in menu order (container slots, then any player inventory; -999 outside),
    /// `button` and `mode` are the click input, and `uuid` identifies the clicking player.
    /// Menu-button clicks, recipe placement and the creative set-slot packet do not call it.
    /// Return [`Flow::Cancel`] after applying the click yourself to skip the default: for
    /// example, copy an item into a filter slot without consuming the cursor.
    /// A display-only menu has no default moves. Other menus keep their ordinary slot rules
    /// when unhandled. The client predicts the ordinary click and the server corrects it.
    /// No player's state is swapped in. Use their current `PlayerRef::container_menu` or the
    /// native menu's UUID and window to reach its slots and cursor. Host writes are kept if
    /// the action succeeds; a failed or busy action skips the default and leaves no writes.
    #[hook(
        side = "server",
        contract = "cancel",
        out = "handled",
        cancellable = true,
        neoforge = "none (a Java menu overrides AbstractContainerMenu.clicked; no event)",
        version = 3
    )]
    fn menu_clicked();
}
