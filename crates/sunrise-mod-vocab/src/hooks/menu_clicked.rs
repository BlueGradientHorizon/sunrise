//! The `menu_clicked` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0034

pub trait Hooks {
    /// A click in `menu`, before its default action (`ServerGamePacketListenerImpl.handleContainerClick`,
    /// `AbstractContainerMenu.clicked`). Only parts registered for that menu type are asked.
    /// `slot` is in menu order (container slots, then any player inventory; -999 outside),
    /// `button` and `mode` are the click input, and `uuid` identifies the clicking player.
    /// Menu-button clicks, recipe placement and the creative set-slot packet do not call it.
    /// Return [`Flow::Cancel`] after applying the click yourself to skip the default: for
    /// example, copy an item into a filter slot without consuming the cursor.
    /// A display-only menu has no default moves. Other menus keep their ordinary slot rules
    /// when unhandled. The client predicts the ordinary click and the server corrects it.
    /// No player's state is swapped in. Use their current `PlayerRef::container_menu` or the
    /// native menu's UUID and window to reach its slots and cursor.
    /// Runs directly on the receiving menu without a synthetic decided-action wrapper or
    /// whole-inventory rollback: open a transaction explicitly with [`ServerHost::transaction`]
    /// when a callback wants atomic multi-step edits, and prior unrelated writes stand even if
    /// the default click is cancelled or refused. On [`Flow::Continue`], the server rechecks
    /// receiver preconditions (live player, same menu generation and window, non-spectator,
    /// valid backing slots and slot index) before running default click execution on the live
    /// menu; closing or replacing the menu inside the callback skips the default click.
    /// A panic in a part's `menu_clicked` callback is isolated: any transaction opened by that
    /// callback unwinds and aborts, only that mod's `menu_clicked` callback is disabled for the
    /// session with an error log and error toast naming the mod, and remaining parts continue.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_menu_clicked",
        dispatch = "api_carrier",
        side = "server",
        contract = "cancel",
        scope = "menu",
        key = "menu",
        out = "handled",
        cancellable = true,
        isolate = true,
        neoforge = "none (a Java menu overrides AbstractContainerMenu.clicked; no event)",
        version = 4
    )]
    fn menu_clicked(
        menu: &str,
        uuid: u128,
        window: i32,
        slot: i32,
        button: i32,
        mode: i32,
        handled: &mut bool,
    );
}
