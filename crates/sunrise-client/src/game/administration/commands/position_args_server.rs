use super::{Context, Game, R, err};

impl Game<'_, crate::game::server_side::ServerState> {
    /// BlockPosArgument.getLoadedBlockPos.
    pub(in crate::game::server_side) fn block_pos_arg(
        &self,
        ctx: &Context,
        name: &str,
    ) -> R<[i32; 3]> {
        let c = self
            .coords(ctx, name)
            .ok_or_else(|| err("argument.pos.missing.int", &[]))?;
        // BlockPos.containing(getPosition)
        let p = self.resolve_coords(&c).map(|v| v.floor() as i32);
        let dimension = self.side.command_source.map_or(self.dim, |s| s.dimension);
        if dimension != self.dim || !self.world.chunks.contains_key(&(p[0] >> 4, p[2] >> 4)) {
            return Err(err("argument.pos.unloaded", &[]));
        }
        if p[1] < self.world.bottom_y() || p[1] >= self.world.top_y() {
            return Err(err("argument.pos.outofworld", &[]));
        }
        Ok(p)
    }
}
