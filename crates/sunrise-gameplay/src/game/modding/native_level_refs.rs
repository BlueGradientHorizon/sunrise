//! A level's blocks, height and time through a `LevelRef`
//! (docs/modding/src/how-to/native-mods.md, "Entity and level handles", #7387). Server reads
//! reach any loaded level; both-side reads also reach the client's current level.
//! A handle of an earlier open or an unloaded block reads `None` (or `false`).
use crate::game::ServerGame;
use sunrise_gameplay_common::game::native_level_refs_common::{block_pos, horizontal_in_bounds};
use mod_api::world::mod_chunk_tickets::ModTicket;
use mod_api::{
    BlockHit, BlockTransformPreview, ChunkRef, ChunkStatus, ClipBlock, ClipFluid, GlobalPos, Hand,
    Identifier, LevelRef, MapState, PlayerRef, RespawnData, TicketController,
};
use sunrise_core::{
    geometry::Direction,
    position::{BlockPos, ChunkPos},
};
use sunrise_items::items::ItemStack;
use sunrise_level::chunk_streamer::ChunkStreamer;
use sunrise_level::world::World;

/// Whether chunk `pos` lies inside the world (`/forceload`'s 30 million block bound).
fn chunk_in_world(pos: [i32; 2]) -> bool {
    const LIMIT: i32 = 30_000_000 >> 4;
    pos.iter().all(|c| (-LIMIT..LIMIT).contains(c))
}

/// Whether the mod running owns `controller`: a mod uses only the controllers it registered.
fn calling_mod_owns(controller: TicketController) -> bool {
    controller
        .owner()
        .is_some_and(|owner| mod_api::content::crash::current_native_mod() == Some(owner))
}

/// A mod's registry id or `#tag`, namespaced as a command argument is.
fn normalized_selector(id: &str) -> String {
    use sunrise_command::args::normalized;
    match id.strip_prefix('#') {
        Some(tag) => format!("#{}", normalized(tag)),
        None => normalized(id).into_owned(),
    }
}

/// Bound the synchronous generator work before constructing its spiral or height vector.
pub fn biome_search_within_budget(parameters: mod_api::BiomeSearch, height: i32) -> bool {
    if parameters.radius < 0
        || parameters.horizontal_step <= 0
        || parameters.vertical_step <= 0
        || height <= 0
    {
        return false;
    }
    let rings = parameters.radius as u64 / parameters.horizontal_step as u64;
    let width = 2 * rings + 1;
    // Clamping the origin into the build interval cannot increase this height count.
    let heights = (height as u64 - 1) / parameters.vertical_step as u64 + 1;
    width
        .checked_mul(width)
        .and_then(|columns| columns.checked_mul(heights))
        .is_some_and(|samples| samples <= mod_api::BiomeSearch::MAX_SAMPLES)
}

impl ServerGame<'_> {
    pub(in crate::game) fn level_add_water_ticket(
        &mut self,
        level: LevelRef,
        min: [i32; 3],
        max: [i32; 3],
    ) -> Option<mod_api::WaterTicket> {
        let owner = mod_api::content::crash::current_native_mod()?;
        let owner_index = mod_api::__private::mod_index(owner)?;
        if !mod_api::__private::mod_index_on(owner_index) {
            return None;
        }
        self.native_level_world(level)?;
        let world = if level.dimension() == self.common.dim.key() {
            &mut self.common.world
        } else {
            self.server
                .levels
                .0
                .parked_world_mut_of(level.dimension_id())?
        };
        world
            .water_tickets
            .add(owner, min, max, || {
                mod_api::__private::mod_index_on(owner_index)
            })
            .map(|id| mod_api::WaterTicket::new(level, id))
    }

    pub(in crate::game) fn water_ticket_remove(&mut self, ticket: mod_api::WaterTicket) -> bool {
        let Some(owner) = mod_api::content::crash::current_native_mod() else {
            return false;
        };
        let Some(owner_index) = mod_api::__private::mod_index(owner) else {
            return false;
        };
        if !mod_api::__private::mod_index_on(owner_index) {
            return false;
        }
        let level = ticket.level();
        if self.native_level_world(level).is_none() {
            return false;
        }
        let world = if level.dimension() == self.common.dim.key() {
            &mut self.common.world
        } else if let Some(world) = self.server.levels.0.parked_world_mut_of(level.dimension_id()) {
            world
        } else {
            return false;
        };
        world.water_tickets.remove(owner, ticket.id())
    }

    pub(in crate::game) fn level_has_water_ticket(&self, level: LevelRef, pos: [i32; 3]) -> bool {
        self.native_level_world(level)
            .is_some_and(|world| world.water_tickets.contains(pos))
    }

    /// `ServerLevel.findClosestBiome3d`, using the same generator query as `/locate biome`.
    pub(in crate::game) fn level_find_closest_biome(
        &self,
        level: LevelRef,
        biome: &str,
        parameters: mod_api::BiomeSearch,
    ) -> Option<mod_api::BiomeSearchResult> {
        let world = self.native_level_world(level)?;
        if !biome_search_within_budget(parameters, world.shape.max_y() - world.shape.min_y) {
            return None;
        }
        let terrain = if level.dimension() == self.common.dim.key() {
            self.common.end.locator()
        } else {
            self.server.levels.0.parked_locator_of(level.dimension_id())
        }?;
        let selector = normalized_selector(biome);
        let allowed = |name: &str| self.biome_matches(&selector, name);
        let query = sunrise_world::worldgen::generator::BiomeSearch {
            allowed: &allowed,
            origin: parameters.origin,
            radius: parameters.radius,
            horizontal: parameters.horizontal_step,
            vertical: parameters.vertical_step,
            // Resident columns are padded to the Overworld's storage span.
            // BiomeSource searches use the dimension's build range instead.
            min_y: world.shape.min_y,
            max_y: world.shape.max_y() - 1,
        };
        terrain
            .find_closest_biome(&query)
            .map(|(biome, position)| mod_api::BiomeSearchResult::new(biome, position))
    }

    /// `ServerLevel.findNearestMapStructure` without skipping known starts, through
    /// the generator search `/locate structure` runs.
    pub(in crate::game) fn level_find_nearest_structure(
        &self,
        level: LevelRef,
        structure: &str,
        parameters: mod_api::StructureSearch,
    ) -> Option<mod_api::StructureSearchResult> {
        self.native_level_world(level)?;
        // Inside the world the placement cells' chunks stay far from overflowing.
        if !(0..=mod_api::StructureSearch::MAX_RADIUS).contains(&parameters.radius)
            || !horizontal_in_bounds(parameters.origin)
        {
            return None;
        }
        let terrain = if level.dimension() == self.common.dim.key() {
            self.common.end.locator()
        } else {
            self.server.levels.0.parked_locator_of(level.dimension_id())
        }?;
        let names = self.structure_names(&normalized_selector(structure));
        terrain
            .locate(&names, parameters.origin, parameters.radius)
            .map(|(structure, position)| mod_api::StructureSearchResult::new(structure, position))
    }

    /// The authoritative server's `LevelReader.getBiome`, including parked levels.
    pub(in crate::game) fn level_biome_at(&self, level: LevelRef, pos: [i32; 3]) -> Option<String> {
        if !self.common.is_logical_server() {
            return None;
        }
        self.native_read_biome(level, pos)
    }

    /// The border of a resident server level, through the handle's lifetime check.
    pub(in crate::game) fn level_world_border(&self, level: LevelRef) -> Option<[f64; 4]> {
        self.native_level_world(level)?;
        Some(self.common.border.bounds())
    }
    /// Whether this chunk is loaded; false on a client or for a stale handle.
    pub(in crate::game) fn chunk_loaded(&self, chunk: ChunkRef) -> bool {
        self.chunk(chunk.level(), chunk.pos()).is_some()
    }
    /// The status of a loaded chunk. Proto chunks are not exposed here.
    pub(in crate::game) fn chunk_status(&self, chunk: ChunkRef) -> Option<ChunkStatus> {
        self.chunk(chunk.level(), chunk.pos())
            .map(|_| ChunkStatus::Full)
    }
    /// A handle for chunk `pos` of an open level, loaded or not, so a mod can
    /// force an unloaded chunk. None for a stale handle, on a client, and for
    /// a chunk outside the world (`ChunkPos` past 30 million blocks).
    pub(in crate::game) fn level_chunk_ref(
        &self,
        level: LevelRef,
        pos: [i32; 2],
    ) -> Option<ChunkRef> {
        self.native_level_world(level)?;
        chunk_in_world(pos).then_some(ChunkRef::new(level, pos))
    }
    /// `Level.getChunk(x, z)`: load or generate chunk `pos` to FULL, blocking the server tick
    /// until it is there, under a one-tick `UNKNOWN` ticket (`Game::load_full_chunk`).
    /// `Unloaded` for a stale handle, on a client, for a chunk past `ChunkPos.isValid`, and when
    /// the level's generation is paused or stopped; `Failed` when a generation it needs panicked.
    pub(in crate::game) fn level_load_chunk(
        &mut self,
        level: LevelRef,
        pos: [i32; 2],
    ) -> Result<(), mod_api::ChunkLoadError> {
        if self.native_level_streaming(level).is_none() {
            return Err(mod_api::ChunkLoadError::Unloaded);
        }
        let parked = (level.dimension() != self.common.dim.key()).then_some(level);
        self.load_full_chunk(parked, (pos[0], pos[1]))
    }
    /// Whether `/forceload` holds the chunk (`ServerLevel.getForcedChunks`),
    /// loaded or not; a mod's own ticket on it does not count. None for a stale handle or on a client.
    pub(in crate::game) fn chunk_forced(&self, chunk: ChunkRef) -> Option<bool> {
        let [x, z] = chunk.pos();
        Some(
            self.native_level_streaming(chunk.level())?
                .commanded()
                .contains(&(x, z)),
        )
    }
    /// `/forceload add` or `remove` of one chunk (`ServerLevel.setChunkForced`):
    /// true when the forced set changed. A forced chunk is kept loaded and
    /// entity ticking, saved with the level, and the level keeps ticking
    /// without a player. This is the world's own `/forceload` state, not a
    /// ticket the mod owns: the chunk stays forced across restarts, and after
    /// the mod is unloaded or removed, until `set_forced(false)` or
    /// `/forceload remove` clears it. A newly forced chunk loads before
    /// returning, or when streaming starts if it is still paused.
    /// False for a stale handle, on a client, outside the world, or when
    /// adding after generation has stopped.
    pub(in crate::game) fn chunk_set_forced(&mut self, chunk: ChunkRef, forced: bool) -> bool {
        if self.native_level_streaming(chunk.level()).is_none() || !chunk_in_world(chunk.pos()) {
            return false;
        }
        let [x, z] = chunk.pos();
        if chunk.level().dimension() == self.common.dim.key() {
            self.server
                .streaming
                .set_forced_loaded(&mut self.common.world, x, z, forced)
        } else {
            self.server
                .levels
                .0
                .force_parked_chunk(chunk.level().dimension_id(), x, z, forced)
                .unwrap_or(false)
        }
    }
    fn mod_ticket_streaming_mut(&mut self, dim: super::Dim) -> Option<&mut ChunkStreamer> {
        if dim == self.common.dim {
            Some(&mut self.server.streaming)
        } else {
            self.server.levels.0.parked_streaming_of_mut(dim.id())
        }
    }

    /// The mods validate the chunk tickets `dim`'s level loaded from its save,
    /// controller by controller (`LoadingValidationCallback.validateTickets`),
    /// and the survivors take hold. Tickets a mod added since the load (in
    /// `level_load`, say) are not candidates, and a controller registered
    /// without a validator keeps all of its tickets. Nothing is built while no
    /// mod listens.
    pub(in crate::game::server_side) fn validate_mod_chunk_tickets(&mut self, dim: super::Dim) {
        let Some(streaming) = self.mod_ticket_streaming_mut(dim) else {
            return;
        };
        let mut loaded = streaming.take_loaded_tickets();
        if !loaded.is_empty() && mod_api::__private::listened(mod_api::HookId::ValidateTickets) {
            let mut controllers: Vec<u32> = loaded.iter().map(|t| t.controller).collect();
            controllers.sort_unstable();
            controllers.dedup();
            for slot in controllers {
                let Some(controller) = TicketController::from_slot(slot) else {
                    continue;
                };
                let Some(owner) = controller.owner().filter(|_| controller.validates()) else {
                    continue;
                };
                let (own, rest): (Vec<_>, Vec<_>) =
                    loaded.into_iter().partition(|t| t.controller == slot);
                let mut validation =
                    mod_api::world::mod_chunk_tickets::ControllerTickets::new(owner, own);
                self.native_validate_tickets(dim.id(), controller, &mut validation);
                loaded = rest;
                loaded.extend(validation.into_loaded());
            }
        }
        if let Some(streaming) = self.mod_ticket_streaming_mut(dim) {
            for ticket in loaded {
                streaming.set_mod_forced(ticket, true);
            }
        }
    }
    /// The chunk ticket of `controller` for `owner` on `chunk`
    /// (`TicketController.forceChunk`): true when the controller's tickets
    /// changed. The controller must be the calling mod's own.
    /// A chunk stays forced, entity ticking and saved, and its level keeps
    /// ticking, while any ticket or `/forceload` holds it. With
    /// `natural_spawning` the chunk joins the spawning chunks (inhabited
    /// time, thunder, a spawn attempt) while it lies in the spawning area of a
    /// player (`ChunkMap.collectSpawningChunks`); the local mob cap still lets
    /// mobs spawn only with a non-spectator within 128 blocks. One
    /// owner holds a plain and a natural-spawning ticket on a chunk apart. Tickets are
    /// scoped by (controller, owner, chunk), so two owners in one chunk, or
    /// two controllers, release separately. They are saved in
    /// `data/sunrise/mod_chunk_tickets.dat` under the controller's id; when
    /// the world loads, those of a controller no loaded mod registers are
    /// discarded. False outside a mod's hook, for another mod's controller, on
    /// a client, for a chunk outside the world and for a level the server has
    /// not opened.
    pub(in crate::game) fn chunk_set_mod_forced(
        &mut self,
        chunk: ChunkRef,
        controller: TicketController,
        owner: mod_api::TicketOwner,
        forced: bool,
        natural_spawning: bool,
    ) -> bool {
        if !calling_mod_owns(controller) {
            return false;
        }
        if self.native_level_streaming(chunk.level()).is_none() || !chunk_in_world(chunk.pos()) {
            return false;
        }
        let [x, z] = chunk.pos();
        let ticket = ModTicket {
            controller: controller.slot(),
            owner,
            chunk: (x, z),
            natural_spawning,
        };
        let dimension = chunk.level().dimension_id();
        let streaming = if dimension == self.common.dim.id() {
            &mut self.server.streaming
        } else if let Some(streaming) = self.server.levels.0.parked_streaming_of_mut(dimension) {
            streaming
        } else {
            return false;
        };
        streaming.set_mod_forced(ticket, forced)
    }
    /// The chunk tickets `controller` holds in the level, as (owner, chunk
    /// position, natural_spawning), in order. None for a stale handle, on a
    /// client, outside a mod's hook and for another mod's controller.
    pub(in crate::game) fn level_mod_forced_chunks(
        &self,
        level: LevelRef,
        controller: TicketController,
    ) -> Option<Vec<(mod_api::TicketOwner, [i32; 2], bool)>> {
        if !calling_mod_owns(controller) {
            return None;
        }
        let tickets = self.native_level_streaming(level)?.mod_tickets();
        Some(
            tickets
                .of_controller(controller.slot())
                .map(|t| (t.owner, [t.chunk.0, t.chunk.1], t.natural_spawning))
                .collect(),
        )
    }
    /// The level's force-loaded chunks (`/forceload query`), sorted as
    /// the command lists them. None for a stale handle or on a client.
    pub(in crate::game) fn level_forced_chunks(&self, level: LevelRef) -> Option<Vec<[i32; 2]>> {
        let mut forced: Vec<(i32, i32)> = self
            .native_level_streaming(level)?
            .commanded()
            .iter()
            .copied()
            .collect();
        forced.sort_unstable_by_key(|&(x, z)| ChunkPos::new(x, z).pack());
        Some(forced.into_iter().map(|(x, z)| [x, z]).collect())
    }

    /// The block state ID at `pos` (`Level.getBlockState`), without formatting a string.
    /// None for a chunk that isn't loaded or a `y` outside the world. On a client, the state
    /// the server sent for a column the client holds, of its own level or a viewed one.
    pub(in crate::game) fn server_level_view_world(&self, level: LevelRef) -> Option<&World> {
        self.native_level_world(level)
    }

    pub(in crate::game) fn server_level_handle_current(&self, level: LevelRef) -> bool {
        self.level_handle_current(level)
    }

    pub(in crate::game) fn server_level_datapack_set(
        &self,
        level: LevelRef,
    ) -> Option<std::sync::Arc<mod_api::DatapackRegistrySet>> {
        self.native_level_world(level)?;
        self.common.mods.datapack_registries.clone()
    }

    /// `BlockState.isRedstoneConductor(level, pos)` of the block at `pos`. None for a stale
    /// handle, a client or an unloaded `pos`.
    pub(in crate::game) fn level_is_redstone_conductor(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<bool> {
        let state = self.level_block_state(level, pos)?;
        Some(self.common.res.behaviours().is_redstone_conductor(state))
    }
    /// Preview a held tool's transformation without changing the block or stack. The existing
    /// block-tool hook fires with `simulated` true; listeners must not mutate the level in a
    /// simulation. The player must be live in the level in place, and the block must be loaded;
    /// parked levels cannot provide the hook's level context. A stale handle, invalid face,
    /// unavailable player or level, or no transformation returns None.
    pub(in crate::game) fn level_block_transform(
        &self,
        level: LevelRef,
        player: PlayerRef,
        hand: Hand,
        stack: &ItemStack,
        pos: [i32; 3],
        hit: BlockHit,
    ) -> Option<BlockTransformPreview> {
        use super::native_server::PlayerRead;
        let world = self.native_level_read_world(level)?;
        if level.dimension() != self.common.dim.key()
            || !world.is_loaded_at(pos[0], pos[1], pos[2])
            || !matches!(
                hit.face,
                [1, 0, 0] | [-1, 0, 0] | [0, 1, 0] | [0, -1, 0] | [0, 0, 1] | [0, 0, -1]
            )
        {
            return None;
        }
        let (offhand, sneaking) = match self.player_handle_read(player)? {
            PlayerRead::Acting { .. } if self.common.dim.key() == level.dimension() => {
                (self.common.offhand.as_ref(), self.server_acting_sneak())
            }
            PlayerRead::Guest(guest) if guest.dimension == level.dimension_id() => {
                (guest.offhand.as_ref(), guest.sneaking)
            }
            _ => return None,
        };
        let blocking = !sneaking && offhand.is_some_and(|s| self.blocks_attacks(s).is_some());
        let hit = super::Hit {
            pos,
            face: hit.face,
            point: hit.point,
            inside: hit.inside,
        };
        let (state, sound, loot, durability, particle, _) = self.common.block_transform_in(
            stack,
            &hit,
            super::TransformContext {
                world,
                player: Some(player.entity().id()),
                hand,
                blocking,
                simulated: true,
            },
        )?;
        use sunrise_gameplay_common::game::api_values::TryIntoApi;
        Some(BlockTransformPreview::new(
            state.try_into_api().ok()?,
            sound,
            loot,
            durability,
            particle,
        ))
    }
    /// `f` over the game's own `SignalGetter` for the level in place, as the level's blocks
    /// and mods' parts answer it; `None` for another level, a stale handle or a client.
    /// A shared borrow also lends Parts their restricted read-only level host. Comparator
    /// outputs and moving-piston state come from this level's live tick state.
    fn native_level_signals<R>(
        &self,
        level: LevelRef,
        f: impl FnOnce(
            &sunrise_world::level::Signals<
                '_,
                super::native_level_signals::ReadLevel<
                    '_,
                    '_,
                    crate::game::server_side::ServerState,
                >,
            >,
        ) -> R,
    ) -> Option<R> {
        if !self.level_in_place(level) {
            return None;
        }
        let view = super::native_level_signals::ReadLevel::new(self, level);
        Some(f(&self.common.res.behaviours().signals(&view, &self.common.block_ticks)))
    }
    /// How strongly the block at `pos` powers the block on its `direction` side, power it
    /// conducts included (`SignalGetter.getSignal(pos, direction)`, as a block reads its
    /// neighbour `pos` that lies `direction` from it), 0 to 15. None for another level, a stale
    /// handle or a client. Takes a shared host and opens no transaction.
    pub(in crate::game) fn level_signal(
        &self,
        level: LevelRef,
        pos: [i32; 3],
        direction: Direction,
    ) -> Option<i32> {
        self.native_level_signals(level, |s| s.signal(block_pos(pos), direction))
    }
    /// The strong power alone that the block at `pos` gives that way
    /// (`SignalGetter.getDirectSignal(pos, direction)`), 0 to 15. None as for `signal`.
    pub(in crate::game) fn level_direct_signal(
        &self,
        level: LevelRef,
        pos: [i32; 3],
        direction: Direction,
    ) -> Option<i32> {
        self.native_level_signals(level, |s| s.direct_signal(block_pos(pos), direction))
    }
    /// The strongest strong power reaching the block at `pos` from its six neighbours
    /// (`SignalGetter.getDirectSignalTo`), 0 to 15. None as for `signal`.
    pub(in crate::game) fn level_direct_signal_to(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<i32> {
        self.native_level_signals(level, |s| s.direct_signal_to(block_pos(pos)))
    }
    /// The strongest power reaching the block at `pos` from its six neighbours
    /// (`SignalGetter.getBestNeighborSignal`, what a dispenser or a piston reads), 0 to 15.
    /// None as for `signal`.
    pub(in crate::game) fn level_best_neighbor_signal(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<i32> {
        self.native_level_signals(level, |s| s.best_neighbor_signal(block_pos(pos)))
    }
    /// Whether any neighbour powers the block at `pos` (`SignalGetter.hasNeighborSignal`),
    /// asking no side past the first that does. False where `signal` is None.
    pub(in crate::game) fn level_has_neighbor_signal(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> bool {
        self.native_level_signals(level, |s| s.has_neighbor_signal(block_pos(pos)))
            .unwrap_or(false)
    }

    /// `LevelLightEngine.checkBlock` at `pos`, queued with the level's light edits: the next
    /// update asks a `dynamic_light` block's parts again (`ask_edited_light_emission`) and
    /// relights the cell, which `LightTables::differs` lets through for such a state set to
    /// itself. A check already queued there since the last ask queues nothing, as vanilla's
    /// queue is a set. False off the level in place, for a stale handle, on a client and for a
    /// position that isn't loaded.
    pub(in crate::game) fn level_check_light(&mut self, level: LevelRef, pos: [i32; 3]) -> bool {
        if !self.native_level_write_allowed(level, pos) || self.common.dim.key() != level.dimension() {
            return false;
        }
        let [x, y, z] = pos;
        let edits = &self.common.world.light_edits;
        if let Some(&i) = self.common.world.light_checks.get(&(x, y, z))
            && edits.get(i).is_some_and(|edit| edit.0 == (x, y, z))
        {
            return true;
        }
        self.common.world.light_checks.insert((x, y, z), edits.len());
        let state = self.common.world.block(x, y, z).0;
        self.common.world.light_edits.push(((x, y, z), state, state));
        true
    }
    /// The capability generation at `pos`, in an active or parked level. None for a stale
    /// handle, a client, an unloaded position and one outside build height.
    pub(in crate::game) fn level_capability_generation(
        &self,
        level: LevelRef,
        [x, y, z]: [i32; 3],
    ) -> Option<u64> {
        let world = self.native_level_world(level)?;
        (world.loaded(x, z) && world.shape.contains_y(y))
            .then(|| world.capability_generations.get((x, y, z)))
    }
    /// Moves the capability generation at `pos` on, at the root commit inside a transaction.
    /// False off the level in place, for a stale handle, on a client and for a position that
    /// isn't loaded.
    pub(in crate::game) fn level_invalidate_capabilities(
        &mut self,
        level: LevelRef,
        [x, y, z]: [i32; 3],
    ) -> bool {
        if !self.level_in_place(level)
            || !self.common.world.loaded(x, z)
            || !self.common.world.shape.contains_y(y)
        {
            return false;
        }
        self.after_commit(super::transactions::After::Capabilities((x, y, z)));
        true
    }
    /// `LivingEntity.hasLineOfSight`'s clip between two points (collider shapes, no fluids):
    /// whether it misses. None as for `level_clip`.
    pub(in crate::game) fn level_line_of_sight(
        &self,
        level: LevelRef,
        from: [f64; 3],
        to: [f64; 3],
    ) -> Option<bool> {
        let hit = self.level_clip(level, from, to, ClipBlock::Collider, ClipFluid::None)?;
        Some(hit.miss)
    }
    /// Whether `level` is a current handle on the server: what the world's own data (its
    /// respawn data and maps, shared by its levels) needs.
    fn native_world_data_allowed(&self, level: LevelRef) -> bool {
        self.common.is_logical_server() && self.level_handle_current(level)
    }
    /// `LevelData.getRespawnData`: the world spawn and its facing. None on a client, for a
    /// stale handle, and before the world has chosen its spawn.
    pub(in crate::game) fn level_respawn_data(&self, level: LevelRef) -> Option<RespawnData> {
        if !self.native_world_data_allowed(level) {
            return None;
        }
        let [yaw, pitch] = self.common.level.world_spawn_angle;
        let [x, y, z] = self.common.level.world_spawn?;
        let dimension = self
            .common.level
            .world_spawn_dimension
            .as_deref()
            .unwrap_or("minecraft:overworld");
        Some(RespawnData {
            position: GlobalPos::new(Identifier::parse(dimension).ok()?, BlockPos::new(x, y, z)),
            yaw,
            pitch,
        })
    }
    /// `/setworldspawn`'s write (`set_world_spawn`) in the handle's level. False for a stale
    /// handle, a client, a non-finite angle and a position outside the spawnable bounds.
    pub(in crate::game) fn level_set_respawn_data(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
        yaw: f32,
        pitch: f32,
    ) -> bool {
        if !self.native_world_data_allowed(level)
            || !(yaw.is_finite() && pitch.is_finite())
            || self.level_in_spawnable_bounds(level, pos) != Some(true)
        {
            return false;
        }
        self.set_world_spawn(super::Dim::from_id(level.dimension_id()), pos, [yaw, pitch]);
        true
    }
    /// Map `id`'s saved data as it stands (`with_map`). None on a client, for a stale handle
    /// and for a map nothing made.
    pub(in crate::game) fn level_map_data(&self, level: LevelRef, id: i32) -> Option<MapState> {
        if !self.native_world_data_allowed(level) {
            return None;
        }
        self.with_map(id, |map| MapState {
            center: map.center,
            scale: map.scale,
            dimension: map.dimension.clone(),
            locked: map.locked,
            tracking: map.tracking,
            unlimited: map.unlimited,
            colors: map.colors.clone(),
        })
    }
    /// Repaint map `id` (`MapData::paint`). False for another length, a stale handle, a
    /// client, a map nothing made and a map whose pixels are all as given.
    pub(in crate::game) fn level_set_map_data(
        &mut self,
        level: LevelRef,
        id: i32,
        colors: &[u8],
    ) -> bool {
        if !self.native_world_data_allowed(level) {
            return false;
        }
        self.map(id)
            .and_then(|map| map.paint(colors))
            .unwrap_or(false)
    }
    /// Queue the level to be saved at the start of the next server tick. False for a stale
    /// handle and on a client.
    pub(in crate::game) fn level_request_save(&mut self, level: LevelRef) -> bool {
        if self.native_level_world(level).is_none() {
            return false;
        }
        self.server
            .levels
            .0
            .request_save(super::Dim::from_id(level.dimension_id()));
        true
    }
    /// The due tick path for the block at `pos` now (`Behaviours::due_block_tick`). False for
    /// a refused write, a stale or parked handle, a client and an unloaded or out-of-height
    /// position.
    pub(in crate::game) fn level_block_tick(&mut self, level: LevelRef, pos: [i32; 3]) -> bool {
        if !self.native_level_tick_allowed(level, pos) {
            return false;
        }
        let at = block_pos(pos);
        self.common.with_level(|b, level, ticks| b.due_block_tick(level, ticks, at));
        let _ = self.server_process_level_events();
        true
    }
    /// The flow step of the fluid at `pos` now (`Behaviours::scheduled_fluid_tick`, the body of
    /// a due fluid tick). False as for `level_block_tick`.
    pub(in crate::game) fn level_fluid_tick(&mut self, level: LevelRef, pos: [i32; 3]) -> bool {
        if !self.native_level_tick_allowed(level, pos) {
            return false;
        }
        let at = block_pos(pos);
        self.common.with_level(|b, level, ticks| b.scheduled_fluid_tick(level, ticks, at));
        let _ = self.server_process_level_events();
        true
    }
    /// `tickChunk`'s random tick of `pos` now (`Behaviours::random_tick`). False as for
    /// `level_block_tick`.
    pub(in crate::game) fn level_block_random_tick(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> bool {
        if !self.native_level_tick_allowed(level, pos) {
            return false;
        }
        let at = block_pos(pos);
        self.common.with_level(|b, level, ticks| b.random_tick(level, ticks, at));
        let _ = self.server_process_level_events();
        true
    }
    /// Whether a tick may run at `pos`: a current handle of the level in place on the server,
    /// a loaded position inside the build height.
    fn native_level_tick_allowed(&self, level: LevelRef, pos: [i32; 3]) -> bool {
        self.native_world_data_allowed(level)
            && self.common.dim.key() == level.dimension()
            && self.common.world.loaded(pos[0], pos[2])
            && self.common.world.shape.contains_y(pos[1])
    }
    /// `ServerLevel.findLightningTargetAround` (`lightning_target`) for the column at `pos`,
    /// in the level in place. None for another level, a stale handle, a client and an
    /// unloaded column.
    pub(in crate::game) fn level_lightning_target(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<[i32; 3]> {
        if !self.native_world_data_allowed(level) || self.common.dim.key() != level.dimension() {
            return None;
        }
        self.lightning_target(pos[0], pos[2])
    }
    /// The time of the level's own clock (its dimension type's `default_clock`) in ticks: what the
    /// level's sky, spawning and beds go by, and what `/time` sets there. None for a level without
    /// a default clock, a stale handle and a client.
    pub(in crate::game) fn level_clock_time(&self, level: LevelRef) -> Option<i64> {
        self.native_level_read_world(level)?;
        if !self.common.is_logical_server() {
            return None;
        }
        let dim = super::Dim::from_id(level.dimension_id());
        self.common.dimension_type_of(dim).default_clock.as_ref()?;
        Some(self.common.command_clock_time(&self.common.level_clock_name(dim)))
    }
}

use sunrise_gameplay_common::game::transactions::TransactionsGameExt as _;

use sunrise_gameplay_common::game::held_items::HeldItemsGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_level_refs_common::NativeLevelRefsCommonGameExt1 as _;
