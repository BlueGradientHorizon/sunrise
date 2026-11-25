use super::Game;
use sunrise_core::nbt::Tag;

impl Game<'_, crate::game::server_side::ServerState> {
    /// `BlockEntity.saveWithoutMetadata` of the block entity at `pos`, without id or position.
    pub(in crate::game) fn block_entity_tag(&self, pos: [i32; 3]) -> Option<Tag> {
        let e = self.world.block_entities.get(&(pos[0], pos[1], pos[2]))?;
        let name = self.data_block_name(pos);
        let mut tag =
            self.encode_live_block_entity((pos[0], pos[1], pos[2]), e, &name, self.res.items());
        if let Tag::Compound(c) = &mut tag {
            for key in ["id", "x", "y", "z", "keepPacked"] {
                c.remove(&key.into());
            }
        }
        Some(tag)
    }
}
