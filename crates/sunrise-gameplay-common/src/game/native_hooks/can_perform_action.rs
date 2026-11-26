use super::*;

/// The hook points. Each does nothing in Sunrise.
pub trait GameHooks<'__sg>: crate::game::SideGame<'__sg> {
    /// `IItemExtension.canPerformAction`: can `stack` perform the item ability
    /// `ability` (`shield_block`, `shears_harvest`...)?
    /// `can_perform` arrives with vanilla's answer; the parts of the stack's
    /// item may set it. Both sides.
    #[inline(never)]
    fn native_can_perform_action(&self, stack: &ItemStack, ability: &str, can_perform: &mut bool) {
        crate::game::native_hooks::fire::FireHooks::fire_can_perform_action(
            self,
            stack,
            ability,
            can_perform,
        );
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> GameHooks<'__sg> for G {}
