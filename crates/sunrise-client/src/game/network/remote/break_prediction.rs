//! Optimistic block writes and reconciliation on the multiplayer client.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::native_connection_groups::{GroupUpdates, ReceivedGroup, check_group_updates};
#[allow(unused_imports)]
use sunrise_core::position::BlockPos;

/// `BlockStatePredictionHandler.ServerVerifiedState`: the most recent sequence,
/// the server's latest state, and the player's position at the first prediction.
pub(in crate::game) struct Prediction {
    pub(in crate::game) sequence: i32,
    pub(in crate::game) state: BlockStateId,
    pub(in crate::game) player_pos: [f64; 3],
    pub(in crate::game) removed_entity: Option<Box<RemovedEntity>>,
    pub(in crate::game) group: Option<ReceivedGroup>,
}

/// A predicted write displaced this entity. The server need not resend its payload
/// or its synced attachments when a denial puts the same block back.
pub(in crate::game) struct RemovedEntity {
    pub(in crate::game) state: BlockStateId,
    pub(in crate::game) entity: BlockEntity,
    pub(in crate::game) attachments: Option<ResidentAttachments>,
}
