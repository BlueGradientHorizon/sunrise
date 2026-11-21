//! The `menu_quick_move` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0235

pub trait Hooks {
    /// A player shift-clicked slot `slot` of the menu of the mod block
    /// entity at `pos` (`AbstractContainerMenu.quickMoveStack`, the method a
    /// Java menu overrides), on the server, before the game's own routing:
    /// move the stack where your block wants it. `slot` is the menu's index:
    /// below `container_slots` it is a container slot, from there on the
    /// player's inventory (27 main slots, then the hotbar). A container slot
    /// can belong to `entity.items`, a linked container, or the menu's crafting result.
    /// `stack` copies what the live slot holds, after `Slot.mayPickup` allows the click.
    /// Take what you move out of `stack` and set `handled`: the same source keeps
    /// the remainder (`None` empties it), the game's routing is skipped, and the
    /// player's client gets the menu again. For a linked or crafting-result source,
    /// the remainder does not replace any slot in `entity.items`. A crafting result
    /// is not a craft by itself: the handler is responsible for consuming any inputs.
    /// The remainder is the shown result for the inputs the handler leaves, until
    /// the grid next changes and prepares a new result. Pickup uses that shown result,
    /// not a fresh recipe assembly.
    /// Left unhandled, listener writes are undone and the same menu routes the move
    /// without asking `Slot.mayPickup` again. A later listener sees earlier edits to `stack`.
    ///
    /// The move is one action: the answer is validated once and applied once,
    /// and a refused move changes nothing, with every write the listener made
    /// undone and the client given the menu again. It is refused when a
    /// listener fails (panics or traps), when the owner or clicked stack cannot be
    /// read by the API (reported each time, without calling the handler), when the answer is invalid (a stack
    /// past its item's maximum in `entity.items` or `stack`), or when a
    /// listener wrote through the `Host` to a player's inventory, cursor or
    /// menu storage (the move read its own player's; a write to any player's
    /// counts). Writes to another block, as a move into its `ITEMS`
    /// capability makes, do not make it stale.
    ///
    /// For a mod container on a vanilla menu type and on a mod menu type,
    /// the left or right button, a slot that holds something and that the
    /// mod's screen shows; not for a spectator or a dead player, a menu no
    /// longer valid, nor during a drag.
    /// Asked once per click, where vanilla asks `quickMoveStack` again while
    /// it keeps moving the same item. The player's state is swapped in, so
    /// `host.players().acting_player()` names them; `entity` is out of the world
    /// during the call, as in `menu_button`.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_menu_quick_move",
        dispatch = "flat",
        side = "server",
        contract = "cancel",
        out = "handled",
        scope = "block_entity",
        key = "entity",
        neoforge = "none (a Java menu overrides AbstractContainerMenu.quickMoveStack; no event)",
        cancellable = true,
        version = 4
    )]
    fn menu_quick_move(
        pos: [i32; 3],
        entity: &mut ModBlockEntity,
        container_slots: usize,
        slot: usize,
        stack: &mut Option<mod_api::ItemStack>,
        handled: &mut bool,
    );
}
