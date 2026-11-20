impl Game<'_> {
    /// The stack physically resident in the level in place, without the tick proposal.
    pub(super) fn unscoped_item_entity_stack(&self, id: u32) -> Option<&ItemStack> {
        self.dropped
            .iter()
            .find(|entity| entity.id == id)
            .map(|entity| &entity.stack)
    }

    #[cold]
    #[inline(never)]
    pub(super) fn scoped_item_entity_stack(&self, id: u32) -> Option<&ItemStack> {
        let live = self.unscoped_item_entity_stack(id)?;
        // `Some(None)` is a removal or a no-longer-resident proposal, not a live read.
        self.tick_item_view(id).unwrap_or(Some(live))
    }
}
