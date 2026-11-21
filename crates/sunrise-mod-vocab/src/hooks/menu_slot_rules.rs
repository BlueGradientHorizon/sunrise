//! The `menu_slot_rules` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0131

pub trait Hooks {
    /// Slot placement, pickup and stack limits for registered vanilla and mod
    /// menus; mounts use `sunrise:mount_inventory`, while player inventory and
    /// the creative picker have no registered type. `slot` is the backing index,
    /// including player inventory slots. The incoming `may_place` includes the
    /// backing container's declared admission; a Part cannot reopen a slot the
    /// backing refuses. Backing admission also applies to quick-move top-ups:
    /// those already refused by the backing are not asked. The maximum is
    /// bounded by the slot and the offered stack's component-aware item limit.
    /// This is a read-only query, including for a LAN guest on the server.
    /// Click planning reuses an answer for the same slot, stack and base rules;
    /// validation and application do not ask again. The client only runs the
    /// query when it can predict the rules of the opening. A declared mod menu
    /// with server-owned backing sends clicks unpredicted; the server owns the
    /// slot rules and sends back the resulting slots and cursor.
    /// `container` is the container `slot` indexes ([`MenuContainer`]): a mod
    /// crafting grid's slots are `Grid`'s and its result `Result`'s slot 0.
    /// `context` says who and where the ask is about ([`MenuSlotContext`]):
    /// the player whose menu it is, the menu's block entity and a read-only
    /// copy of the menu's own containers' slots, keyed by the same container
    /// (`context.slot(container, index)`), built once per click plan. On the
    /// client, only data synchronized for the modeled menu is available.
    ///
    /// `may_pickup` is `Slot.mayPickup`, answered only for a registered mod
    /// menu type (its own slots and the player's inventory slots in it); a
    /// vanilla menu type keeps its slots' own answer. It is asked for the
    /// stack the slot holds, wherever the click would take from it: a pickup
    /// click on a slot that holds an item (after `item_stacked_on_other`,
    /// before any take, merge or exchange), quick move (once, on the clicked
    /// slot, before the move; moving into a slot asks only `may_place`), a
    /// hotbar or offhand swap that takes the slot's item (with both stacks
    /// present `may_place` must hold too), throw (inside the take, so a refusal
    /// drops nothing and stops the repeat), pick-all (it gathers when the
    /// clicked slot is empty or refuses, and skips each source that refuses),
    /// and every take the menu makes. A drag spread, a creative clone, an
    /// insertion into an empty slot and a click outside the window never ask
    /// it; a drag that ends on one slot is a pickup click on it. A refusal
    /// leaves the slot and the cursor as they were (nothing is taken, swapped
    /// or merged) and the server corrects the client's prediction. A slot that
    /// refuses placement gives all of its stack or none. A display-only slot
    /// answers `may_pickup` false and `may_place` false. `may_pickup` arrives
    /// true: the slot's own rule comes first, and a slot it already refuses is
    /// not asked; then the mods, in the order `may_place` is asked, one ask
    /// answering both. A shift-click a part answers with `menu_quick_move` is
    /// asked the pickup rule before that hook.
    #[hook(
        receiver = "read",
        dispatch = "api_carrier",
        side = "both",
        contract = "listen",
        scope = "menu",
        key = "menu",
        host = "read",
        cancellable = false,
        version = 6,
        neoforge = "none (vanilla Slot.mayPlace and mayPickup; NeoForge adds no event or extension for it)"
    )]
    fn menu_slot_rules(
        menu: &str,
        slot: usize,
        container: MenuContainer,
        stack: &ItemStack,
        context: &MenuSlotContext,
        may_place: &mut bool,
        may_pickup: &mut bool,
        max_stack: &mut u32,
    );
}
