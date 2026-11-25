#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_chunk_unloaded::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::ModBlockEntity;

impl Game<'_> {
    /// Chunk `chunk` of this server level `dimension` left it, after its last save: the
    /// parts of its mod block entities are told, then the mods of the chunk.
    pub(in crate::game) fn native_chunk_and_block_entities_unload(
        &mut self,
        dimension: &str,
        chunk: [i32; 2],
    ) {
        if mod_api::__private::has_part_hook(mod_api::HookId::OnChunkUnloaded)
            && let Some(level) = self.level(dimension)
        {
            self.notify_chunk_block_entities_unloaded(level, (chunk[0], chunk[1]));
        }
        self.native_chunk_unload(dimension, chunk);
    }
}
