//! Moved here from the game so mods can name it.
#[allow(unused_imports)]
use crate::*;
/// One click on the local player's open menu, submitted with `MenuRef::click`.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
pub enum MenuClick {
    /// Pick up, place or drop the carried stack; `None` means outside.
    Pickup {
        slot: Option<usize>,
        button: MouseButton,
    },
    /// Shift-click a menu-order slot into the opposite inventory.
    QuickMove { slot: usize },
    /// Exchange a menu-order slot with a hotbar slot or the offhand.
    Swap { slot: usize, with: SwapWith },
    /// Clone a menu-order slot in creative mode.
    Clone { slot: usize },
    /// Drop one item or the whole stack from a menu-order slot.
    Throw { slot: usize, whole_stack: bool },
    /// Distribute the carried stack across menu-order slots as one complete gesture.
    Drag { kind: DragKind, slots: Vec<usize> },
    /// Collect matching stacks into the carried stack.
    PickupAll { slot: usize },
}

/// The mouse button used to pick up or place a stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two vanilla pickup buttons
#[derive(sunrise_mod_api_macros::Wire)]
pub enum MouseButton {
    Left,
    Right,
}

/// The inventory location exchanged with a menu slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: vanilla swaps target the hotbar or the offhand
#[derive(sunrise_mod_api_macros::Wire)]
pub enum SwapWith {
    /// A hotbar index in 0..=8; other indices refuse the entire batch.
    Hotbar(u8),
    Offhand,
}

/// Whether [`PlayerRef::open_chest_menu`] or [`PlayerRef::open_mod_menu`] first sends the client a
/// close for the menu it replaces, as a Java `MenuProvider` chooses
/// (`shouldTriggerClientSideContainerClosingOnOpen`). Either way the old menu closes on the
/// server, its cursor stack going back to the player's inventory and `player_container_close`
/// firing once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: vanilla's choice is a boolean
#[derive(sunrise_mod_api_macros::Wire)]
pub enum MenuReplace {
    /// `ServerPlayer.closeContainer`, vanilla's default: the client closes the old screen first,
    /// which centres the mouse again.
    CloseFirst,
    /// `ServerPlayer.doCloseContainer`: the new screen takes the old one's place, title and rows
    /// included, and the mouse stays where it is.
    InPlace,
}

/// How a complete drag distributes the carried stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: vanilla has split, one-per-slot and creative-fill drags
#[derive(sunrise_mod_api_macros::Wire)]
pub enum DragKind {
    Split,
    One,
    /// Fill each slot in creative mode only.
    Clone,
}

/// Identity and location of a player pressing a mod container button.
// exhaustive: the game builds it with a literal; mods only read it
#[derive(sunrise_mod_api_macros::Wire)]
pub struct MenuButtonContext {
    pub player: u128,
    pub player_entity_id: i32,
    /// The block backing the menu, or None when the menu was opened without a block.
    pub pos: Option<[i32; 3]>,
    pub id: i32,
}

#[cfg(test)]
mod menu_button_context_tests {
    use super::MenuButtonContext;
    use crate::wire::{Reader, Writer};

    #[test]
    fn menu_button_context_round_trips_with_and_without_a_block_position() {
        for pos in [None, Some([12, -4, 29])] {
            let context = MenuButtonContext {
                player: 0x1234,
                player_entity_id: 57,
                pos,
                id: 8,
            };
            let mut writer = Writer::new();
            writer.put(&context);
            let decoded = Reader::decode::<MenuButtonContext>(writer.bytes()).unwrap();
            assert_eq!(decoded.player, context.player);
            assert_eq!(decoded.player_entity_id, context.player_entity_id);
            assert_eq!(decoded.pos, context.pos);
            assert_eq!(decoded.id, context.id);
        }
    }
}

/// The logical side on which a menu click runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a closed set the game matches without a wildcard
#[derive(sunrise_mod_api_macros::Wire)]
pub enum LogicalSide {
    Client,
    Server,
}
/// Which phase of a pickup click (`AbstractContainerMenu.tryItemClickBehaviourOverride`) is
/// asking `item_stacked_on_other`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: NeoForge's event, then vanilla's carried-on-other and other-on-me overrides
pub enum StackedOnOtherDirection {
    /// The global `Hooks::item_stacked_on_other` event, before either item override.
    #[default]
    Event,
    /// `Item.overrideStackedOnOther`: the carried (cursor) stack's `ItemPart`, asked first.
    CarriedOnOther,
    /// `Item.overrideOtherStackedOnMe`: the clicked slot stack's `ItemPart`, asked second.
    OtherStackedOnMe,
}
/// Vanilla's `ClickAction`: which action a pickup click is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: vanilla's two click actions
pub enum ClickAction {
    /// `ClickAction.PRIMARY`: with the default bindings, a left click.
    Primary,
    /// `ClickAction.SECONDARY`: with the default bindings, a right click.
    Secondary,
}
/// One edit a listener made to a pickup click's cursor or clicked slot, in the order it made it.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: a cursor write, a slot write, or a take from the slot
// no wire: recorded and applied in-process within one click.
pub enum StackedEdit {
    /// `SlotAccess.set` on the cursor.
    Carried(Option<ItemStack>),
    /// `Slot.set` on the clicked slot.
    Clicked(Option<ItemStack>),
    /// `Slot.safeTake` of this many items from the clicked slot.
    Take(u32),
}
/// The cursor and the clicked slot as one pickup click's listeners have left them so far, and
/// the edits that put them there, in order. Every phase of the click shares it.
#[derive(Debug, Default)]
// exhaustive: the game builds it and reads it back
// no wire: lent to listeners within one click.
pub struct StackedEdits {
    carried: Option<ItemStack>,
    clicked: Option<ItemStack>,
    edits: Vec<StackedEdit>,
}
impl StackedEdits {
    /// The click's cursor and clicked slot before any listener ran.
    // hidden: a game-side helper, not part of the mod surface
    #[doc(hidden)]
    pub fn new(carried: Option<ItemStack>, clicked: Option<ItemStack>) -> Self {
        Self {
            carried,
            clicked,
            edits: Vec::new(),
        }
    }
    /// The cursor and clicked slot now, and how many edits have been made.
    // hidden: a game-side helper, not part of the mod surface
    #[doc(hidden)]
    pub fn state(&self) -> (Option<&ItemStack>, Option<&ItemStack>, usize) {
        (
            self.carried.as_ref(),
            self.clicked.as_ref(),
            self.edits.len(),
        )
    }
    /// The edits made since edit `from`.
    // hidden: a game-side helper, not part of the mod surface
    #[doc(hidden)]
    pub fn since(&self, from: usize) -> &[StackedEdit] {
        &self.edits[from..]
    }
    /// Every edit, in the order the listeners made them.
    // hidden: a game-side helper, not part of the mod surface
    #[doc(hidden)]
    pub fn into_edits(self) -> Vec<StackedEdit> {
        self.edits
    }
}
/// A pickup click on a menu slot, as `item_stacked_on_other` is asked about it, with the cursor
/// and the clicked slot lent for this call (NeoForge's `getCarriedSlotAccess` and `getSlot`).
/// Reads see what earlier phases and this call already changed. Edits are applied to the menu in
/// the order they were made, once the click is decided.
#[derive(Debug)]
#[non_exhaustive]
// no wire: borrows the click's working stacks for the hook call.
pub struct StackedOnOther<'a> {
    /// The clicking player's UUID, when a player is clicking.
    pub player: Option<u128>,
    /// The clicking player's entity id, when a player is clicking.
    pub player_entity_id: Option<i32>,
    /// The clicked slot's menu index.
    pub slot: usize,
    /// Which container the slot is in, as the menu numbers them.
    pub container: usize,
    /// The slot's index in that container.
    pub container_slot: usize,
    /// The click action (`ItemStackedOnOtherEvent.getClickAction`).
    pub action: ClickAction,
    /// Which phase of `tryItemClickBehaviourOverride` is firing: the global event, the carried
    /// stack's `ItemPart` (`CarriedOnOther`), or the clicked stack's `ItemPart`
    /// (`OtherStackedOnMe`).
    pub direction: StackedOnOtherDirection,
    /// `ItemStackedOnOtherEvent.setCancellationResult`: on a cancelled click, `true` (the start
    /// value, as in NeoForge) when the listener's work is all of it, `false` when plain pickup
    /// still runs.
    pub handled: bool,
    edits: &'a mut StackedEdits,
}
impl<'a> StackedOnOther<'a> {
    /// The click `slot` (menu index) of `container`'s slot `container_slot`, lending `edits`.
    // hidden: a game-side helper, not part of the mod surface
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        player: Option<u128>,
        player_entity_id: Option<i32>,
        slot: usize,
        container: usize,
        container_slot: usize,
        action: ClickAction,
        direction: StackedOnOtherDirection,
        edits: &'a mut StackedEdits,
    ) -> Self {
        Self {
            player,
            player_entity_id,
            slot,
            container,
            container_slot,
            action,
            direction,
            handled: true,
            edits,
        }
    }
    /// The working stacks and the number of edits, for undoing a panicking listener's edits.
    // hidden: the generated dispatch's unwind checkpoint, not part of the mod surface
    #[doc(hidden)]
    pub fn checkpoint(&self) -> (Option<ItemStack>, Option<ItemStack>, usize) {
        let edits = &*self.edits;
        (
            edits.carried.clone(),
            edits.clicked.clone(),
            edits.edits.len(),
        )
    }
    /// Undo the edits made since `checkpoint`.
    // hidden: the generated dispatch's unwind restore, not part of the mod surface
    #[doc(hidden)]
    pub fn rollback(
        &mut self,
        (carried, clicked, len): (Option<ItemStack>, Option<ItemStack>, usize),
    ) {
        self.edits.carried = carried;
        self.edits.clicked = clicked;
        self.edits.edits.truncate(len);
    }
    /// The stack on the cursor, `None` when it is empty (`SlotAccess.get`).
    pub fn carried(&self) -> Option<&ItemStack> {
        self.edits.carried.as_ref()
    }
    /// Put `stack` on the cursor (`SlotAccess.set`). It must be a valid stack: a known item and
    /// a count from 1 to the item's maximum, or the click is refused.
    pub fn set_carried(&mut self, stack: Option<ItemStack>) {
        self.edits.carried = stack.clone();
        self.edits.edits.push(StackedEdit::Carried(stack));
    }
    /// The stack in the clicked slot, `None` when it is empty (`Slot.getItem`).
    pub fn clicked(&self) -> Option<&ItemStack> {
        self.edits.clicked.as_ref()
    }
    /// Write the clicked slot directly (`Slot.set`), without the slot's take side effects:
    /// `None` clears it. The slot's rules still answer: a write they refuse, or an invalid
    /// stack, refuses the click.
    pub fn set_clicked(&mut self, stack: Option<ItemStack>) {
        self.edits.clicked = stack.clone();
        self.edits.edits.push(StackedEdit::Clicked(stack));
    }
    /// Take `count` items from the clicked slot through `Slot.safeTake`, which runs the slot's
    /// take side effects (furnace experience, `recipes_used`, crafting remainders, take hooks)
    /// when the click applies. Returns the stack taken, or `None` (nothing recorded) when the
    /// slot is empty, `count` is 0 or more than it holds. Only `ItemPart` overrides may take: a
    /// global listener's take refuses the click.
    pub fn take_clicked(&mut self, count: u32) -> Option<ItemStack> {
        let current = self
            .edits
            .clicked
            .take_if(|current| (1..=current.count()).contains(&count))?;
        let split = current.split(count);
        self.edits.clicked = split.remainder;
        self.edits.edits.push(StackedEdit::Take(count));
        split.taken
    }
}
/// Whether a listener keeps the original stack or replaces it.
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: a stack is left alone or replaced, nothing else
#[derive(sunrise_mod_api_macros::Wire)]
pub enum StackResult {
    /// Leave the original stack unchanged.
    #[default]
    Keep,
    /// Replace the stack: `None` empties it.
    Set(Option<ItemStack>),
}
/// The inputs and vanilla result of `AnvilMenu.createResult`, borrowed for
/// the `anvil_update` callback. Edits by earlier listeners do not change these values.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the game builds it with a literal; mods only read it
// no wire: borrows the converted stacks and rename text for the hook call.
pub struct AnvilUpdateInputs<'a> {
    /// The stack in the left input slot. The hook is not asked without one.
    pub left: &'a ItemStack,
    /// The stack in the right input slot, if present.
    pub right: Option<&'a ItemStack>,
    /// The rename box's text, `None` if the client sent none.
    pub name: Option<&'a str>,
    /// The result calculated by vanilla, if any.
    pub vanilla_output: Option<&'a ItemStack>,
    /// The level cost calculated by vanilla.
    pub vanilla_xp_cost: i32,
    /// The number of material items vanilla would charge from the right slot.
    pub vanilla_material_cost: i32,
}

/// The shared answer to `anvil_update`. It starts at `Keep` and vanilla's costs.
/// The game applies the final edits after dispatch; cancellation leaves no output
/// and zero costs, while an invalid replacement output is logged and ignored
/// (keeping vanilla's output).
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the game builds it with a literal
// no wire: staged edits are applied in-process at the end of the hook call.
pub struct AnvilUpdateEdits {
    /// Keep vanilla's output, or set a replacement. `Set(None)` clears it.
    /// A replacement differing from vanilla must be a valid stack; an invalid
    /// stack is logged and ignored, leaving vanilla's output intact.
    pub output: StackResult,
    /// Signed level cost during listener dispatch (`AnvilUpdateEvent.setXpCost`).
    /// Negative values clamp to `0` when applied (`AnvilMenu.setCost`), and a
    /// zero cost prevents taking the result (`AnvilMenu.mayPickup`).
    pub xp_cost: i32,
    /// Units consumed from the right slot. Negative values clamp to zero;
    /// zero or a value exceeding the stack consumes all of it on take.
    pub material_cost: i32,
}
/// The inputs and vanilla result of `GrindstoneMenu.createResult`, borrowed for
/// the `grindstone_place` callback. Edits by earlier listeners do not change these values.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the game builds it with a literal; mods only read it
// no wire: borrows the converted stacks for the hook call.
pub struct GrindstonePlaceInputs<'a> {
    /// The stack in the top input slot, if present.
    pub top: Option<&'a ItemStack>,
    /// The stack in the bottom input slot, if present.
    pub bottom: Option<&'a ItemStack>,
    /// The result calculated by vanilla, if any.
    pub vanilla_output: Option<&'a ItemStack>,
}

/// The shared answer to `grindstone_place`, initially no override and `-1` XP.
/// Cancellation is checked before converting `output`; an invalid replacement is logged
/// and ignored so vanilla's result stays intact.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the game builds it with a literal
// no wire: staged edits are applied in-process at the end of the hook call.
pub struct GrindstonePlaceEdits {
    /// Leave vanilla's result and experience with `None`, or replace the result with a
    /// valid stack. To clear the result, cancel the hook instead.
    pub output: Option<ItemStack>,
    /// Experience to award when the replacement is taken. Any negative value falls
    /// through to vanilla's award; nonnegative values set the award.
    pub xp: i32,
}
/// The input stacks before the grindstone result was taken, borrowed for the
/// `grindstone_take` callback. Edits by earlier listeners do not change this snapshot.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the game builds it with a literal; mods only read it
// no wire: borrows the converted pre-take stacks for the hook call.
pub struct GrindstoneTakeInputs<'a> {
    /// The stack in the top input slot before the take, if present.
    pub top: Option<&'a ItemStack>,
    /// The stack in the bottom input slot before the take, if present.
    pub bottom: Option<&'a ItemStack>,
}

/// The shared answer to `grindstone_take`. Both remainders start at `None`;
/// cancellation is checked before converting remainders, and an invalid remainder is
/// logged and ignored as an empty slot.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the game builds it with a literal
// no wire: staged edits are applied in-process at the end of the hook call.
pub struct GrindstoneTakeEdits {
    /// Experience to award, initially the resolved award (`>= 0`, from `grindstone_place`
    /// or vanilla's pre-rolled award). Non-positive values (`<= 0`) spawn no experience orbs.
    pub xp: i32,
    /// A valid replacement for the top input, or `None` to empty the slot.
    pub new_top: Option<ItemStack>,
    /// A valid replacement for the bottom input, or `None` to empty the slot.
    pub new_bottom: Option<ItemStack>,
}
/// The anvil inputs and result before the take. Both `anvil_craft_pre` and
/// `anvil_craft_post` borrow this same snapshot for their callback.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the game builds it with a literal; mods only read it
// no wire: borrows the pre-take snapshot for this hook call.
pub struct AnvilCraftInputs<'a> {
    /// The stack in the left slot before the take, if present.
    pub left: Option<&'a ItemStack>,
    /// The stack in the right slot before the take, if present.
    pub right: Option<&'a ItemStack>,
    /// The result stack that was taken.
    pub output: &'a ItemStack,
}

/// One of a menu's containers: the key a `menu_slot_rules` ask names its slot's container by
/// (`event.container`, with `event.slot` indexing inside it) and [`MenuSlotContext::slot`] reads
/// by. A plain menu has `Main` and `Player`; a mod menu type with a crafting grid keeps its grid
/// slots in `Grid` and its crafted result in `Result` (slot 0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum MenuContainer {
    /// The menu's own slots (a chest's, a machine's).
    #[default]
    Main,
    /// A crafting grid's slots, row by row.
    Grid,
    /// A crafting grid's result slot.
    Result,
    /// The player's inventory, using vanilla `Inventory` indices: `0..36` inventory, `36..40`
    /// armor (feet, legs, chest, head) and `40` offhand.
    Player,
}
/// The menu's player, its block entity and a read-only view of its live slots for one
/// `menu_slot_rules` ask, on either the server or the predicting client. Read only the slots
/// needed to answer the rule; the view cannot outlive the ask.
// no wire: this view borrows the menu's live slots during a click.
#[non_exhaustive]
pub struct MenuSlotContext<'a> {
    /// The UUID of the player whose menu it is; None where no player is known.
    pub player: Option<u128>,
    /// The block entity the menu belongs to; None when the menu is not a block's
    /// (opened by a mod with no block, a vehicle's, the player's own inventory).
    pub pos: Option<[i32; 3]>,
    read: &'a dyn Fn(MenuContainer, usize) -> Option<ItemStack>,
}
impl<'a> MenuSlotContext<'a> {
    /// Lend a read of the menu's live slots, with the player and block position of this ask.
    /// The reader returns `None` for empty, absent or unrepresentable slots.
    pub fn new(
        player: Option<u128>,
        pos: Option<[i32; 3]>,
        read: &'a dyn Fn(MenuContainer, usize) -> Option<ItemStack>,
    ) -> Self {
        Self { player, pos, read }
    }

    /// Read one slot at its backing index, converting just that stack to an API value.
    /// Returns `None` if it is empty, absent or cannot be represented. For `Player`, indices follow
    /// vanilla `Inventory`: `0..36` inventory, `36..40` armor (feet-to-head) and `40` offhand.
    pub fn slot(&self, container: MenuContainer, index: usize) -> Option<ItemStack> {
        (self.read)(container, index)
    }
}
