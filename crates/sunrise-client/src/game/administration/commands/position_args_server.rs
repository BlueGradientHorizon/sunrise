#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Context, Dim, Game, R, err};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::position_args::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;

impl Game<'_> {
    /// BlockPosArgument.getLoadedBlockPos.
    pub(in crate::game) fn block_pos_arg(&self, ctx: &Context, name: &str) -> R<[i32; 3]> {
        let c = self
            .coords(ctx, name)
            .ok_or_else(|| err("argument.pos.missing.int", &[]))?;
        // BlockPos.containing(getPosition)
        let p = self.resolve_coords(&c).map(|v| v.floor() as i32);
        let dimension = self.server.command_source.map_or(self.dim, |s| s.dimension);
        if dimension != self.dim || !self.world.chunks.contains_key(&(p[0] >> 4, p[2] >> 4)) {
            return Err(err("argument.pos.unloaded", &[]));
        }
        if p[1] < self.world.bottom_y() || p[1] >= self.world.top_y() {
            return Err(err("argument.pos.outofworld", &[]));
        }
        Ok(p)
    }
}
