#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_connection_groups::{
    GroupUpdates, ReceivedGroup, check_group_updates,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::break_prediction::Prediction;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::break_prediction::RemovedEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::break_prediction::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::position::BlockPos;

impl Game<'_> {
    pub(in crate::game::client_side) fn check_active_group_updates(
        &self,
        groups: &GroupUpdates,
        limit: usize,
    ) -> sunrise_protocol::Result<()> {
        check_group_updates(
            &self.world,
            groups.iter().filter_map(|(&pos, &group)| {
                (!self
                    .remote
                    .as_ref()
                    .is_some_and(|r| r.predictions.contains_key(&pos)))
                .then_some((pos, group))
            }),
            limit,
        )
    }
    /// Retire the synced copy from the live holder; retain it only with a removed
    /// prediction entity from the block the server still believes is here.
    pub(in crate::game::client_side) fn retire_client_block_entity(
        &mut self,
        pos: [i32; 3],
        previous: BlockStateId,
        removed: Option<BlockEntity>,
    ) {
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        let mut attachments = r.attachments.remove(&Holder::BlockEntity(pos));
        if let (Some(p), Some(entity)) = (r.predictions.get_mut(&pos), removed)
            && p.removed_entity.is_none()
            && self.res.facts().get(p.state).name == self.res.facts().get(previous).name
        {
            if let Some(attachments) = &mut attachments {
                attachments.take_index_binding();
                attachments.retire();
            }
            p.removed_entity = Some(Box::new(RemovedEntity {
                state: previous,
                entity,
                attachments,
            }));
        }
    }
    /// `MultiPlayerGameMode.stopDestroyBlock`: aborts the break and starts the attack strength
    /// again, if one is running (`if (!isDestroying) return;`), and says whether it did. Only the
    /// progress goes: `destroyBlockPos` and `destroyingItem` keep the target, so a dig resumed on
    /// the same block sends no START and the server's clock runs on (#4124).
    pub(in crate::game) fn stop_remote_mining(&mut self) -> bool {
        let kept = self.remote.as_mut().and_then(|r| {
            if !std::mem::take(&mut r.is_destroying) {
                return None;
            }
            let (pos, face, _) = r.destroying?;
            r.destroying = Some((pos, face, 0.0));
            r.destroying
        });
        let Some((pos, _, _)) = kept else {
            return false;
        };
        let state = self.world.block(pos[0], pos[1], pos[2]);
        self.tutorial_destroy(state, -1.0);
        self.send(S::PlayerAction(play::PlayerAction {
            action: 2,
            pos,
            direction: 0,
            sequence: 0,
        }));
        self.reset_attack_strength();
        true
    }
    /// `handleLevelChunkWithLight` writes the column directly, unlike verified block
    /// updates. When a pending correction carries group data, that newer authoritative
    /// snapshot supersedes the correction (including absence). Predictions without a
    /// group correction retain vanilla's acknowledgement behaviour.
    pub(in crate::game::client_side) fn supersede_group_predictions(
        &mut self,
        chunk: &sunrise_client::chunk::Chunk,
    ) {
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        for (&[x, y, z], prediction) in &mut r.predictions {
            if prediction.group.is_some()
                && (x >> 4, z >> 4) == (chunk.x, chunk.z)
                && chunk.section_index(y).is_some()
            {
                prediction.state = chunk.block(x & 15, y, z & 15);
                prediction.group = Some(ReceivedGroup(chunk.connection_group(x & 15, y, z & 15)));
                prediction.removed_entity = None;
            }
        }
    }
}
