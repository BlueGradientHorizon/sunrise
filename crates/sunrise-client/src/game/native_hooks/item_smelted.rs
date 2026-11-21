use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// `FurnaceResultSlot.checkTakeAchievements` ran for a server player at a
    /// furnace, blast furnace or smoker's result slot with a nonzero
    /// `removeCount` (NeoForge `PlayerEvent.ItemSmeltedEvent`): `player` is
    /// the live player handle, `stack` lends the stack vanilla passed there
    /// (`getSmelting`), and `amount_removed` is `removeCount`
    /// (`getAmountRemoved`). Fires after the furnace awards its used recipes
    /// and pops experience (`AbstractFurnaceBlockEntity.awardUsedRecipesAndPopExperience`).
    /// Server-side.
    pub(in crate::game) fn native_item_smelted(
        &mut self,
        call: mod_api::__private::ItemSmeltedCall<'_>,
    ) {
        self.fire_item_smelted(call);
    }
}
