//! Entity and level handles for native server mods (docs/modding/src/how-to/native-mods.md,
//! "Entity and level handles", #7387): `entity(id)` turns an event's
//! entity id into an `EntityRef`, and each `native_entity_*` helper reads the
//! entity a handle names. They read where the world is simulated: on a client,
//! with no world open, or for a handle of an earlier open, they return `None`,
//! except the reads marked side `both` (`entity`, `level`, `entity_position`,
//! `entity_rotation`, `entity_velocity`), which on a client answer from its own
//! copy: the local player and the entities the server told it of, in the level
//! it shows. The entities are those of the level in place (the one a hook runs
//! in) and the server's players, whatever their level.
#[cfg(test)]
#[path = "native_entity_refs/dragon_parts_table.rs"]
pub(in crate::game) mod dragon_parts_table;
#[path = "native_entity_refs/dragon_query.rs"]
pub(in crate::game) mod dragon_query;
#[path = "native_entity_refs/save_lending.rs"]
pub(in crate::game) mod save_lending;
#[allow(unused_imports)]
use super::{Game, commands::Target, entity::Entity, pick_entity_common::aabb_extents};
#[allow(unused_imports)]
use core::ops::ControlFlow;
#[allow(unused_imports)]
use mod_api::{
    BlockFace, BlockShapeKind, BlockUpdateFlags, ClipBlock, ClipFluid, ClipHit, DimensionKey,
    EntityRayHit, EntityRef, LevelReadError, LevelRef, NeighbourUpdate,
};
#[allow(unused_imports)]
use sunrise_core::geometry::{Aabb, Vec3};
#[allow(unused_imports)]
use sunrise_gameplay_common::game::transactions::After;
#[allow(unused_imports)]
use sunrise_items::items::ItemStack;
#[allow(unused_imports)]
use sunrise_sim::join_seq::{
    JoinList, JoinSeq, Joined,
    residency::table::{EntityLookup, Location, SectionSearch},
};
#[allow(unused_imports)]
use sunrise_world::registry::BlockStateId;

pub(in crate::game::server_side) fn list_query_body<T: Entity + Joined, const DIRECT: bool>(
    list: &JoinList<T, DIRECT>,
    join: JoinSeq,
) -> Option<Aabb> {
    let entity = list.get(list.locate(join)?)?;
    (!entity.is_removed()).then(|| entity.bounds())
}

impl crate::game::ServerGame<'_> {
    /// `ServerLevel.getEntity(UUID)` for [`LevelRef::entity_by_uuid`]: the level's UUID index,
    /// in place or parked, then the same resolution and visibility as any handle of that level.
    pub(in crate::game) fn level_entity_by_uuid(
        &self,
        level: LevelRef,
        uuid: mod_api::EntityUuid,
    ) -> Option<EntityRef> {
        self.native_level_world(level)?;
        let dimension = level.dimension();
        let uuid = sunrise_sim::lan::playerdata::uuid_ints(uuid.0);
        let mut after = None;
        loop {
            let id = self
                .common
                .resident_entities
                .table
                .borrow_mut()
                .level_uuid_candidate(dimension, uuid, after)?;
            let entity = EntityRef::new(level.epoch(), id.get());
            if self
                .native_entity_target_in(entity, dimension)
                .is_some_and(|(target, _)| self.resident_source_target_visible(dimension, target))
            {
                return Some(entity);
            }
            // A queued departure can overlap a replacement with the same UUID.
            after = Some(id);
        }
    }
    /// Loaded server levels, in vanilla dimension order. Empty without a server world.
    pub(in crate::game) fn native_list_levels(&self) -> Vec<LevelRef> {
        if !self.common.is_logical_server() || self.common.native_lifecycle.epoch().is_none() {
            return Vec::new();
        }
        self.loaded_dims()
            .into_iter()
            .filter_map(|dim| self.level(dim.id()))
            .collect()
    }
    pub(in crate::game::server_side) fn level_entity_candidates(
        &self,
        level: LevelRef,
    ) -> impl Iterator<Item = EntityRef> + '_ {
        let mut after = None;
        std::iter::from_fn(move || {
            // Copy the key and release the residency borrow before resolution or a visitor.
            let key = self
                .common
                .resident_entities
                .table
                .borrow_mut()
                .level_entity_candidate_after(level.dimension(), after)?;
            after = Some(key);
            Some(key.1.get())
        })
        .filter(move |id| {
            !self
                .common
                .persist
                .removals
                .iter()
                .chain(self.common.persist.destroying_vehicles.iter())
                .any(|target| matches!(*target, Target::Boat(n) | Target::Cart(n) if n == *id))
        })
        .map(move |id| EntityRef::new(level.epoch(), id))
    }

    /// Visit lookup members of the active server level, without spatial filtering.
    pub(in crate::game) fn level_visit_all_entities(
        &self,
        level: LevelRef,
        visit: &mut dyn FnMut(EntityRef) -> ControlFlow<()>,
    ) -> Result<ControlFlow<()>, LevelReadError> {
        if !self.common.is_logical_server() {
            return Err(LevelReadError::Unavailable);
        }
        if !self.level_handle_current(level) {
            return Err(LevelReadError::Stale);
        }
        if level.dimension() != self.common.dim.key() {
            return Err(LevelReadError::Unavailable);
        }

        for entity in self.level_entity_candidates(level) {
            if self.native_entity_target(entity).is_some() && visit(entity).is_break() {
                return Ok(ControlFlow::Break(()));
            }
        }
        Ok(ControlFlow::Continue(()))
    }
    /// Engine callers that mutate after querying need a snapshot of the matching handles.
    pub(in crate::game::server_side) fn level_entities_in(
        &self,
        level: LevelRef,
        min: [f64; 3],
        max: [f64; 3],
    ) -> Option<Vec<EntityRef>> {
        let mut entities = Vec::new();
        let _ = self
            .level_visit_entities_in(level, min, max, &mut |entity| {
                entities.push(entity);
                ControlFlow::Continue(())
            })
            .ok()?;
        Some(entities)
    }

    /// Where `entity` is resident on the server (`EntityPartState` / `EntityResidency`).
    pub(in crate::game) fn server_entity_residency(
        &self,
        entity: EntityRef,
    ) -> mod_api::EntityResidency {
        use mod_api::EntityResidency;
        if self.common.native_lifecycle.epoch() != Some(entity.epoch()) {
            return EntityResidency::Gone;
        }
        match self.server_resident_entity(entity.id()) {
            EntityLookup::Found { location, .. } => {
                if location.owner.dimension == self.common.dim.key() {
                    EntityResidency::InPlace
                } else {
                    EntityResidency::OtherLevel
                }
            }
            EntityLookup::Taken => EntityResidency::Busy,
            EntityLookup::Missing => EntityResidency::Gone,
        }
    }

    /// Visit intersecting, readable entities in `ServerLevel.getEntities` query order.
    pub(in crate::game) fn server_level_visit_entities_in(
        &self,
        level: LevelRef,
        min: [f64; 3],
        max: [f64; 3],
        visit: &mut dyn FnMut(EntityRef) -> ControlFlow<()>,
    ) -> Result<ControlFlow<()>, LevelReadError> {
        if !self.level_handle_current(level) {
            return Err(LevelReadError::Stale);
        }
        if !min.iter().chain(&max).all(|v| v.is_finite())
            || level.dimension() != self.common.dim.key()
        {
            return Err(LevelReadError::Unavailable);
        }
        let bounds = Aabb::new(
            Vec3::new(min[0], min[1], min[2]),
            Vec3::new(max[0], max[1], max[2]),
        );
        let search = SectionSearch::new(bounds);
        self.file_server_section_members(level);
        let mut after = None;
        loop {
            let candidate = self
                .common
                .resident_entities
                .table
                .borrow()
                .level_section_candidate_after(level.dimension(), search, after);
            let Some(key) = candidate else { break };
            after = Some(key);
            let (_, (_, id)) = key;
            let entity = EntityRef::new(level.epoch(), id.get());
            if let Some((target, Some(at))) = self.native_entity_target_with_location(entity)
                && self
                    .query_body(target, at.join, true)
                    .is_some_and(|body| bounds.intersects(body))
                && visit(entity).is_break()
            {
                return Ok(ControlFlow::Break(()));
            }
        }
        Ok(self.visit_dragon_query_parts(bounds, &mut |id| {
            let entity = EntityRef::new(level.epoch(), id);
            if self.native_entity_target(entity).is_some() {
                visit(entity)
            } else {
                ControlFlow::Continue(())
            }
        }))
    }

    /// File the rows admitted since the last query. One walk over the families files every
    /// member they hold at its position; only the rows no family walk reaches resolve one by one.
    pub(in crate::game::server_side) fn file_server_section_members(&self, level: LevelRef) {
        let table = &self.common.resident_entities.table;
        if !table
            .borrow_mut()
            .has_unfiled_section_members(level.dimension())
        {
            return;
        }
        {
            let mut table = table.borrow_mut();
            crate::game::server_side::pick_entity_server::visit_server_section_members(
                self,
                &mut |_, _, id, position| {
                    if let Some(id) = std::num::NonZeroU32::new(id) {
                        table.file_section_member(id, position);
                    }
                },
            );
        }
        let mut after = None;
        loop {
            let next = table
                .borrow_mut()
                .unfiled_section_candidate_after(level.dimension(), after);
            let Some(id) = next else { break };
            after = Some(id);
            let entity = EntityRef::new(level.epoch(), id.get());
            if let Some(target) = self.native_entity_target(entity)
                && let Some(position) = self.target_position(target)
            {
                table.borrow_mut().file_section_member(id, position);
            }
        }
    }

    pub(in crate::game::server_side) fn query_body(
        &self,
        target: Target,
        join: JoinSeq,
        server: bool,
    ) -> Option<Aabb> {
        match target {
            Target::Player if self.common.mode_id() != 3 => {
                let guest = self.native_target_guest(target)?;
                if !guest.participates_in_player_simulation() || !guest.placed {
                    return None;
                }
                self.common.player.as_ref().map(|p| p.bounds())
            }
            Target::Guest(conn) => self
                .command_guest(conn)
                .filter(|guest| {
                    guest.participates_in_player_simulation()
                        && guest.placed
                        && guest.game_mode != 3
                })
                .map(|guest| guest.bounds()),
            Target::Other(_) if server => self
                .server
                .others
                .0
                .get(self.server.others.0.locate(join)?)?
                .display_marker_or_interaction_bounds(),
            Target::Hook(id) if server => self
                .fishing_hooks_here()
                .find_map(|(hook, _)| (hook.id() == id).then(|| hook.bounds())),
            Target::Dragon(_) => self.common.dragon.as_ref().map(|d| d.body_bounds()),
            Target::Frame(_) => self
                .common
                .item_frames
                .get(self.common.item_frames.locate(join)?)
                .map(|f| aabb_extents(f.bounds())),
            Target::Painting(_) => self
                .common
                .paintings
                .get(self.common.paintings.locate(join)?)
                .map(|p| aabb_extents(p.bounds())),
            Target::Mob(_) => list_query_body(&self.common.mobs, join),
            Target::Item(_) => list_query_body(&self.common.dropped, join),
            Target::Orb(_) => list_query_body(&self.common.orbs, join),
            Target::Boat(_) => list_query_body(&self.common.boats, join),
            Target::Cart(_) => list_query_body(&self.common.carts, join),
            Target::Stand(_) => list_query_body(&self.common.armor_stands, join),
            Target::Tnt(_) => list_query_body(&self.common.tnts, join),
            Target::Falling(_) => list_query_body(&self.common.falling, join),
            Target::Projectile(_) => list_query_body(&self.common.creatures.projectiles, join),
            Target::Rocket(_) => list_query_body(&self.common.rockets, join),
            Target::Trident(_) => list_query_body(&self.common.tridents, join),
            Target::General(_) => {
                list_query_body(&self.common.resident_entities.general.rows, join)
            }
            _ => None,
        }
    }
    /// Every geometric hit, stably sorted nearest first. None for a refused walk.
    pub(in crate::game) fn level_ray_trace_entities(
        &self,
        level: LevelRef,
        from: [f64; 3],
        to: [f64; 3],
        margin: f64,
        exclude: Option<EntityRef>,
    ) -> Option<Vec<EntityRayHit>> {
        let mut hits = Vec::new();
        self.visit_entity_ray_hits(
            level,
            from,
            to,
            margin,
            exclude,
            |_| true,
            |hit, _| hits.push(hit),
        )
        .ok()?;
        hits.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        Some(hits)
    }

    /// What the block at `pos` would drop (`Block.getDrops`) with `tool` and `entity` as the
    /// breaker, without breaking it: the level's `block_drops_of`, which draws from the random
    /// as vanilla's roll does. None for a refused write, a stale or parked handle, a client, an
    /// unloaded or out-of-height position, and an `entity` that names nothing.
    pub(in crate::game) fn level_block_drops(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
        tool: Option<&ItemStack>,
        entity: Option<EntityRef>,
    ) -> Option<Vec<ItemStack>> {
        if self.entity_ids_exhausted() {
            return None;
        }
        if !self.native_level_write_allowed(level, pos)
            || self.common.dim.key() != level.dimension()
        {
            return None;
        }
        if let Some(entity) = entity {
            self.native_entity_target(entity)?;
        }
        let state = self.common.world.block(pos[0], pos[1], pos[2]);
        let at = sunrise_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        Some(self.common.with_level(|_, level, _| {
            crate::game::server_side::spill::block_drops_of(level, at, state, tool, entity)
        }))
    }
    /// One roll of the world's loot table `table` (`LootTable.getRandomItems`) with `origin` as
    /// ORIGIN, `tool` as TOOL and `entity` as THIS_ENTITY when given: the context
    /// `/loot … fish <table> <pos> <tool>` gives. It draws from the table's random sequence,
    /// else the level random, and the stacks come split to their stack sizes. None for a
    /// missing table, a stale handle or one of a parked level, a client, and an `entity` that
    /// names nothing.
    pub(in crate::game) fn level_roll_loot(
        &mut self,
        level: LevelRef,
        table: &str,
        origin: [f64; 3],
        tool: Option<&ItemStack>,
        entity: Option<EntityRef>,
    ) -> Option<Vec<ItemStack>> {
        if !self.common.is_logical_server()
            || !self.level_handle_current(level)
            || self.common.dim.key() != level.dimension()
            || !world_items!(self).has_loot_table(table)
        {
            return None;
        }
        let this = match entity {
            Some(entity) => {
                let target = self.native_entity_target(entity)?;
                Some(self.target_loot_entity(target))
            }
            None => None,
        };
        self.roll_table_at(table, Some(origin), tool, this.as_ref())
    }
    /// Set a block through Level.setBlock. Typed errors distinguish stale/unavailable
    /// levels; Ok(false) preserves vanilla's position/no-change refusal.
    pub(in crate::game) fn level_set_block(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
        flags: BlockUpdateFlags,
    ) -> Result<bool, mod_api::LevelWriteError> {
        if !self.native_level_write_position(level, pos)? {
            return Ok(false);
        }
        if let Some(mut host) = super::journal_level_write::JournalLevelWrite::new(self, level) {
            return Ok(host.write_block(pos, state, flags.bits()));
        }
        Ok(self.native_set_block(level.dimension_id(), pos, state, flags.bits()))
    }
    /// Set or clear the mod-published connection group at `pos`.
    /// Returns `Ok(true)` if changed, `Ok(false)` if unchanged, unloaded, out of height, or
    /// fenced, and `Err(LevelWriteError::Stale | Unavailable)` for stale or non-writable levels.
    pub(in crate::game) fn level_set_connection_group(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
        group: Option<u64>,
    ) -> Result<bool, mod_api::LevelWriteError> {
        if !self.native_level_write_position(level, pos)? {
            return Ok(false);
        }
        Ok(self.journal_set_connection_group(pos, group))
    }
    /// `Block.updateFromNeighbourShapes` for `state` at `pos`, nothing written: the state
    /// fitted to the loaded blocks around it. A position that is not loaded or outside the
    /// world gives `state` back.
    pub(in crate::game) fn level_update_from_neighbour_shapes(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Result<BlockStateId, mod_api::LevelWriteError> {
        if !self.native_level_write_position(level, pos)? {
            return Ok(state);
        }
        let at = sunrise_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        let fitted = self
            .common
            .with_level(|b, level, ticks| b.update_from_neighbour_shapes(level, ticks, at, state));
        let _ = self.server_process_level_events();
        Ok(fitted)
    }
    /// Remove a block in place (`Level.removeBlock`), preserving its fluid.
    pub(in crate::game) fn level_remove_block(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
        moved: bool,
    ) -> bool {
        if !self.native_level_write_allowed(level, pos)
            || self.common.dim.key() != level.dimension()
        {
            return false;
        }
        if let Some(mut host) = super::journal_level_write::JournalLevelWrite::new(self, level) {
            return mod_api::LevelWriteHost::remove_block(&mut host, pos, moved);
        }
        let (state, flags) = self.level_removal_write(pos, moved);
        let changed = self.common.set_block_live(pos, state, flags, false);
        let _ = self.server_process_level_events();
        changed
    }
    /// Tell the neighbours of `pos` that a block changed there, as vanilla's own writes do:
    /// `Neighbours` (`Level.updateNeighborsAt`: a lever's or a button's own block, and the one
    /// it is attached to, when the signal it gives changes), `NeighboursExcept(direction)`
    /// (`updateNeighborsAtExceptFromFacing`: a diode's front) or `OutputSignal`
    /// (`updateNeighbourForOutputSignal`: the comparators beside it, when its comparator output
    /// changes without its state changing). `source` is the state whose block vanilla passes as
    /// the one that changed. False, with nothing done, off the level in place, for a position
    /// that isn't loaded, or for a `source` that is no block state.
    pub(in crate::game) fn level_update_neighbors(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
        source: BlockStateId,
        update: NeighbourUpdate,
    ) -> Result<bool, mod_api::LevelWriteError> {
        if !self.native_level_write_position(level, pos)? {
            return Ok(false);
        }
        let at = sunrise_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        self.common.with_level(|b, level, ticks| {
            b.neighbour_update(level, ticks, at, b.block(source), update)
        });
        let _ = self.server_process_level_events();
        Ok(true)
    }
    /// Tell the block at `pos` that `source`'s block beside it changed (`Level.neighborChanged`),
    /// as a diode tells the block in front of it before updating that block's neighbours. False,
    /// with nothing done, as for `update_neighbors`: off the level in place, for a position that
    /// isn't loaded, or for a `source` that is no block state.
    pub(in crate::game) fn level_neighbor_changed(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
        source: BlockStateId,
    ) -> Result<bool, mod_api::LevelWriteError> {
        if !self.native_level_write_position(level, pos)? {
            return Ok(false);
        }
        let at = sunrise_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        self.common
            .with_level(|b, level, ticks| b.neighbor_changed_at(level, ticks, at, b.block(source)));
        let _ = self.server_process_level_events();
        Ok(true)
    }
    /// Destroy a block in the level in place (`Level.destroyBlock`), optionally dropping
    /// resources with `tool` (None is an empty hand). With `collect`, return the dropped
    /// stacks without spawning their item entities; otherwise return an empty list after
    /// spawning them. The one level drop path runs loot, block-drop hooks and experience
    /// once, in vanilla's random order. `drop` false suppresses resources entirely.
    /// None means nothing broke: air, a refused write, a stale handle, a client, an
    /// unloaded/out-of-height position, or a parked level.
    pub(in crate::game) fn level_destroy_block(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
        drop: bool,
        tool: Option<&ItemStack>,
        collect: bool,
    ) -> Option<Vec<ItemStack>> {
        // Loot and break effects cannot be rolled back by the level journal.
        if self.has_open_transaction()
            || !self.native_level_write_allowed(level, pos)
            || self.common.dim.key() != level.dimension()
        {
            return None;
        }
        let at = sunrise_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        let (changed, drops) = self.common.with_level(|b, level, ticks| {
            level.drop_tool = Some(((pos[0], pos[1], pos[2]), tool.cloned()));
            level.drop_into = collect.then(Vec::new);
            let changed = b.destroy_block(level, ticks, at, drop);
            (changed, level.drop_into.take().unwrap_or_default())
        });
        let _ = self.server_process_level_events();
        changed.then_some(drops)
    }

    /// Send `ServerLevel.destroyBlockProgress` in this level: stages 0..9 show a crack,
    /// any other stage removes it. A machine uses a stable negative breaker id.
    pub(in crate::game) fn level_set_break_progress(
        &mut self,
        level: LevelRef,
        breaker: i32,
        pos: [i32; 3],
        progress: i32,
    ) -> bool {
        if !self.native_level_write_allowed(level, pos)
            || self.common.dim.key() != level.dimension()
        {
            return false;
        }
        self.after_commit(After::BreakProgress(breaker, pos, progress));
        true
    }
    /// `Level.sendBlockUpdated`: queue the block at `pos` to be sent to the clients again.
    pub(in crate::game) fn level_send_block_updated(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> bool {
        if !self.native_level_write_allowed(level, pos)
            || self.common.dim.key() != level.dimension()
        {
            return false;
        }
        self.common.notify_block_clients((pos[0], pos[1], pos[2]));
        true
    }
    /// Apply bone meal to the block at `pos` as a dispenser does (`BoneMealItem.growCrop`, then
    /// `growWaterPlant`): the mods' `bonemeal` hook first, level event 1505 when something grew.
    /// False for a refused write, a stale or parked handle, a client, and a block that did not
    /// grow.
    pub(in crate::game) fn level_bone_meal(&mut self, level: LevelRef, pos: [i32; 3]) -> bool {
        if !self.native_level_write_allowed(level, pos)
            || self.common.dim.key() != level.dimension()
        {
            return false;
        }
        let at = sunrise_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        let mut stack = sunrise_items::items::ItemStack::new("minecraft:bone_meal", 1);
        let ask = self.bonemeal(at, false, &mut stack);
        let grown = self.common.with_level(|b, level, ticks| {
            ask.crop
                .unwrap_or_else(|| b.grow_crop(level, ticks, at, ask.check))
                || b.grow_water_plant(level, ticks, at, None)
        });
        if grown {
            self.common
                .level_events
                .push(sunrise_world::level::LevelEvent::LevelEvent {
                    id: 1505,
                    pos: at,
                    data: 15,
                    source: None,
                });
        }
        let _ = self.server_process_level_events();
        grown
    }
    /// Set the biome of the cell holding `pos` to `biome` (`/fillbiome` for one cell). False for a
    /// refused write, a stale or parked handle, a client, an unknown biome and an unchanged cell.
    pub(in crate::game) fn level_set_biome(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
        biome: &str,
    ) -> bool {
        if !self.native_level_write_allowed(level, pos)
            || self.common.dim.key() != level.dimension()
        {
            return false;
        }
        self.set_biome_at(pos, biome)
    }
    /// One residency boundary for state-bearing block writes; position refusal is vanilla.
    pub(in crate::game) fn native_level_write_position(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Result<bool, mod_api::LevelWriteError> {
        let world = self.native_level_world_result(level)?;
        if self.common.dim.key() != level.dimension() {
            return Err(mod_api::LevelWriteError::Unavailable);
        }
        Ok(!self.common.world.carrier_fences.locks_cell(&pos)
            && world.loaded(pos[0], pos[2])
            && world.shape.contains_y(pos[1]))
    }
    pub(crate) fn native_level_write_allowed(&self, level: LevelRef, pos: [i32; 3]) -> bool {
        // A carrier transfer fence holds this cell: every mod write
        // through the level gate refuses while the operation runs.
        !self.common.world.carrier_fences.locks_cell(&pos)
            && self.common.is_logical_server()
            && self.level_handle_current(level)
            && self.common.world.loaded(pos[0], pos[2])
            && self.common.world.shape.contains_y(pos[1])
    }

    /// Server-side `Entity.getType`: `minecraft:zombie`. API 373.
    pub(in crate::game) fn server_entity_kind(
        &self,
        entity: EntityRef,
    ) -> Option<mod_api::EntityTypeKey> {
        match self.native_entity_read_target(entity) {
            Some(target) => self.native_entity_kind_key(target),
            None => {
                if self.common.native_lifecycle.epoch() == Some(entity.epoch())
                    && let Some((_, mob)) = self.mob_in_other_level(entity.id())
                {
                    return mob.kind.type_key;
                }
                None
            }
        }
    }
    /// The entity's UUID (`Entity.getUUID`), the one it is saved with; an
    /// entity that has none yet gets it now, as a new vanilla entity does.
    /// On a client, the one the server added it with (a player's is its
    /// profile's). API 373.
    pub(in crate::game) fn entity_uuid(&mut self, entity: EntityRef) -> Option<u128> {
        if self.common.native_lifecycle.epoch() == Some(entity.epoch())
            && self.mob_in_other_level(entity.id()).is_some()
        {
            return self
                .common
                .resident_entities
                .table
                .borrow_mut()
                .resident_uuid(entity.id())
                .map(uuid_from_words);
        }
        let words = match self.native_entity_target(entity)? {
            Target::Player => {
                let dimension = self.current_player_resident_location()?.owner.dimension;
                return self
                    .known_resident_entity_uuid_at(dimension, entity.id())
                    .map(uuid_from_words);
            }
            Target::Guest(conn) => return self.command_guest(conn).map(|g| g.uuid),
            _ => self.uuid_for(entity.id()),
        };
        Some(uuid_from_words(words))
    }
    /// The level the entity is in on the server (`Entity.level`).
    pub(in crate::game) fn server_entity_level(&self, entity: EntityRef) -> Option<LevelRef> {
        let key = match self.native_entity_target(entity) {
            Some(t) => self.target_level(t).key(),
            None if self.common.native_lifecycle.epoch() == Some(entity.epoch()) => {
                self.mob_in_other_level(entity.id())?.0.key()
            }
            None => return None,
        };
        Some(self.level_ref(entity.epoch(), key))
    }
}

/// A saved UUID's four ints (`UUIDUtil.uuidFromIntArray`) as one number.
pub(in crate::game::server_side) fn uuid_from_words(words: [i32; 4]) -> u128 {
    words
        .iter()
        .fold(0, |uuid, w| (uuid << 32) | u128::from(*w as u32))
}

#[cfg(test)]
pub(in crate::game) mod ray_tests {
    use super::*;
    use sunrise_gameplay_common::game::native_entity_refs_common::clip_entity_ray;

    fn entity(id: u32) -> EntityRef {
        EntityRef::new(mod_api::WorldEpoch::new(1), id)
    }

    /// A unit box with its lowest corner at `x` on the x axis, around the ray along x.
    fn at(x: f64) -> [[f64; 3]; 2] {
        [[x, -0.5, -0.5], [x + 1.0, 0.5, 0.5]]
    }

    #[test]
    fn clipping_reports_the_entry_and_distance_of_each_hit() {
        let to = Vec3::new(10.0, 0.0, 0.0);
        for (id, x, squared) in [(1, 6.0, 36.0), (2, 2.0, 4.0)] {
            assert_eq!(
                clip_entity_ray(Vec3::ZERO, to, Vec3::ZERO, entity(id), at(x)),
                Some((
                    EntityRayHit {
                        entity: entity(id),
                        location: [x, 0.0, 0.0],
                        distance: x,
                    },
                    squared,
                )),
            );
        }
        assert_eq!(
            clip_entity_ray(Vec3::ZERO, to, Vec3::ZERO, entity(3), at(20.0)),
            None,
            "the box past the segment's end is not hit"
        );
    }

    #[test]
    fn the_margin_grows_every_box_and_a_start_inside_one_misses_it() {
        // The box is above the ray: missed without a margin, hit with one.
        let above = [[3.0, 0.8, -0.5], [4.0, 1.8, 0.5]];
        let to = Vec3::new(10.0, 0.0, 0.0);
        assert!(clip_entity_ray(Vec3::ZERO, to, Vec3::ZERO, entity(1), above).is_none());
        let (grown, _) =
            clip_entity_ray(Vec3::ZERO, to, Vec3::new(1.0, 1.0, 1.0), entity(1), above)
                .expect("the ray enters the grown box");
        assert_eq!(
            grown.location,
            [2.0, 0.0, 0.0],
            "entered at the grown box's face"
        );
        // `AABB.clip` from inside finds no entry, as for a projectile still in its shooter.
        assert!(
            clip_entity_ray(Vec3::new(0.5, 0.0, 0.0), to, Vec3::ZERO, entity(1), at(0.0)).is_none()
        );
        // A negative margin deflates via `AABB.inflate`, and over-deflation swaps min/max.
        let grazing_from = Vec3::new(0.0, 0.375, 0.0);
        let grazing_to = Vec3::new(10.0, 0.375, 0.0);
        assert!(
            clip_entity_ray(grazing_from, grazing_to, Vec3::ZERO, entity(1), at(3.0)).is_some()
        );
        assert!(
            clip_entity_ray(
                grazing_from,
                grazing_to,
                Vec3::new(-0.25, -0.25, -0.25),
                entity(1),
                at(3.0),
            )
            .is_none()
        );
        let over_from = Vec3::new(0.0, 0.625, 0.0);
        let over_to = Vec3::new(10.0, 0.625, 0.0);
        assert!(clip_entity_ray(over_from, over_to, Vec3::ZERO, entity(1), at(3.0)).is_none());
        let (over, _) = clip_entity_ray(
            over_from,
            over_to,
            Vec3::new(-1.25, -1.25, -1.25),
            entity(1),
            at(3.0),
        )
        .expect("over-deflation swaps min/max like vanilla AABB");
        assert_eq!(over.location, [2.75, 0.625, 0.0]);
    }
}

use sunrise_gameplay_common::game::transactions::TransactionsGameExt as _;

use sunrise_gameplay_common::game::GameGameExt2 as _;

use sunrise_gameplay_common::game::commands_common::CommandsCommonGameExt2 as _;

use sunrise_gameplay_common::game::native_entity_motion_values::NativeEntityMotionValuesGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt4 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt5 as _;

use sunrise_gameplay_common::game::native_hooks::ModGameExt1 as _;

use sunrise_gameplay_common::game::native_level_refs_common::NativeLevelRefsCommonGameExt1 as _;
