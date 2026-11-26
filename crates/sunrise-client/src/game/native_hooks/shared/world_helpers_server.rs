use super::*;

impl Game<'_, crate::game::server_side::ServerState> {
    /// Sets a block in the current authoritative level through `Level.setBlock`.
    /// Returns false for another dimension, an invalid state, or no change.
    /// `flags` selects vanilla's neighbour, shape and client updates. A block
    /// entity block gets its block entity.
    pub(in crate::game) fn native_set_block(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        state: BlockStateId,
        flags: i32,
    ) -> bool {
        if !self.is_logical_server()
            || dimension != self.dim.id()
            || self.pack.assets.registry.state(state).is_none()
        {
            return false;
        }
        let changed = self.set_block_live(pos, state, flags, true);
        let _ = self.server_process_level_events();
        changed
    }

    /// The item entities of the server level `dimension` whose box meets
    /// the box `min`..`max`, in the level's order: vanilla's
    /// `Level.getEntitiesOfClass(ItemEntity.class, box, ENTITY_STILL_ALIVE)`, as a
    /// hopper's `getItemsAtAndAbove` asks for its block and the one above
    /// (`[x, y, z]`..`[x + 1, y + 2, z + 1]`). Empty for a level not in place
    /// (as `native_set_block`) and on a client of a server.
    #[allow(dead_code)]
    pub(in crate::game) fn native_item_entities(
        &self,
        dimension: &str,
        min: [f64; 3],
        max: [f64; 3],
    ) -> Vec<NativeItemEntity> {
        if (self.side.serving.is_none() && self.options.dedicated.is_none())
            || dimension != self.dim.id()
        {
            return Vec::new();
        }
        self.dropped
            .iter()
            .filter(|e| e.stack.count > 0 && box_meets(&e.bounds(), min, max))
            .map(|e| NativeItemEntity {
                id: e.id,
                position: [e.position.x, e.position.y, e.position.z],
                stack: e.stack.clone(),
                // The helper exposes an unsigned delay; keep its projection.
                pickup_delay: e.pickup_delay.clamp(0, i64::from(u32::MAX)) as u32,
            })
            .collect()
    }
}

/// An item entity as `native_item_entities` lists it.
/// Read by mods only.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(in crate::game) struct NativeItemEntity {
    /// The entity's ID.
    pub id: u32,
    pub position: [f64; 3],
    pub stack: ItemStack,
    /// Ticks before a player may pick it up (`ItemEntity.pickupDelay`);
    /// hoppers take it whatever this is.
    pub pickup_delay: u32,
}

/// `AABB.intersects`: the boxes overlap, touching faces excluded.
pub(in crate::game) fn box_meets(
    b: &sunrise_core::geometry::Aabb,
    min: [f64; 3],
    max: [f64; 3],
) -> bool {
    let (lo, hi) = (b.min_position(), b.max_position());
    lo.x < max[0]
        && hi.x > min[0]
        && lo.y < max[1]
        && hi.y > min[1]
        && lo.z < max[2]
        && hi.z > min[2]
}
