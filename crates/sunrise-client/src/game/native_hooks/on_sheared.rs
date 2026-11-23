use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// `IShearable.onSheared` on the single entity part at `part_slot` whose `is_shearable`
    /// answered `Some(true)`. Server-side.
    pub(in crate::game) fn native_on_sheared(
        &mut self,
        entity: EntityRef,
        player: Option<PlayerRef>,
        shears: mod_api::ItemStackView<'_>,
        drops: &mut Vec<mod_api::ItemStack>,
        part_slot: (usize, usize),
        entity_kind: &str,
    ) -> Result<(), ()> {
        mod_api::dispatch_on_sheared(self, entity, player, shears, drops, part_slot, entity_kind)
    }
}
