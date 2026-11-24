#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::kept_columns::Column;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::kept_columns::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    chunk_map::ClientChunkRange, fast_hash::FxHashSet,
};

impl Game<'_> {
    pub(in crate::game) fn free_remote_columns(
        &mut self,
        gone: &mut FxHashSet<Column>,
        keep_terrain: bool,
    ) {
        if gone.is_empty() {
            return;
        }
        let freed =
            sunrise_client::streaming::Streaming::take_remote_columns(&mut self.world, gone);
        self.streaming.forget_columns(&mut self.world, gone);
        if keep_terrain && let Some(lod) = &mut self.lod.0 {
            for chunk in &freed {
                lod.observe(chunk.clone(), self.res.facts());
            }
        }
        if let Some(renderer) = self.renderer.0.as_mut() {
            renderer.remove_columns(gone.iter().copied());
        }
        sunrise_client::chunk::free_columns(freed, gone.len());
        self.be_anims
            .pots
            .retain(|[x, _, z], _| !gone.contains(&(*x >> 4, *z >> 4)));
        sunrise_client::spans::time("unload_block_entities", || {
            for &column in gone.iter() {
                self.world.block_entities.unload_column(column);
            }
        });
        if let Some(r) = self.remote.as_mut() {
            r.attachments.retain(|holder, _| match *holder {
                Holder::Chunk([x, z]) => !gone.contains(&(x, z)),
                Holder::BlockEntity([x, _, z]) => !gone.contains(&(x >> 4, z >> 4)),
                _ => true,
            });
        }
        gone.clear();
    }
    pub(in crate::game) fn update_remote_chunk_view(&mut self, range: ClientChunkRange) {
        let (hidden, shown) = self.world.chunks.set_client_chunk_range(range);
        if let Some(renderer) = &mut self.renderer.0 {
            renderer.remove_columns(hidden.iter().copied());
        }
        self.streaming
            .remote_chunk_visibility_changed(&mut self.world, &hidden, &shown);
    }
}
