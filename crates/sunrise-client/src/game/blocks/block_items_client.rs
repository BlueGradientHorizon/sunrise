use super::Game;
use sunrise_client::{block_entity_item, containers::BlockEntity, items::ItemStack, save};
use sunrise_core::nbt::Tag;

impl Game<'_, crate::game::client_side::ClientState> {
    /// Client arm of [`Game::side_encode_live_block_entity`].
    pub(in crate::game) fn client_encode_live_block_entity(
        &self,
        key: (i32, i32, i32),
        entity: &BlockEntity,
        block: &str,
    ) -> Tag {
        save::block_entity_encode::encode(key, entity, block, self.res.items())
    }

    /// `BlockItem.updateBlockEntityComponents` on a client's level, for the
    /// block it predicts at `pos`: the block entity takes the stack's
    /// components but not its `block_entity_data` (`updateCustomBlockEntityTag`
    /// returns on a client), so a copied sign's editor opens with its text (#8102).
    pub(in crate::game::client_side) fn apply_predicted_item(
        &mut self,
        pos: [i32; 3],
        stack: &ItemStack,
    ) {
        let mut stack = stack.clone();
        if stack.data().is_some() {
            stack
                .data_mut()
                .set_other(block_entity_item::BLOCK_ENTITY_DATA, None);
        }
        if self.apply_placed_item_common(pos, &stack, false).is_some() {
            self.side_update_spawner_display((pos[0], pos[1], pos[2]));
        }
    }
}
