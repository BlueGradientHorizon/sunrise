use super::*;

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    /// The block at `pos` of `dimension` went from `old` to `new` in this client's level, from the
    /// server, a prediction (`predicted`) or its reconciliation. Client-side.
    pub(in crate::game) fn native_client_block_changed(
        &self,
        dimension: &str,
        pos: BlockPos,
        old: BlockStateId,
        new: BlockStateId,
        predicted: bool,
    ) {
        self.fire_client_block_changed(dimension, pos, old, new, predicted);
    }
}
