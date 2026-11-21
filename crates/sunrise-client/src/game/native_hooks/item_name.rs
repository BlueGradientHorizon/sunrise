use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// `Item.getName(ItemStack)`: the name the parts of `stack`'s item give
    /// it. `name` arrives as `None` (the default name); a part sets it.
    /// Both sides.
    pub(in crate::game) fn native_item_name(&self, stack: &ItemStack, name: &mut Option<Text>) {
        self.fire_item_name(stack, name);
    }
}
