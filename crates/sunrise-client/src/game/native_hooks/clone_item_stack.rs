use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// The block's clone answer for the requesting player, before block-entity data
    /// is copied onto the chosen stack (`handlePickItemFromBlock`).
    #[inline(always)]
    pub(in crate::game) fn native_clone_item_stack(
        &self,
        call: super::fire::CloneItemStackCall<'_>,
    ) {
        self.fire_clone_item_stack(call);
    }
}
