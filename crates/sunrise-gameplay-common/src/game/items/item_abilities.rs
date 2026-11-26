//! Item abilities (`ItemAbility`): what a stack can do, asked where vanilla checks the item
//! kind for an action. Vanilla's answer comes first, then the parts of the stack's item
//! (`ItemPart::can_perform_action`, NeoForge's `IItemExtension.canPerformAction`).

pub use crate::game::CommonGame;
pub use sunrise_items::items::{ItemInfo, ItemStack};

/// The abilities the game asks, plain case-sensitive names, like a mod's own.
pub mod ability {
    /// A raised item blocks attacks (`blocks_attacks`).
    pub const SHIELD_BLOCK: &str = "shield_block";
    /// Shearing a mob, a bee nest's honeycombs, a dispenser's shears.
    pub const SHEARS_HARVEST: &str = "shears_harvest";
    /// Carving a pumpkin.
    pub const SHEARS_CARVE: &str = "shears_carve";
}

pub trait ItemAbilitiesGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// `ItemStack.canPerformAction(ability)`, once vanilla answered `vanilla`. A stack whose
    /// item has no part for it keeps `vanilla` at the cost of one table lookup, and the stack
    /// is not looked at.
    fn stack_can_perform(&self, stack: &ItemStack, ability: &str, vanilla: bool) -> bool {
        let mut can_perform = vanilla;
        if mod_api::__private::item_listens(mod_api::HookId::CanPerformAction, &stack.item) {
            self.native_can_perform_action(stack, ability, &mut can_perform);
        }
        can_perform
    }

    /// Whether `stack` shears as shears do (`ability`), vanilla's answer being that the item
    /// is the shears.
    fn stack_shears(&self, stack: &ItemStack, ability: &str) -> bool {
        self.stack_can_perform(stack, ability, &*stack.item == "minecraft:shears")
    }

    /// Whether `stack` carries the `blocks_attacks` component (its own, or its item's unless
    /// removed) as `getUseDuration` and the use animation read it, once the stack's item parts
    /// answered `shield_block`.
    fn stack_carries_blocks_attacks(&self, stack: &ItemStack, info: Option<&ItemInfo>) -> bool {
        self.stack_can_perform(
            stack,
            ability::SHIELD_BLOCK,
            stack.carries_component(info, "minecraft:blocks_attacks"),
        )
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> ItemAbilitiesGameExt1<'__sg> for G {}

use crate::game::native_hooks::can_perform_action::GameHooks as _;
