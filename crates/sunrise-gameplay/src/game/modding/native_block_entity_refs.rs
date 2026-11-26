//! Block entity handles: storage writes join the journal; explicit world notifications happen
//! immediately and survive abort. Raw mod storage writes and restores synthesize no notice.
use crate::game::ServerGame;
use mod_api::__private::game::*;
use mod_api::{
    BlockEntityDataError, BlockEntityError, BlockEntityItemError, BlockEntityRef,
    BlockEntityShapes, BlockEntityShapesError, PlayerRef, TransactionContext,
};
use sunrise_core::nbt::{Compound, Tag};
use sunrise_core::position::BlockPos;
use sunrise_game_data::block_entity_nbt;
use sunrise_gameplay_common::game::native_block_entity_read::{
    loaded_block_entity, mod_item_slots, mod_payload, save_with_metadata, save_without_metadata,
};
use sunrise_items::items::ItemStack;
use sunrise_level::containers::{BlockEntity, ModBlockEntity};
use sunrise_sim::save;
use sunrise_world::registry::BlockStateId;

impl ServerGame<'_> {
    /// The data of the block entity at `pos`, as saved without identity or position
    /// metadata, for the hook that hears its block's removal: the entity is gone from
    /// the world by then, so it is read first. None for a block without one, or while
    /// no mod listens.
    pub(in crate::game::server_side) fn removed_block_entity_data(
        &self,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<Compound> {
        if !mod_api::__private::listened(mod_api::HookId::PlayerDestroyedBlock) {
            return None;
        }
        let entity = loaded_block_entity(&self.common.world, pos)?;
        let block = &self.common.pack.assets.registry.state(state)?.name;
        save_without_metadata(
            &self.common.world,
            (pos[0], pos[1], pos[2]),
            entity,
            block,
            self.common.res.items(),
        )
    }

    /// The saved data of `entity` at `key` without identity or position metadata, with its
    /// `CustomName`, for a block `block`: what a handle to it reads.
    pub(in crate::game) fn saved_block_entity_data(
        &self,
        key: (i32, i32, i32),
        entity: &BlockEntity,
        block: &str,
    ) -> Option<Compound> {
        save_without_metadata(
            &self.common.world,
            key,
            entity,
            block,
            self.common.res.items(),
        )
    }

    /// Opens a block entity's menu for a connected player (`ServerPlayer.openMenu`).
    /// Returns Stale for an invalidated entity; Ok(false) for a wrong-level player,
    /// non-provider, cancelled open, or an unconnected player such as a fake player.
    pub(in crate::game) fn player_open_menu(
        &mut self,
        player: PlayerRef,
        entity: BlockEntityRef,
    ) -> Result<bool, BlockEntityError> {
        self.native_block_entity_target(entity)
            .ok_or(BlockEntityError::Stale)?;
        if self.player_handle_read(player).is_none() || !self.level_in_place(entity.fields().level)
        {
            return Ok(false);
        }
        let Some(conn) = self.server.lan.as_ref().and_then(|host| {
            host.hub
                .guests
                .iter()
                .find(|guest| {
                    guest.uuid == player.fields().uuid
                        && guest.placed
                        && guest.dimension == entity.fields().level.dimension_id()
                })
                .map(|guest| guest.conn)
        }) else {
            return Ok(false);
        };
        if self.menu_click_in_progress(conn) {
            return Ok(false);
        }
        let [x, y, z] = entity.fields().position;
        match self.open_block_menu(conn, (x, y, z)) {
            sunrise_gameplay_common::game::lan_actions::MenuOpen::Stale => {
                Err(BlockEntityError::Stale)
            }
            opened => Ok(opened == sunrise_gameplay_common::game::lan_actions::MenuOpen::Opened),
        }
    }

    /// Change the lock through the same storage used by container opening and saving.
    pub(in crate::game) fn block_entity_set_lock(
        &mut self,
        entity: BlockEntityRef,
        lock: Option<Compound>,
    ) -> Result<bool, BlockEntityError> {
        let (key, block) = self
            .native_block_entity_write_target(entity)
            .ok_or(BlockEntityError::Stale)?;
        if matches!((self.world.block_lock(key), &lock), (None, None))
            || matches!((self.world.block_lock(key), &lock), (Some(Tag::Compound(old)), Some(new)) if old == new)
        {
            return Ok(false);
        }
        let Some(mut tag) = self
            .common
            .world
            .block_entities
            .get(&key)
            .and_then(|current| {
                save_with_metadata(
                    &self.common.world,
                    key,
                    current,
                    &block,
                    self.common.res.items(),
                )
            })
        else {
            return Ok(false);
        };
        match lock {
            Some(lock) => {
                tag.insert("lock".into(), Tag::Compound(lock));
            }
            None => {
                tag.remove(&"lock".into());
            }
        }
        self.journal_block_entity(key);
        self.common.world.set_block_lock(key, &Tag::Compound(tag));
        self.block_entity_updated(key);
        self.container_changed(key);
        Ok(true)
    }

    /// A loaded block entity of the server level in place, when a write may change it.
    pub(in crate::game) fn native_block_entity_write_key(
        &self,
        entity: BlockEntityRef,
    ) -> Option<(i32, i32, i32)> {
        if !self.level_in_place(entity.fields().level) {
            return None;
        }
        let pos = entity.fields().position;
        self.native_block_entity_target(entity)?;
        Some((pos[0], pos[1], pos[2]))
    }

    /// The block entity's key and block name when a write may change it.
    fn native_block_entity_write_target(
        &self,
        entity: BlockEntityRef,
    ) -> Option<((i32, i32, i32), String)> {
        let key = self.native_block_entity_write_key(entity)?;
        let block = self
            .common
            .pack
            .assets
            .registry
            .state(self.common.world.block(key.0, key.1, key.2))?
            .name
            .clone();
        Some((key, block))
    }

    /// Set a mod block entity's menu data slot silently in the current transaction.
    /// Ok(false) for a vanilla entity, invalid slot or equal value; Stale for an invalidated
    /// or unwritable handle. Notification is explicit; this does not resize slots.
    pub(in crate::game) fn block_entity_set_data_slot(
        &mut self,
        entity: BlockEntityRef,
        slot: u32,
        value: i32,
    ) -> Result<bool, BlockEntityError> {
        let key = self
            .native_block_entity_write_key(entity)
            .ok_or(BlockEntityError::Stale)?;
        let Some(block_entity) = self.mod_block_entity_write_target(key) else {
            return Ok(false);
        };
        let index = slot as usize;
        if block_entity
            .data
            .get(index)
            .is_none_or(|current| *current == value)
        {
            return Ok(false);
        }
        self.write_mod_block_entity(key, |block_entity| block_entity.data[index] = value);
        Ok(true)
    }

    /// Sizes a mod block entity's transient menu data using the same silent
    /// write and journal as scalar data-slot writes.
    pub(in crate::game) fn block_entity_set_data_slots(
        &mut self,
        entity: BlockEntityRef,
        values: Vec<i32>,
    ) -> Result<bool, BlockEntityDataError> {
        let key = self
            .native_block_entity_write_key(entity)
            .ok_or(BlockEntityDataError::Stale)?;
        let block_entity = self
            .mod_block_entity_write_target(key)
            .ok_or(BlockEntityDataError::Unavailable)?;
        if block_entity.data == values {
            return Ok(false);
        }
        self.write_mod_block_entity(key, |block_entity| block_entity.data = values);
        Ok(true)
    }

    fn mod_block_entity_write_target(&self, key: (i32, i32, i32)) -> Option<&ModBlockEntity> {
        let BlockEntity::Mod(block_entity) = self.common.world.block_entities.get(&key)? else {
            return None;
        };
        Some(block_entity)
    }

    // The target was resolved above, and journaling invokes no listener that could replace it.
    fn write_mod_block_entity(
        &mut self,
        key: (i32, i32, i32),
        edit: impl FnOnce(&mut ModBlockEntity),
    ) {
        self.journal_block_entity(key);
        self.edit_mod_block_entity(key, edit);
    }

    /// `edit` on a resolved mod holder; the caller journaled the storage. Notification is the
    /// mod's explicit responsibility, including any notice it wants while restoring storage.
    fn edit_mod_block_entity(
        &mut self,
        key: (i32, i32, i32),
        edit: impl FnOnce(&mut ModBlockEntity),
    ) {
        let BlockEntity::Mod(block_entity) =
            self.common.world.block_entities.get_mut(&key).unwrap()
        else {
            unreachable!("resolved mod holder replaced before its write")
        };
        edit(block_entity);
    }

    /// Replace internal storage under the existing transaction; external ITEMS routing is separate.
    pub(in crate::game) fn block_entity_set_item_slot(
        &mut self,
        entity: BlockEntityRef,
        tx: &TransactionContext,
        slot: u32,
        stack: Option<ItemStack>,
    ) -> Result<bool, BlockEntityItemError> {
        if !self.transaction_context_is_current(tx) {
            return Err(BlockEntityItemError::InvalidContext);
        }
        let key = self
            .native_block_entity_write_key(entity)
            .ok_or(BlockEntityItemError::Stale)?;
        let block_entity = self
            .mod_block_entity_write_target(key)
            .ok_or(BlockEntityItemError::Unavailable)?;
        let slots = mod_item_slots(&self.common.world, key, slot)?;
        if let Some(stack) = &stack {
            if stack.count == 0
                || stack.item.as_ref() == "minecraft:air"
                || self.items().get(&stack.item).is_none()
            {
                return Err(BlockEntityItemError::InvalidStack);
            }
            let limit = self.stack_max_size(stack);
            if stack.count > limit {
                return Err(BlockEntityItemError::StackTooLarge {
                    count: stack.count,
                    limit,
                });
            }
        }
        let index = slot as usize;
        if block_entity.items.get(index).and_then(Option::as_ref) == stack.as_ref()
            && !block_entity.has_opaque_item_slot(index)
        {
            return Ok(false);
        }
        // A slot of a list already its full length: the journal keeps that slot, not the
        // entity's whole saved data. A list still to grow is kept whole, length and all.
        if index < block_entity.items.len() {
            self.journal_block_entity_slot(key, index);
            self.edit_mod_block_entity(key, |block_entity| {
                *block_entity.item_slot_mut(index).unwrap() = stack;
            });
        } else {
            self.write_mod_block_entity(key, |block_entity| {
                block_entity.items.resize(slots, None);
                *block_entity.item_slot_mut(index).unwrap() = stack;
            });
        }
        self.journal_chunk_save(
            (key.0 >> 4, key.2 >> 4),
            sunrise_gameplay_common::game::transactions::SaveChunkScope::Level,
        );
        Ok(true)
    }

    /// Merges `data` into the block entity's saved data and loads it, as `/data merge block`
    /// does (`BlockDataAccessor.setData`: `loadWithComponents`, `setChanged`, then
    /// `sendBlockUpdated`, so comparators re-read it and clients get the update). `id`, `x`,
    /// `y` and `z` in `data` are ignored. Ok(true) when data changed; Ok(false) for data
    /// nested too deep, no change, or data the entity cannot load. Stale or unwritable
    /// handles return BlockEntityError::Stale. Inside a transaction an
    /// abort puts the block entity, its name, `lock`, loot table roll and attachments back.
    pub(in crate::game) fn block_entity_merge_data(
        &mut self,
        entity: BlockEntityRef,
        data: Compound,
    ) -> Result<bool, BlockEntityError> {
        self.write_block_entity_data(entity, data, false)
    }

    /// Replaces the saved payload, retaining identity and position metadata.
    /// Uses the same transactional load and client update path as `merge_data`.
    /// Ok(false) for unsupported data or no change; Stale for an invalidated handle.
    pub(in crate::game) fn block_entity_replace_data(
        &mut self,
        entity: BlockEntityRef,
        data: Compound,
    ) -> Result<bool, BlockEntityError> {
        self.write_block_entity_data(entity, data, true)
    }

    fn write_block_entity_data(
        &mut self,
        entity: BlockEntityRef,
        data: Compound,
        replace: bool,
    ) -> Result<bool, BlockEntityError> {
        let (key, block) = self
            .native_block_entity_write_target(entity)
            .ok_or(BlockEntityError::Stale)?;
        let nbt = Tag::Compound(mod_payload(data));
        if sunrise_game_data::nbt_path::too_deep(&nbt, 0) {
            return Ok(false);
        }
        let Some(old) = self
            .common
            .world
            .block_entities
            .get(&key)
            .and_then(|current| {
                save_with_metadata(
                    &self.common.world,
                    key,
                    current,
                    &block,
                    self.common.res.items(),
                )
            })
            .map(Tag::Compound)
        else {
            return Ok(false);
        };
        let merged = loaded_payload(&old, &nbt, replace);
        if merged == old {
            return Ok(false);
        }
        self.journal_block_entity(key);
        let cleared = matches!(
            self.world.block_entities.get(&key),
            Some(BlockEntity::Mod(current)) if current.shapes_cleared()
        );
        if !self.load_block_entity_data(key, &merged) {
            return Ok(false);
        }
        if cleared
            && let Some(BlockEntity::Mod(loaded)) = self.common.world.block_entities.get_mut(&key)
        {
            loaded.keep_shapes_cleared(true);
        }
        self.block_entity_updated(key);
        self.container_changed(key);
        Ok(true)
    }

    /// `BlockEntity.setChanged`: notify at once, including inside a transaction. An abort
    /// restores storage, not notifications already delivered to observers or comparators.
    pub(in crate::game) fn block_entity_set_changed(
        &mut self,
        entity: BlockEntityRef,
    ) -> Result<bool, BlockEntityError> {
        let key = self
            .native_block_entity_write_key(entity)
            .ok_or(BlockEntityError::Stale)?;
        self.container_changed(key);
        Ok(true)
    }

    /// Register a generation-bound observer in its level's keyed contents index.
    pub(in crate::game) fn block_entity_watch_contents(
        &mut self,
        entity: BlockEntityRef,
        source: mod_api::BlockPos,
        enabled: bool,
    ) -> Result<(), mod_api::ContentsWatchError> {
        let fields = entity.fields();
        if !self.level_in_place(fields.level)
            || self.block_entity_identity(fields.level, fields.position) != Some(entity)
        {
            return Err(mod_api::ContentsWatchError::Stale);
        }
        let [x, y, z] = fields.position;
        if !sunrise_gameplay_common::game::api_values::block_state::block_listens(
            mod_api::HookId::ContentsChanged,
            self.common.world.block(x, y, z),
        ) {
            return Err(mod_api::ContentsWatchError::NotSubscribed);
        }
        self.journal_block_entity((x, y, z));
        let observer = self
            .common
            .world
            .block_entities
            .identity((x, y, z))
            .unwrap();
        self.common.world.block_entities.contents_watches.set(
            observer,
            (source.x, source.y, source.z),
            enabled,
        );
        Ok(())
    }

    /// Publishes the shapes the block entity's block has in place of its declared ones, or
    /// clears them with None. Applied at once, to the queries that follow, and the block's
    /// neighbours are notified; clients get them with the block entity update. Inside a
    /// transaction an abort puts the old ones back.
    pub fn block_entity_set_shapes(
        &mut self,
        entity: BlockEntityRef,
        shapes: Option<BlockEntityShapes>,
    ) -> Result<bool, BlockEntityShapesError> {
        let key = self
            .native_block_entity_write_key(entity)
            .ok_or(BlockEntityShapesError::Stale)?;
        let state = self.common.world.block(key.0, key.1, key.2);
        if !self.common.collision.publishes_shapes(state) {
            return Err(BlockEntityShapesError::NotDeclared);
        }
        let shapes = shapes
            .map(sunrise_level::containers::PublishedShapes::new)
            .transpose()?
            .map(std::sync::Arc::new);
        let Some(current) = self.mod_block_entity_write_target(key) else {
            return Err(BlockEntityShapesError::Stale);
        };
        if current.shapes() == shapes.as_deref() {
            return Ok(false);
        }
        if cfg!(debug_assertions)
            && shapes
                .as_ref()
                .is_some_and(|shapes| shapes.reaches_past_one_cell())
        {
            static WARNED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
            let block = self
                .common
                .pack
                .assets
                .registry
                .state(state)
                .map(|s| s.name.clone());
            if let (Some(block), Ok(mut warned)) = (block, WARNED.lock())
                && !warned.contains(&block)
            {
                log::warn!(
                    "{block}: a published shape has a box beyond one block of the block; collision never finds it"
                );
                warned.push(block);
            }
        }
        self.write_mod_block_entity(key, |block_entity| {
            block_entity.set_shapes(shapes);
        });
        self.block_entity_updated(key);
        // Fluids and other neighbours read the shape: tell them it changed (the shulker lid's
        // `doNeighborUpdates`, without its shape updates).
        let at = BlockPos::new(key.0, key.1, key.2);
        self.with_level(|b, level, ticks| {
            let block = b.block(state);
            b.update_neighbors_at(level, ticks, at, block);
        });
        Ok(true)
    }

    /// The custom name (`Nameable.getCustomName`) as plain text. None when the block entity
    /// has none or isn't nameable (chests, barrels, furnaces, hoppers, brewing stands,
    /// banners, beacons, enchanting tables, command blocks...), or for a stale handle or a
    /// client.
    pub(in crate::game) fn block_entity_name(&self, entity: BlockEntityRef) -> Option<String> {
        let (world, block_entity) = self.native_block_entity_target(entity)?;
        let [x, y, z] = entity.fields().position;
        match block_entity {
            BlockEntity::Banner(banner) => banner.name.clone(),
            _ => world.block_names.get(&(x, y, z)).map(|name| match name {
                Tag::String(text) => text.to_string_lossy(),
                component => sunrise_game_data::component::name_text(component).plain(),
            }),
        }
    }

    /// The players whose open menu holds this block entity's container (for openers'
    /// containers, `ContainerOpenersCounter.getEntitiesWithContainerOpen`'s players), from the menus the opener counts read
    /// (`tick_mod_container_viewers`, `ChestBlockEntity$1.isOwnContainer`). Guests in
    /// its level, not spectators nor fake players, then the acting player for a menu
    /// their action opened, which the hub records when the action ends. None for a
    /// stale handle or a client.
    pub(in crate::game) fn block_entity_viewers(
        &self,
        entity: BlockEntityRef,
    ) -> Option<Vec<PlayerRef>> {
        if !self.common.is_logical_server() {
            return None;
        }
        self.native_block_entity_target(entity)?;
        let epoch = self.common.native_lifecycle.epoch()?;
        let fields = entity.fields();
        let level = fields.level;
        let [x, y, z] = fields.position;
        let key = (x, y, z);
        let mut viewers: Vec<PlayerRef> = self
            .server
            .lan
            .iter()
            .flat_map(|host| host.hub.guests.simulation_iter().map(move |g| (host, g)))
            .filter(|(host, g)| {
                // The acting guest's mode is live in `Game` during its turn; the hub
                // hears of a change only in the put-back at the end of `guest_turn`.
                let spectator = if self.server.lan_acting_uuid == Some(g.uuid) {
                    self.common.spectator()
                } else {
                    g.spectator()
                };
                g.participates_online()
                    && g.placed
                    && !spectator
                    && g.dimension == level.dimension_id()
                    && host.menus.get(&g.conn).is_some_and(|menus| {
                        menus.menu_holds(g.menu.as_ref(), key, fields.generation)
                    })
            })
            .filter_map(|(_, g)| {
                let id = u32::try_from(g.entity_id).ok()?;
                Some(player_ref(mod_api::EntityRef::new(epoch, id), g.uuid))
            })
            .collect();
        if self.level_in_place(level)
            && !self.server.lan_acting_fake
            && !self.common.spectator()
            && self
                .common
                .container
                .is_some_and(|open| open == key || self.common.chest_other_half(open) == Some(key))
            && let Some(player) = self.native_acting_player()
            && !viewers.contains(&player)
        {
            viewers.push(player);
        }
        Some(viewers)
    }

    /// Sets the custom name, or clears it with None, as an anvil-renamed item placed with
    /// `components` sets `CustomName`. Plain text only. Clients get a block entity update.
    /// Ok(false) for an entity that is not nameable; Stale for an invalidated or
    /// unwritable handle. Inside a transaction an abort puts the name back.
    pub(in crate::game) fn block_entity_set_name(
        &mut self,
        entity: BlockEntityRef,
        name: Option<&str>,
    ) -> Result<bool, BlockEntityError> {
        let (key, block) = self
            .native_block_entity_write_target(entity)
            .ok_or(BlockEntityError::Stale)?;
        let nameable = self
            .common
            .world
            .block_entities
            .get(&key)
            .is_some_and(|current| nameable(key, current, &block, self.common.res.items()));
        if !nameable {
            return Ok(false);
        }
        self.journal_block_entity(key);
        match self.common.world.block_entities.get_mut(&key) {
            Some(BlockEntity::Banner(banner)) => banner.name = name.map(str::to_owned),
            _ => {
                match name {
                    Some(name) => self
                        .common
                        .world
                        .block_names
                        .insert(key, Tag::String(name.into())),
                    None => self.common.world.block_names.remove(&key),
                };
            }
        }
        self.block_entity_updated(key);
        Ok(true)
    }
}

/// The compound a merge (or, with `replace`, a replace) of `payload` loads over the block entity
/// saved as `old`. A replace keeps the identity, position and published shapes, which are not
/// the payload's to replace.
fn loaded_payload(old: &Tag, payload: &Tag, replace: bool) -> Tag {
    let base = if replace {
        let mut metadata = Compound::new();
        if let Tag::Compound(fields) = old {
            for key in ["id", "x", "y", "z", sunrise_level::containers::SHAPES_KEY] {
                if let Some(value) = fields.get(&key.into()) {
                    metadata.insert(key.into(), value.clone());
                }
            }
        }
        Tag::Compound(metadata)
    } else {
        old.clone()
    };
    sunrise_sim::data_cmd::merged(&base, payload)
}

#[cfg(test)]
mod payload_tests {
    use super::*;

    #[test]
    fn merge_and_replace_keep_the_published_shapes_whatever_the_payload_carries() {
        use sunrise_level::containers::SHAPES_KEY;
        let shapes = Tag::Compound(Compound::from([(
            "collision".into(),
            Tag::LongArray(vec![1, 2, 3, 4, 5, 6]),
        )]));
        let old = Tag::Compound(Compound::from([
            ("id".into(), Tag::String("test:bus".into())),
            ("x".into(), Tag::Int(1)),
            ("progress".into(), Tag::Int(7)),
            ("kept".into(), Tag::Int(9)),
            (SHAPES_KEY.into(), shapes.clone()),
        ]));
        // A payload that carries the key, to overwrite it or to remove it.
        let colliding = || {
            mod_payload(Compound::from([
                ("progress".into(), Tag::Int(8)),
                ("id".into(), Tag::String("other:type".into())),
                (SHAPES_KEY.into(), Tag::Compound(Compound::new())),
            ]))
        };
        for replace in [false, true] {
            let loaded = loaded_payload(&old, &Tag::Compound(colliding()), replace);
            assert_eq!(loaded.get(SHAPES_KEY), Some(&shapes), "replace: {replace}");
            // The ordinary field applies, and the identity stays.
            assert_eq!(loaded.get("progress"), Some(&Tag::Int(8)));
            assert_eq!(loaded.get("id"), Some(&Tag::String("test:bus".into())));
            // A merge keeps the fields the payload omits; a replace drops them.
            assert_eq!(loaded.get("kept").is_some(), !replace);
        }
    }
}

/// Whether the block entity keeps a `CustomName` (`Nameable`): its saved form does.
fn nameable(
    pos: (i32, i32, i32),
    entity: &BlockEntity,
    block: &str,
    items: &sunrise_items::items::Items,
) -> bool {
    let mut tag = save::block_entity_encode::encode(pos, entity, block, items);
    if let Tag::Compound(map) = &mut tag {
        map.insert("CustomName".into(), Tag::String("x".into()));
    }
    block_entity_nbt::canonical(&tag).is_some_and(|tag| tag.get("CustomName").is_some())
}

#[cfg(test)]
mod tests {
    use sunrise_gameplay_common::game::server_side::native_block_entity_refs::tests::compound;

    use super::*;
    use mod_api::HostTransaction;
    #[cfg(test)]
    use mod_api::LevelRef;
    use mod_api::ServerHost;
    use mod_api::TransactionError;
    use std::sync::Arc;
    use sunrise_level::world::World;
    use sunrise_level::{chunk::Chunk, containers::ModBlockEntityFields};
    #[cfg(test)]
    use sunrise_sim::lan::menu::Slots;
    use sunrise_world::registry::BlockStateId;

    #[test]
    fn saved_data_omits_metadata_but_keeps_contents() {
        let world = World::new(BlockStateId(0), 0);
        let entity = BlockEntity::Chest(vec![None; 27]);
        let data = save_without_metadata(
            &world,
            (1, 64, 2),
            &entity,
            "minecraft:chest",
            &Default::default(),
        )
        .unwrap();
        for key in ["id", "x", "y", "z"] {
            assert!(!data.contains_key(&key.into()));
        }
        assert!(data.contains_key(&"Items".into()));
    }

    /// What a reader of a vanilla machine's progress (a crafting CPU's estimate, a progress
    /// line) needs is in `BlockEntity.saveWithoutMetadata`, which `block_entity_data` returns:
    /// a furnace's cooking and burning timers and the fuel's speed, a brewing stand's brew
    /// timer counting down, its total, its fuel and its bottles.
    #[test]
    fn saved_data_carries_vanilla_machine_progress() {
        use sunrise_level::{
            brewing::BrewingStand,
            containers::{Furnace, FurnaceKind},
        };
        let world = World::new(BlockStateId(0), 0);
        let int = |data: &Compound, key: &str| match data.get(&key.into()) {
            Some(Tag::Int(value)) => *value,
            other => panic!("{key} is {other:?}"),
        };
        let float = |data: &Compound, key: &str| match data.get(&key.into()) {
            Some(Tag::Float(bits)) => f32::from_bits(*bits),
            other => panic!("{key} is {other:?}"),
        };
        let slots = |data: &Compound| match data.get(&"Items".into()) {
            Some(Tag::List(items)) => items
                .iter()
                .map(|item| match item.get("Slot") {
                    Some(Tag::Byte(slot)) => *slot,
                    other => panic!("slot is {other:?}"),
                })
                .collect::<Vec<_>>(),
            other => panic!("Items is {other:?}"),
        };

        let furnace = BlockEntity::Furnace(Furnace {
            kind: FurnaceKind::BlastFurnace,
            items: [Some(ItemStack::new("minecraft:raw_iron", 3)), None, None],
            lit_remaining: 750,
            lit_total: 800,
            cooking_timer: 40,
            cooking_total: 100,
            speed: 2.0,
            recipes_used: Default::default(),
        });
        let data = save_without_metadata(
            &world,
            (0, 64, 0),
            &furnace,
            "minecraft:blast_furnace",
            &Default::default(),
        )
        .unwrap();
        assert_eq!(int(&data, "cooking_time_spent"), 40);
        assert_eq!(int(&data, "cooking_total_time"), 100);
        assert_eq!(int(&data, "lit_time_remaining"), 750);
        assert_eq!(int(&data, "lit_total_time"), 800);
        assert_eq!(float(&data, "speed_multiplier"), 2.0);
        assert_eq!(slots(&data), vec![0], "the input slot is held");

        let mut stand = BrewingStand::new();
        stand.brew_time = 150;
        stand.total_brew_time = 400;
        stand.fuel = 12;
        stand.items[0] = Some(ItemStack::new("minecraft:potion", 1));
        stand.items[2] = Some(ItemStack::new("minecraft:potion", 1));
        stand.items[3] = Some(ItemStack::new("minecraft:nether_wart", 1));
        let stand = BlockEntity::Brewing(Box::new(stand));
        let data = save_without_metadata(
            &world,
            (1, 64, 0),
            &stand,
            "minecraft:brewing_stand",
            &Default::default(),
        )
        .unwrap();
        assert_eq!(int(&data, "BrewTime"), 150);
        assert_eq!(int(&data, "total_brew_time"), 400);
        assert_eq!(int(&data, "Fuel"), 12);
        assert_eq!(float(&data, "speed_multiplier"), 1.0);
        assert_eq!(
            slots(&data),
            vec![0, 2, 3],
            "two bottles and the ingredient"
        );
    }

    #[test]
    #[ignore = "daily"]
    fn retained_handles_reject_replacements_and_rollback_restores_identity() {
        use super::super::native_entity_refs::tests::{run_fresh_cases, with_game};
        if run_fresh_cases(
            "game::server_side::native_block_entity_refs::tests::retained_handles_reject_replacements_and_rollback_restores_identity",
            "SUNRISE_RETAINED_HANDLES_CHILD",
            &["holder"],
        ) {
            return;
        }
        with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let level = game.level("minecraft:overworld").unwrap();
            let pos = (1, 64, 1);
            let holder = || {
                BlockEntity::Mod(Box::new(ModBlockEntity::new(
                    "test:holder".into(),
                    ModBlockEntityFields {
                        nbt: compound("{count:3}"),
                        ..Default::default()
                    },
                )))
            };
            game.world.block_entities.insert(pos, holder());
            let original = game.block_entity(level, [1, 64, 1]).unwrap();
            game.with_level(|_, host, ticks| {
                let wrapped = super::super::mod_api_host::LevelTicks::new(&*host, ticks);
                assert_eq!(
                    mod_api::LevelHost::block_entity(&wrapped, [1, 64, 1]),
                    mod_api::LevelHost::block_entity(&*host, [1, 64, 1]),
                );
                assert_eq!(
                    mod_api::LevelHost::block_entity(&wrapped, [1, 64, 1]),
                    Some(original),
                );
                assert_eq!(
                    mod_api::LevelHost::block_entity(&wrapped, [1, 400, 1]),
                    None,
                );
                // The ticks wrapper hands the same reads through, saved data included.
                let data = mod_api::LevelHost::block_entity_data(&*host, original);
                assert_eq!(
                    data.as_ref().and_then(|data| data.get(&"count".into())),
                    Some(&mod_api::Tag::Int(3))
                );
                assert_eq!(
                    mod_api::LevelHost::block_entity_data(&wrapped, original),
                    data,
                );
            });
            assert_eq!(original.set_data_slots(game, vec![7]), Ok(true));
            let mut tentative = None;
            let _: Result<(), ()> = game.storage_transaction(|game, _| {
                game.journal_block_entity(pos);
                game.world
                    .block_entities
                    .insert(pos, BlockEntity::Chest(vec![]));
                assert_eq!(original.data_slots(game), None);
                tentative = game.block_entity(level, [1, 64, 1]);
                Err(())
            });
            assert_eq!(original.data_slots(game), Some(vec![7]));
            assert_eq!(tentative.unwrap().data(game), None);
            let (lease, taken) = game.world.block_entities.take(&pos).unwrap();
            assert_eq!(original.data_slots(game), None);
            assert!(game.world.block_entities.put(lease, taken));
            assert_eq!(original.data_slots(game), Some(vec![7]));
            game.world.block_entities.remove(&pos);
            game.world.block_entities.insert(pos, holder());
            let replacement = game.block_entity(level, [1, 64, 1]).unwrap();
            assert_ne!(original, replacement);
            let stale = Err(BlockEntityError::Stale);
            assert_eq!(original.set_lock(game, None), stale);
            assert_eq!(original.set_name(game, Some("stale")), stale);
            assert_eq!(original.merge_data(game, Compound::new()), stale);
            assert_eq!(original.replace_data(game, Compound::new()), stale);
            assert_eq!(original.set_changed(game), stale);
            assert_eq!(original.set_data_slot(game, 0, 99), stale);
            assert_eq!(
                original.set_data_slots(game, vec![99]),
                Err(BlockEntityDataError::Stale)
            );
            assert_eq!(
                original.item_slot(game, 0),
                Err(BlockEntityItemError::Stale)
            );
            assert_eq!(replacement.data_slots(game), Some(vec![]));
            assert_eq!(replacement.set_data_slots(game, vec![8]), Ok(true));
            game.world.block_entities.load_chunk(vec![(pos, holder())]);
            assert_eq!(
                replacement.set_data_slots(game, vec![99]),
                Err(BlockEntityDataError::Stale)
            );
        });
    }

    #[test]
    fn lookup_requires_loaded_chunk_and_tracks_removal_and_replacement() {
        let mut world = World::new(BlockStateId(0), 0);
        let pos = [-1, 64, 0];
        let key = (-1, 64, 0);
        world.block_entities.insert(key, BlockEntity::Chest(vec![]));
        assert!(loaded_block_entity(&world, pos).is_none());
        world.chunks.insert((-1, 0), Arc::new(Chunk::empty(-1, 0)));
        assert!(matches!(
            loaded_block_entity(&world, pos),
            Some(BlockEntity::Chest(_))
        ));
        world.block_entities.remove(&key);
        assert!(loaded_block_entity(&world, pos).is_none());
        world
            .block_entities
            .insert(key, BlockEntity::DaylightDetector);
        assert!(matches!(
            loaded_block_entity(&world, pos),
            Some(BlockEntity::DaylightDetector)
        ));
        let (lease, detached) = world.block_entities.take(&key).unwrap();
        assert!(loaded_block_entity(&world, pos).is_none());
        assert!(world.block_entities.put(lease, detached));
        assert!(matches!(
            loaded_block_entity(&world, pos),
            Some(BlockEntity::DaylightDetector)
        ));
        let outside_pos = [-1, world.shape.max_y(), 0];
        world
            .block_entities
            .insert((-1, outside_pos[1], 0), BlockEntity::DaylightDetector);
        assert!(loaded_block_entity(&world, outside_pos).is_none());
        let below_pos = [-1, world.shape.min_y - 1, 0];
        world
            .block_entities
            .insert((-1, below_pos[1], 0), BlockEntity::DaylightDetector);
        assert!(loaded_block_entity(&world, below_pos).is_none());
    }

    #[test]
    #[ignore = "daily"]
    fn an_abort_puts_back_the_lock_and_the_loot_table_roll() {
        use super::super::native_entity_refs::tests::with_game;
        use mod_api::BlockUpdateFlags;
        with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let level = game.level("minecraft:overworld").unwrap();
            let chest = game.pack.assets.registry.default_state("minecraft:chest");
            let flags = BlockUpdateFlags::NEIGHBORS;
            assert_eq!(
                game.level_set_block(level, [0, 64, 0], chest.unwrap(), flags),
                Ok(true)
            );
            let chest = game.block_entity(level, [0, 64, 0]).unwrap();
            let key = (0, 64, 0);
            let lock = compound("{lock:{items:\"minecraft:stick\"}}");
            let table = compound("{LootTable:\"minecraft:chests/simple_dungeon\"}");
            let before = game.world.block_entity_extras(key);

            let host: &mut dyn ServerHost = game;
            let aborted: Result<(), TransactionError<()>> = host.transaction(|host, _| {
                assert!(
                    chest
                        .set_lock(host, Some(compound("{items:\"minecraft:stick\"}")))
                        .unwrap()
                );
                assert!(chest.merge_data(host, table.clone()).unwrap());
                Err(())
            });
            assert_eq!(aborted, Err(TransactionError::Aborted(())));
            assert_eq!(game.world.block_entity_extras(key), before);
            assert!(game.world.block_lock(key).is_none());

            let host: &mut dyn ServerHost = game;
            let committed: Result<(), TransactionError<()>> = host.transaction(|host, _| {
                assert!(chest.merge_data(host, lock.clone()).unwrap());
                assert!(chest.merge_data(host, table.clone()).unwrap());
                Ok(())
            });
            assert_eq!(committed, Ok(()));
            assert!(game.world.block_lock(key).is_some());
            assert_ne!(game.world.block_entity_extras(key), before);
        });
    }
}

use sunrise_gameplay_common::game::tables::GameItems as _;

use sunrise_gameplay_common::game::transactions::TransactionsGameExt as _;

use sunrise_gameplay_common::game::GameGameExt1 as _;

use sunrise_gameplay_common::game::commands_common::CommandsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_block_entity_read::NativeBlockEntityReadGameExt1 as _;

#[cfg(test)]
use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_level_refs_common::NativeLevelRefsCommonGameExt1 as _;
