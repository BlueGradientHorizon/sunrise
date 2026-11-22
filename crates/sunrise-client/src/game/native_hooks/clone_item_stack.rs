use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// The block's clone answer for the requesting player, before block-entity data
    /// is copied onto the chosen stack (`handlePickItemFromBlock`).
    #[inline(always)]
    #[allow(clippy::too_many_arguments, reason = "matches the hook declaration")]
    pub(in crate::game) fn native_clone_item_stack(
        &self,
        dimension: LevelDimension,
        pos: BlockPos,
        block: &str,
        state: BlockStateId,
        include_data: bool,
        player: PlayerRef,
        stack: &mut Option<ItemStack>,
    ) {
        self.fire_clone_item_stack(dimension, pos, block, state, include_data, player, stack);
    }
}
