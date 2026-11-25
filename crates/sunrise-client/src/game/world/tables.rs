//! Read-only world content tables shared by server and client games.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use std::sync::LazyLock;
#[allow(unused_imports)]
use sunrise_client::{
    blocks::{MAP_COLORS, RenderShape},
    menu::Env,
    neoforge_mob_maps::NeoForgeMobMaps,
    particles::{ParticleOptions, ParticleType},
};
#[allow(unused_imports)]
use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
use sunrise_world::level::data::BlockSet;

/// [`Game::level_suffocation`] from the fields it reads, for a caller that holds other fields: the
/// world's own tables (its datapacks' `causes_suffocation`), else the built-in ones.
#[allow(dead_code)]
pub(in crate::game) fn level_suffocation_of<'a>(
    world_tables: &'a Option<WorldTables>,
    res: &'a Resources,
) -> &'a sunrise_world::suffocation::SuffocationRegistry<'static> {
    world_tables
        .as_ref()
        .map_or_else(|| res.suffocation(), |t| &t.suffocation)
}

impl Game<'_> {
    /// Server reads active station enchantments; client reads synced remote enchantments or active items.
    pub(in crate::game) fn item_enchantments(&self) -> &Arc<sunrise_client::enchant::Enchantments> {
        if self.is_logical_server() {
            &self.stations().enchantments
        } else if let Some(ench) = self.remote.as_ref().and_then(|r| r.enchantments.as_ref()) {
            ench
        } else {
            &self.items().enchantments
        }
    }

    /// The content tables the world plays with: its data packs' over the
    /// game's own when it has packs, `Resources.rest`'s own otherwise.
    pub(in crate::game) fn items(&self) -> &Items {
        self.server_items
            .as_deref()
            .or_else(|| self.side_world_tables().map(|t| t.items.as_ref()))
            .unwrap_or_else(|| self.res.items())
    }

    /// The level's suffocation rules, for its mobs and its players.
    pub(in crate::game) fn level_suffocation(
        &self,
    ) -> &sunrise_world::suffocation::SuffocationRegistry<'static> {
        self.side_world_tables()
            .map_or_else(|| self.res.suffocation(), |t| &t.suffocation)
    }

    pub(in crate::game) fn recipes(&self) -> &Recipes {
        self.side_recipes()
    }

    pub(in crate::game) fn active_providers(&self) -> &Providers {
        self.side_world_tables()
            .map_or_else(|| self.res.providers(), |t| &t.providers)
    }

    pub(in crate::game) fn stations(&self) -> &sunrise_client::stations::StationData {
        self.side_world_tables()
            .map_or_else(|| self.res.stations(), |t| &t.stations)
    }
}

pub(in crate::game) static DEFAULT_CLIENT_FEATURES: LazyLock<Arc<[String]>> =
    LazyLock::new(|| Arc::from([String::from("minecraft:vanilla")]));

pub(in crate::game) fn default_client_features() -> &'static Arc<[String]> {
    &DEFAULT_CLIENT_FEATURES
}

/// Kinds of contextual block provider dispatched by the game.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::game) enum BlockProviderQuery {
    EnchantPower,
    LightEmission,
}

#[derive(PartialEq, Eq)]
pub(in crate::game) struct BlockQueryKey {
    pub(in crate::game) game: *const (),
    pub(in crate::game) state: BlockStateId,
    pub(in crate::game) query: BlockProviderQuery,
}

thread_local! {
    pub(in crate::game) static ACTIVE_BLOCK_QUERIES: RefCell<Vec<BlockQueryKey>> = const { RefCell::new(Vec::new()) };
}

pub(in crate::game) struct PopBlockQuery;

impl Drop for PopBlockQuery {
    fn drop(&mut self) {
        ACTIVE_BLOCK_QUERIES.with(|active| {
            active.borrow_mut().pop();
        });
    }
}

impl Game<'_> {
    pub(in crate::game) fn pack_files(&self) -> sunrise_world::loot::PackOverlay<'_> {
        match self.side_world_tables() {
            Some(t) => sunrise_world::loot::PackOverlay {
                conditions: &t.conditions,
                files: &t.files,
                priority_layers: &t.priority_layers,
            },
            None => sunrise_world::loot::PackOverlay::NONE,
        }
    }

    pub(in crate::game) fn active_enchantment_definitions(
        &self,
    ) -> &sunrise_client::enchant::Enchantments {
        self.item_enchantments()
    }

    pub(in crate::game) fn shared_items(&self) -> Arc<Items> {
        if let Some(items) = &self.server_items {
            Arc::clone(items)
        } else if let Some(tables) = self.side_world_tables() {
            Arc::clone(&tables.items)
        } else {
            self.res.shared_items()
        }
    }

    pub(in crate::game) fn station_recipes(&self) -> &sunrise_client::stations::StationRecipes {
        if self.is_logical_server() {
            &self.stations().recipes
        } else {
            self.recipe_tables
                .server_stations
                .as_ref()
                .unwrap_or(&self.stations().recipes)
        }
    }

    pub(in crate::game) fn mob_maps(&self) -> &NeoForgeMobMaps {
        self.mob_map_snapshot().as_ref()
    }

    pub(in crate::game) fn mob_map_snapshot(&self) -> &Arc<NeoForgeMobMaps> {
        if let Some(tables) = self.side_world_tables() {
            &tables.data_maps.mob_maps
        } else {
            NeoForgeMobMaps::vanilla()
        }
    }

    pub(in crate::game) fn enabled_feature_ids(&self) -> &Arc<[String]> {
        if self.is_logical_server() {
            self.level.enabled_features.as_arc()
        } else {
            match &self.remote {
                Some(remote) => &remote.features,
                None => default_client_features(),
            }
        }
    }

    pub fn limited_crafting(&self) -> bool {
        if self.is_logical_server() {
            self.ui
                .rules
                .bool_at(sunrise_client::gamerules::vanilla::LIMITED_CRAFTING)
        } else {
            self.remote.as_ref().is_some_and(|r| r.do_limited_crafting)
        }
    }

    pub(in crate::game) fn menu_env(&self) -> Env<'_> {
        Env {
            crafting_remainder: Some(self),
            player: self.side_acting_player_uuid(),
            player_entity_id: Some(self.own_entity_id()),
            loop_context: &self.loop_context,
            items: self.items(),
            recipes: self.recipes(),
            brewing: &self.stations().brewing,
            banners: &self.stations().banners,
            station_recipes: self.station_recipes(),
            enchantments: self.item_enchantments(),
            names: &self.pack.names,
            trade: None,
            maps: None,
            limited_crafting: self.limited_crafting(),
            can_equip: None,
            known_recipes: Some(&self.level.recipe_book),
            slot_rules: Some(self),
            stacked: None,
        }
    }

    pub(in crate::game) fn loaded_data_maps(
        &self,
    ) -> std::result::Result<&native_data_maps::DataMaps, mod_api::DataMapError> {
        self.side_world_tables()
            .map(|tables| &tables.data_maps)
            .ok_or(mod_api::DataMapError::Unavailable)
    }

    pub(in crate::game) fn data_map_value(
        &self,
        registry: &str,
        entry: &str,
        map: &str,
    ) -> std::result::Result<Option<&Tag>, mod_api::DataMapError> {
        self.loaded_data_maps()?
            .value(registry, map, &native_data_maps::full_id(entry))
    }

    pub(in crate::game) fn data_map_entries(
        &self,
        registry: &str,
        map: &str,
    ) -> std::result::Result<&[(String, Tag)], mod_api::DataMapError> {
        self.loaded_data_maps()?.entries(registry, map)
    }

    pub(in crate::game) fn biome_key(&self, id: &str) -> Option<mod_api::BiomeKey> {
        self.loaded_data_maps().ok()?.biome_keys.key(id)
    }

    pub(in crate::game) fn biome_key_id(
        &self,
        key: mod_api::BiomeKey,
    ) -> std::result::Result<&str, mod_api::BiomeKeyError> {
        let maps = self
            .loaded_data_maps()
            .map_err(|_| mod_api::BiomeKeyError::Unavailable)?;
        maps.biome_keys.id(key)
    }

    pub(in crate::game) fn mod_recipe_property_contains(
        &self,
        set: &str,
        item: mod_api::ItemKey,
    ) -> bool {
        mod_api::__private::item_key_id(item)
            .is_some_and(|name| self.loop_context.mod_recipe_property_contains(set, name))
    }

    pub(in crate::game) fn refresh_recipe_property_context(&mut self) {
        let mut sets = HashMap::<String, HashSet<String>>::new();
        if !self.is_logical_server() {
            if let Some(server) = &self.recipe_tables.server_properties.sets
                && let Some(remote) = &self.remote
            {
                for (key, ids) in server.iter().filter(|(k, _)| !k.starts_with("minecraft:")) {
                    let items = ids
                        .iter()
                        .filter_map(|id| remote.builtin.get("minecraft:item")?.name(*id))
                        .filter(|name| *name != "minecraft:air")
                        .map(str::to_owned)
                        .collect();
                    sets.insert(key.clone(), items);
                }
            }
        } else if self.side_world_tables().is_some() || self.res.rest.get().is_some() {
            for (key, names) in self.recipes().custom_property_sets() {
                let items = names
                    .iter()
                    .filter(|name| self.res.items().get(name).is_some())
                    .cloned()
                    .collect();
                sets.insert(key.to_owned(), items);
            }
        }
        let tags = self
            .native_tag_items()
            .filter(|items| items.tags_loaded(sunrise_items::items::TagRegistry::Item))
            .and_then(|items| {
                let registry = self
                    .items()
                    .registry_order
                    .iter()
                    .map(AsRef::as_ref)
                    .collect();
                mod_api::__private::BoundItemTags::bind(&items.item_tag_order, &registry)
            });
        let context = Arc::make_mut(&mut self.loop_context);
        context.set_recipe_properties(Arc::new(sets));
        context.set_item_tags(tags);
        if self.is_logical_server()
            && let Some(host) = self.lan.as_mut()
        {
            host.hub.loop_context = self.loop_context.clone();
        }
    }

    pub(in crate::game) fn native_tag_items(&self) -> Option<&Items> {
        if !self.is_logical_server() {
            return self.remote.as_ref()?.tags.as_ref();
        }
        self.side_tags_ready().then(|| self.items())
    }

    pub(in crate::game) fn block_tag_sets(
        &self,
        items: &Items,
    ) -> HashMap<String, HashSet<mod_api::BlockKey>> {
        native_world_read::prepare_block_tags(&items.block_tags)
    }

    pub(in crate::game) fn resolve_level_infiniburn(
        &self,
        facts: &sunrise_world::dimension_type::DimensionFacts,
    ) -> Arc<BlockSet> {
        let name = facts
            .infiniburn
            .strip_prefix('#')
            .unwrap_or(&facts.infiniburn);
        let mut set = BlockSet::default();
        if let Some(members) = self.items().block_tags.get(name) {
            for member in members {
                if let Some(block) = self.res.behaviours().table.block_by_name(member) {
                    set.insert(block);
                }
            }
        }
        Arc::new(set)
    }

    pub(in crate::game) fn prepare_cauldron_interactions(
        &self,
        items: &Items,
    ) -> Result<Arc<cauldron_interactions::Table>> {
        use mod_api::__private::game::RegistrationContextNew;
        let mut registrar = cauldron_interactions::Registrar::new(items, self.pack.assets.registry);
        crate::game::native_hooks::native_register_cauldron_interaction(
            &mod_api::RegistrationContext::new(),
            &mut registrar,
        );
        registrar.finish()
    }

    pub(in crate::game) fn prepare_parts(&self, items: &Items) -> Result<PreparedParts> {
        #[cfg(not(test))]
        if !mod_api::__private::content_owners_published() {
            return Err("Mod parts refused: content ownership has not been published".into());
        }
        let registry = self.pack.assets.registry;
        let block_tags = &items.block_tags;
        let tags = self.block_tag_sets(items);
        let parts = mod_api::__private::prepare_parts(
            block_tags,
            &items.item_tags,
            mod_api::__private::KnownIds {
                block: &|id| registry.default_state(id).is_some(),
                item: &|id| items.items.contains_key(id),
            },
        )
        .map_err(|refused| match refused {
            mod_api::__private::ResolvePartsError::ItemKeys(errors) => format!(
                "Mod item keys refused:\n{}",
                errors
                    .into_iter()
                    .map(|e| format!("mod {} requested unknown item {}", e.mod_id, e.id))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            mod_api::__private::ResolvePartsError::Parts(errors) => {
                format!("Mod parts refused:\n{}", errors.join("\n"))
            }
            _ => "Mod parts refused".to_owned(),
        })?;
        let block_states = api_values::prepare_block_states(registry)?;
        let cauldrons = self.prepare_cauldron_interactions(items)?;
        Ok(PreparedParts {
            cauldrons,
            block_states,
            tags,
            parts,
        })
    }

    pub(in crate::game) fn resolve_parts(&self) -> Result<()> {
        let prepared = self.prepare_parts(self.items())?;
        if !self.is_logical_server()
            && self
                .remote
                .as_ref()
                .is_some_and(|r| !r.registries.tags.is_empty())
        {
            self.bind_parts(prepared);
        } else {
            self.install_parts(prepared);
        }
        self.side_resolve_enchant_particles();
        Ok(())
    }

    pub(in crate::game) fn install_parts(&self, prepared: PreparedParts) {
        self.bind_parts(prepared);
        self.resolve_primary_items();
    }

    pub(in crate::game) fn bind_parts(&self, prepared: PreparedParts) {
        if self.is_logical_server()
            && let Some(host) = &self.lan
        {
            cauldron_prediction::publish_registrations(
                &prepared.cauldrons,
                host.hub
                    .guests
                    .simulation_iter()
                    .filter(|g| g.placed || g.held),
            );
        }
        cauldron_interactions::install(prepared.cauldrons);
        if self.is_logical_server() {
            native_worldgen_parts::publish(
                prepared.parts.as_ref(),
                &prepared.block_states,
                self.pack.assets.registry,
            );
        }
        api_values::install_block_states(&self.tick, prepared.block_states);
        native_world_read::install_block_tags(prepared.tags);
        api_values::install_resolved_parts(&self.tick, prepared.parts);
        self.mark_collision_shape_states();
        self.side_resolve_block_paths();
        self.resolve_piston_reactions();
    }

    pub(in crate::game) fn resolve_primary_items(&self) {
        let mut support = sunrise_world::enchantment_answers::Answers::new();
        let enchantments = self.item_enchantments();
        let items = self.items();
        if mod_api::__private::has_part_hook(mod_api::HookId::SupportsEnchantment) {
            for item in items.items.keys() {
                if !mod_api::__private::item_listens(mod_api::HookId::SupportsEnchantment, item) {
                    continue;
                }
                let stack = ItemStack::new(item, 1);
                for enchantment in &enchantments.list {
                    let mut answer = None;
                    self.native_supports_enchantment(&stack, &enchantment.id, &mut answer);
                    if let Some(supported) = answer {
                        support
                            .entry(item.clone())
                            .or_default()
                            .insert(enchantment.id.clone(), supported);
                    }
                }
            }
        }
        for table in self.enchantment_snapshots() {
            table.support_answers().install(support.clone());
        }
        self.side_resolve_primary_items_server(support);
    }

    pub(in crate::game) fn clear_world_parts(&self) {
        for table in self.enchantment_snapshots() {
            table.support_answers().clear();
            if self.is_logical_server() {
                table.primary_answers().clear();
            }
        }
        self.side_clear_loot_enchantment_answers();
        mod_api::__private::clear_game_parts();
    }

    pub(in crate::game) fn enchantment_snapshots(
        &self,
    ) -> impl Iterator<Item = &sunrise_items::enchant::Enchantments> {
        let stations = &self.stations().enchantments;
        let items = &self.items().enchantments;
        let active = self.item_enchantments();
        let other = (!Arc::ptr_eq(stations, items)).then_some(items);
        let synced =
            (!Arc::ptr_eq(active, stations) && !Arc::ptr_eq(active, items)).then_some(active);
        std::iter::once(stations)
            .chain(other)
            .chain(synced)
            .map(|table| table.as_ref())
    }

    pub(in crate::game) fn with_block_provider_query<R>(
        &self,
        state: BlockStateId,
        query: BlockProviderQuery,
        ask: impl FnOnce() -> R,
    ) -> Option<R> {
        let key = BlockQueryKey {
            game: std::ptr::from_ref(self).cast(),
            state,
            query,
        };
        ACTIVE_BLOCK_QUERIES.with(|active| {
            let mut active = active.borrow_mut();
            if active.contains(&key) {
                return None;
            }
            active.push(key);
            drop(active);
            let _pop = PopBlockQuery;
            Some(ask())
        })
    }

    pub(in crate::game) fn block_name(&self, p: [i32; 3]) -> Arc<str> {
        self.res
            .facts()
            .get(self.world.block(p[0], p[1], p[2]))
            .name
            .clone()
    }

    pub(in crate::game) fn dispenser_block_name(&self, p: (i32, i32, i32)) -> Arc<str> {
        self.res
            .facts()
            .get(self.world.block(p.0, p.1, p.2))
            .name
            .clone()
    }

    pub(in crate::game) fn in_block_tag(&self, tag: &str, block: &str) -> bool {
        self.items()
            .block_tags
            .get(tag)
            .is_some_and(|s| s.contains(block))
    }

    pub(in crate::game) fn named_block_in_tag(&self, tag: &str, block: &str) -> bool {
        self.items()
            .block_tags
            .get(tag)
            .is_some_and(|s| s.contains(block))
    }

    pub(in crate::game) fn item_in_tag(&self, tag: &str, item: &str) -> bool {
        self.items()
            .item_tags
            .get(tag)
            .is_some_and(|t| t.contains(item))
    }

    pub(in crate::game) fn biome_name_at(&self, p: sunrise_core::geometry::Vec3) -> Option<String> {
        let (x, y, z) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        let c = self.world.chunks.get(&(x >> 4, z >> 4))?;
        Some(world_biomes!(self).name(c.biome(x, y, z)).to_owned())
    }

    pub(in crate::game) fn enchantments_compatible(&self, a: &str, b: &str) -> bool {
        self.item_enchantments().compatible(a, b)
    }

    pub(in crate::game) fn item_has_enchantment_effect(
        &self,
        stack: &ItemStack,
        effect: &str,
    ) -> bool {
        self.item_enchantments().has_component(stack, effect)
    }

    pub(in crate::game) fn block_particle_sprite(
        pack: &PackResources,
        state: BlockStateId,
    ) -> Option<String> {
        let catalog = &pack.render.model_catalog;
        let layer = catalog.particle(state).or_else(|| {
            let models = catalog
                .collect(state, sunrise_core::position::BlockPos::new(0, 0, 0))
                .ok()?;
            models
                .iter()
                .flat_map(|(_, m)| m.faces.iter())
                .next()
                .map(|f| f.layer)
        })?;
        pack.layer_names.get(&layer).cloned()
    }

    pub(in crate::game) fn particle_sprite(
        &self,
        state: BlockStateId,
        pos: [i32; 3],
    ) -> Option<(String, u32)> {
        let sprite = Self::block_particle_sprite(&self.pack, state)?;
        let tint = self.terrain_particle_tint(state, pos).unwrap_or(0xFFFFFF);
        Some((sprite, tint))
    }

    pub(in crate::game) fn terrain_particle_tint(
        &self,
        state: BlockStateId,
        pos: [i32; 3],
    ) -> Option<u32> {
        let chunks = &self.world.chunks;
        let (cx, cz) = (pos[0] >> 4, pos[2] >> 4);
        let region: [[Option<&sunrise_client::chunk::Chunk>; 3]; 3] = std::array::from_fn(|dz| {
            std::array::from_fn(|dx| {
                chunks
                    .get(&(cx + dx as i32 - 1, cz + dz as i32 - 1))
                    .map(|c| &**c)
            })
        });
        self.pack.context.tables.terrain_particle_tint(
            state,
            pos,
            &region,
            world_biomes!(self),
            self.streaming.biome_zoom,
            self.streaming.biome_blend,
            self.streaming.nether_shading,
        )
    }
}
