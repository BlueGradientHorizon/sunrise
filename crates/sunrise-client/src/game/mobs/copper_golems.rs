//! Copper golem constants and role-neutral item/shearing queries.
use super::Game;
use sunrise_items::items::ItemStack;
use sunrise_sim::mobs::{
    Mob,
    transport_items::{held, set_held},
};

/// `ContainerOpenersCounter.CHECK_TICK_DELAY`.
pub(in crate::game) const RECHECK_TICKS: i64 = 5;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// `readyForShearing`: the antenna holds a flower shears take.
    pub(in crate::game) fn golem_shearable(&self, index: usize) -> bool {
        self.mobs[index].saddle_item().is_some_and(|a| {
            self.res
                .items()
                .item_tags
                .get("minecraft:shearable_from_copper_golem")
                .is_some_and(|t| t.contains(&*a.item))
        })
    }
}

/// `Mob.dropPreservedEquipment`: the guaranteed drops in `EquipmentSlot`
/// order, the carried stack (main hand) then the antenna flower (saddle),
/// taken off the golem.
pub(in crate::game) fn golem_preserved_equipment(m: &mut Mob) -> Vec<ItemStack> {
    let mut stacks = Vec::new();
    if let Some(stack) = held(m).cloned() {
        set_held(m, None);
        stacks.push(stack);
    }
    if let Some(antenna) = m.saddle_item() {
        m.set_saddle(None);
        stacks.push(antenna);
    }
    stacks
}
