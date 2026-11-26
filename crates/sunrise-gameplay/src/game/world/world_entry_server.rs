use super::*;
use sunrise_gameplay_common::game::api_values;
use {
    crate::game::server_side::global_loot_modifiers,
    crate::game::server_side::native_registry_entries_server,
    sunrise_gameplay_common::game::native_data_maps_common::DataMaps,
    sunrise_gameplay_common::game::native_data_maps_common::WorldRegistries,
    sunrise_gameplay_common::game::world_entry::next_biome_keys_session,
};

impl crate::game::ServerGame<'_> {
    /// Reload the content tables with the world's packs, as
    /// `/reload` reloads a server's (`ReloadableServerResources`).
    /// Publish tables before function admission, leaving advancement rewards queued
    /// until the reload's new function store is available.
    pub(in crate::game) fn install_world_tables(&mut self, prepared: PreparedWorldReload) {
        let PreparedWorldReload {
            structure_spawns,
            tables,
            advancements,
            health,
            parts,
            wire,
            wire_tags,
            datapack_registries,
        } = prepared;
        self.close_stonecutters_for_reload();
        lan::sync::forget_server_sources();
        if let Some(tables) = &tables
            && let Some(sources) = &tables.reload_sources
        {
            let overlay = sunrise_world::loot::PackOverlay {
                conditions: &tables.conditions,
                files: &tables.files,
                priority_layers: &tables.priority_layers,
            };
            lan::sync::install_server_sources(
                &self.common.options.resources,
                &self.common.res.core.item_data,
                &overlay,
                sources.clone(),
            );
        }
        self.common.mods.datapack_registries = datapack_registries.map(Arc::new);
        self.common.health_resources = health;
        self.server.world_tables.0 = tables;
        // Publish the paired item and world evaluators before any callbacks.
        self.load_server_loot();
        self.server.data_map_registries = self
            .server
            .world_tables
            .0
            .as_ref()
            .map_or_else(Vec::new, |t| t.data_maps.registries());
        self.install_block_maps();
        if let Err(error) = self.install_fluid_interactions() {
            log::error!("Sunrise: {error}");
        }
        self.install_profession_save_snapshots();
        self.share_mob_maps();
        self.share_biome_keys();
        self.common.world.pois.clear();
        for world in self.server.levels.0.parked_worlds() {
            world.pois.clear();
        }
        sunrise_sim::datapack_net::serve(self.common.mods.datapack_registries.clone());
        self.send_datapack_snapshots();
        self.refresh_recipe_property_context();
        self.server.structure_spawns = structure_spawns;
        self.refresh_infiniburn();
        self.install_parts(parts);
        self.common.damage_tag_generation = self.common.damage_tag_generation.wrapping_add(1);
        // Before the players are sent the reloaded data below.
        self.sync_recipe_content();
        // PlayerList.reloadResources: the tags and recipes go to the players
        // and their advancements reload. The synchronized registries (the
        // dialogs, and the commands' ids of them) stay the hub's.
        lan::sync::install_wire_reload(
            &self.common.options.resources,
            &self.common.res.core.item_data,
            &self.pack_files(),
            wire.clone(),
            advancements.clone(),
            wire_tags,
        );
        if self.server.lan.is_some()
            && let Some(sync) = wire
        {
            self.with_advancement_hub(|hub, owners| hub.reload_resources(sync, owners));
        }
        self.common.adv.tree = advancements;
        self.common.adv.reset_tree();
        self.common
            .announce_data_bound(mod_api::TagsUpdateCause::ServerDataLoad { reload: true });
        self.announce_data_maps_updated(mod_api::DataMapsUpdateCause::ServerReload);
    }
    /// Build and publish one server snapshot for this world load or reload.
    pub(in crate::game) fn load_server_loot(&mut self) {
        if let Some(tables) = &self.server.world_tables.0 {
            self.common.server_items = Some(tables.items.clone());
            self.copy_loot_enchantment_answers(&tables.loot);
            self.server.server_loot.0.set(Some(tables.loot.clone()));
            global_loot_modifiers::bind_retained(
                &self.server.reload_resources.0,
                &tables.loot_modifier_manager,
            );
            return;
        }
        self.common.server_items = None;
        let overlay = pack_files(&self.server.world_tables.0);
        let loot = match sunrise_world::loot::LootData::load_with_bound_tags(
            &self.common.options.resources,
            &overlay,
            self.common.res.item_defaults(),
            &self.common.health_resources.damage,
            &self.items().ordered_tags(),
        ) {
            Ok(mut loot) => {
                if !mod_api::__private::mods().is_empty() {
                    let context = match lan::sync::server_data_load(
                        &self.common.options.resources,
                        &self.common.res.core.item_data,
                    ) {
                        Ok(context) => context,
                        Err(error) => {
                            log::warn!("Sunrise: server loot inventory unavailable: {error}");
                            self.server.server_loot.0.set(None);
                            return;
                        }
                    };
                    if let Err(error) = loot.apply_native_table_loads(&context) {
                        log::warn!("Sunrise: native loot tables refused: {error}");
                        self.server.server_loot.0.set(None);
                        return;
                    }
                }
                Some(Arc::new(loot))
            }
            Err(e) => {
                log::warn!("Sunrise: server loot tables unavailable: {e}");
                None
            }
        };
        if !mod_api::__private::mods().is_empty()
            && let Some(loot) = &loot
        {
            let mut items = self.items().clone();
            items.set_loot_tables(loot.tables().clone());
            self.common.server_items = Some(Arc::new(items));
        }
        if let Some(loot) = &loot {
            self.copy_loot_enchantment_answers(loot);
        }
        self.server.server_loot.0.set(loot);
    }
    /// A world open's tables, and the first generation of the registries the mods declared.
    pub(in crate::game) fn open_world_tables(
        &mut self,
        staged: super::condition_reload_state::StagedWorldTables,
    ) -> Result<()> {
        self.server.world_tables.0 = staged.tables;
        self.common.health_resources = staged.health;
        self.common.mods.datapack_registries = staged.datapack_registries.map(Arc::new);
        self.common.adv.tree = staged.advancements;
        self.common.adv.reset_tree();
        self.load_server_loot();
        self.server.data_map_registries = self
            .server
            .world_tables
            .0
            .as_ref()
            .map_or_else(Vec::new, |t| t.data_maps.registries());
        self.install_block_maps();
        self.install_fluid_interactions()?;
        self.install_profession_save_snapshots();
        self.share_mob_maps();
        self.share_biome_keys();
        sunrise_sim::datapack_net::serve(self.common.mods.datapack_registries.clone());
        Ok(())
    }
    /// Reload the content tables with the world's packs, as
    /// `/reload` reloads a server's (`ReloadableServerResources`).
    pub(in crate::game) fn prepare_world_reload(
        &mut self,
        stack: &sunrise_gameplay_common::game::function_pack_stack::PackStack,
        root: &std::path::Path,
    ) -> Result<PreparedWorldReload> {
        let super::condition_reload_state::StagedWorldTables {
            tables,
            advancements,
            health,
            datapack_registries,
            ..
        } = self.stage_world_tables(stack, true, root)?;
        let items = tables.as_ref().map_or_else(
            || self.common.res.shared_items(),
            |tables| tables.items.clone(),
        );
        let parts = self.prepare_parts(&items)?;
        let wire_tags = sunrise_sim::lan::data::WorldTags::new(items.clone(), &health.damage);
        let overlay = pack_files(&tables);
        let kept = self
            .server
            .lan
            .as_ref()
            .map(|host| host.hub.lan.shared.sync())
            .or_else(lan::sync::cached_reload_sync);
        let wire = if let Some(kept) = kept {
            let sources = match tables
                .as_ref()
                .and_then(|tables| tables.reload_sources.clone())
            {
                Some(sources) => sources,
                None => {
                    let sources = lan::sync::prepare_server_sources(
                        root,
                        &self.common.res.core.item_data,
                        &items,
                        &overlay,
                    )?;
                    sources.check()?;
                    sources
                }
            };
            Some(
                lan::sync::prepare_wire_reload(
                    root,
                    &self.common.res.core.item_data,
                    &overlay,
                    &kept,
                    &sources,
                    &advancements,
                    &wire_tags,
                )
                .map_err(|error| format!("wire reload: {error}"))?,
            )
        } else {
            None
        };
        overlay.conditions.check_errors()?;
        Ok(PreparedWorldReload {
            advancements,
            structure_spawns: sunrise_sim::mobs::spawn_selection::StructureSpawns::load(root),
            tables,
            health,
            parts,
            wire,
            wire_tags,
            datapack_registries,
        })
    }
    #[cfg(test)]
    #[inline(always)]
    pub(in crate::game) fn resolve_block_paths(&self) {
        self.server_resolve_block_paths();
    }
    #[inline(always)]
    pub(in crate::game) fn server_acting_player_uuid(&self) -> Option<u128> {
        self.server.lan_acting_uuid
    }
    #[inline(always)]
    pub(in crate::game) fn server_clear_random_tick_sections(&mut self) {
        self.server.random_tick_sections.0.clear();
    }
    pub(in crate::game) fn server_resolve_block_paths(&self) {
        use mod_api::{HookId, PathComputationType};
        use sunrise_sim::mobs::data::PartAnswers;
        let data = self.common.res.mob_data();
        let pathfindable =
            Self::resolve_per_state(HookId::IsPathfindable, data.states.len(), |state| {
                PathComputationType::ALL
                    .into_iter()
                    .filter(|&kind| {
                        let mut pathfindable = data.block_pathfindable(state, kind);
                        self.native_is_pathfindable(state, kind, &mut pathfindable);
                        pathfindable
                    })
                    .fold(0, |bits, kind| bits | PartAnswers::pathfindable_bit(kind))
            });
        data.resolve_parts(
            self.common.res.facts(),
            &PartAnswers {
                pathfindable: pathfindable.as_deref(),
            },
        );
    }
    #[inline(always)]
    pub(in crate::game) fn server_tags_ready(&self) -> bool {
        self.server.session_lock.0.is_some()
    }
    #[inline(always)]
    pub(in crate::game) fn server_world_tables(&self) -> Option<&WorldTables> {
        self.server.world_tables.0.as_ref()
    }
    /// Load the world's data packs' content tables (`None`: the world has no
    /// packs, and `Resources.rest` stays in use). This re-reads the tables,
    /// so it runs once per world open and on `/reload`.
    pub(in crate::game) fn stage_world_tables(
        &mut self,
        stack: &sunrise_gameplay_common::game::function_pack_stack::PackStack,
        reload: bool,
        root: &std::path::Path,
    ) -> Result<super::condition_reload_state::StagedWorldTables> {
        // The ask of this load, both here and on `/reload`: a native mod
        // recomputes the pack files it contributes every time.
        let mut contributed = Vec::new();
        self.native_data_pack_files(&mut contributed);
        let modifier_files = global_loot_modifiers::contributed(&contributed);
        let functions::OverlayFiles {
            files,
            read_errors,
            priority_layers,
        } = functions::overlay_files(
            stack,
            contributed
                .into_iter()
                .filter(|(key, _)| !key.starts_with("loot_modifiers/"))
                .collect(),
        );
        let inventory = sunrise_mod_package::enabled::EnabledMods::read(root)?;
        let conditions = self
            .common
            .registered_load_conditions(&inventory, stack, root, &files)?;
        let overlay = sunrise_world::loot::PackOverlay {
            conditions: &conditions,
            files: &files,
            ..sunrise_world::loot::PackOverlay::default()
        };
        let mut data_map_types = Vec::new();
        self.native_data_map_types(&mut data_map_types);
        let base = if reload {
            &self.common.health_resources
        } else {
            &self.common.res.motion().health
        };
        let health = base.with_packs(root, &overlay, reload)?;
        // With no pack, `res` holds the tables, unless content mods switched
        // live since it was loaded: their data is in the new root.
        let mut items = Items::load_registry(
            root,
            self.common.pack.assets.registry,
            self.common.res.facts(),
            &overlay,
            self.common.res.item_defaults(),
            if reload {
                sunrise_items::items::LoadEntityTags::Reload(&self.items().entity_type_tags)
            } else {
                sunrise_items::items::LoadEntityTags::Frozen(
                    self.common.res.items().entity_type_tags.ids(),
                )
            },
        )?;
        if reload {
            items.retain_unbound_tags(self.items());
        }
        class_tags::install(&mut items, self.common.res.facts());
        // Tag files are read before their tags are bound, so the tag readers below keep the
        // context without tags: there `neoforge:tag_empty` is unsupported, never answered from a
        // part of this load's tags.
        let tag_files = sunrise_world::loot::PackOverlay {
            conditions: &conditions,
            files: &files,
            ..sunrise_world::loot::PackOverlay::default()
        };
        // The other tables read from here on see this load's bound tags (`neoforge:tag_empty`).
        let conditions = conditions.with_tags(items.bound_tags());
        let overlay = sunrise_world::loot::PackOverlay {
            conditions: &conditions,
            files: &files,
            priority_layers: &priority_layers,
        };
        let packed_registry_definitions =
            if let Some(tables) = self.server.world_tables.0.as_ref().filter(|_| reload) {
                Ok(tables.packed_registries.clone())
            } else {
                // Admit the frozen pack definitions with this candidate's bound
                // tags, even if no peer joins and triggers the full wire load.
                sunrise_sim::lan::data::SyncData::stage_registry_definitions(
                    root,
                    self.common.res.item_defaults(),
                    &overlay,
                    &read_errors,
                    &self.common.ui.registries,
                    |id| {
                        self.common
                            .ui
                            .registries
                            .ids("minecraft:sound_event")
                            .binary_search_by(|sound| sound.as_str().cmp(id))
                            .is_ok()
                    },
                )
                .map(Arc::new)
            };
        // RegistryDataLoader.RELOADABLE_REGISTRIES: providers are staged and
        // published with the other reloadable data, including on /reload.
        let providers = Providers::load_with(root, &overlay);
        // Visit independently readable registries before consuming any result.
        // One rejected winner must not hide the other entries' diagnostics.
        let loot = sunrise_world::loot::LootData::load_with_bound_tags(
            root,
            &overlay,
            self.common.res.core.item_data.defaults(),
            &health.damage,
            &items.ordered_tags(),
        );
        let advancements = Arc::new(sunrise_sim::advancements::Tree::load_with(
            &root.join("data"),
            &overlay,
        ));
        let datapack_candidate = self.datapack_candidate(stack, &conditions);
        let install_items_result = match &packed_registry_definitions {
            Ok(packed) => {
                packed.install_items(&mut items, root, &overlay, self.common.res.item_defaults())
            }
            Err(_) => Ok(()),
        };
        // Item damage predicates use the same frozen registry as player hurt.
        Arc::make_mut(&mut items.enchantments).bind_damage_catalog(&health.damage);
        items.bind_component_damage_catalog(&health.damage);
        if let Ok(loot) = &loot {
            items.component_defaults = loot.component_defaults().clone();
        }
        // The recipe tables, the stations and the recipe book read one folder:
        // hold its files so it is read once.
        let _recipe_files = sunrise_world::json_folder::read(&root.join("data/minecraft/recipe"));
        let sources = if reload {
            lan::sync::prepare_server_sources(
                root,
                &self.common.res.core.item_data,
                &items,
                &overlay,
            )
        } else {
            lan::sync::load_sources(
                root,
                &self.common.res.core.item_data,
                &items,
                self.common.options.execution_role.data_load_origin(),
                &overlay,
            )
        };
        let packed_registry_definitions =
            match (packed_registry_definitions, conditions.check_errors()) {
                (Ok(packed), Ok(())) => packed,
                (packed, conditions) => {
                    // A rejected registry must not hide independent load errors.
                    let errors = [
                        packed.err(),
                        conditions.err(),
                        providers.as_ref().err().cloned(),
                        install_items_result.err().map(|error| error.to_string()),
                        loot.as_ref().err().map(ToString::to_string),
                        datapack_candidate.as_ref().err().map(ToString::to_string),
                        sources.as_ref().err().map(ToString::to_string),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("\n");
                    return Err(errors.into());
                }
            };
        let providers = providers?;
        install_items_result?;
        let sources = sources?;
        let mut loot = loot?;
        let recipes = Recipes::from_resolved(root, &items, &overlay, &sources)?;
        let mut stations = sunrise_sim::stations::StationData::from_sources(
            root,
            &items,
            &overlay,
            self.common.res.item_defaults(),
            (&sources, &recipes),
        );
        self.prepare_professions(root, &items, &mut stations)?;
        // The world's loaded datapack registries are a mod-declared registry's
        // authority for the data maps' id checks, as the command ids are for
        // the registries they list.
        let mut datapack_candidate = datapack_candidate?;
        if let Some(candidate) = &mut datapack_candidate {
            native_registry_entries_server::bind_datapack_tags(root, &tag_files, candidate)?;
        }
        let datapack_registries =
            datapack_candidate.map(mod_api::DatapackRegistrySet::from_candidate);
        let received = sunrise_sim::datapack_net::received();
        let datapack_set = if self.common.is_logical_server() {
            datapack_registries.as_ref()
        } else {
            received.as_deref()
        };
        let registries = WorldRegistries::new(
            self.common
                .ui
                .registries
                .by_registry
                .iter()
                .map(|(registry, entries)| (registry.as_str(), entries.ids.as_slice())),
            datapack_set,
            &items,
            root,
            &overlay,
        );
        let causes_suffocation = items
            .block_tags
            .get("minecraft:causes_suffocation")
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        let suffocation = sunrise_sim::mobs::suffocation::bind(
            self.common.res.core.collision.clone(),
            &causes_suffocation,
        )?;
        // A registry whose pack tags don't load keeps the game's own tags
        // (`in_tag` falls back to them), rather than matching no tag at all.
        let pack_tags: HashMap<_, _> = WorldTables::PACK_TAG_REGISTRIES
            .into_iter()
            .chain(sunrise_sim::neoforge_mob_maps::MAPS.map(|definition| definition.directory))
            .filter(|directory| !matches!(*directory, "item" | "entity_type" | "damage_type"))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter_map(|registry| {
                match sunrise_items::items::load_tags(root, registry, &tag_files) {
                    Ok(tags) => Some((registry, tags)),
                    Err(e) => {
                        log::warn!("Sunrise: the world's {registry} tags are not applied: {e}");
                        None
                    }
                }
            })
            .collect();
        // Items already decoded the root's command export. Block-state-only
        // command metadata is not the registry universe, and professions and
        // biomes belong to the live world candidate rather than that export.
        for registry in ["minecraft:sound_event", "minecraft:game_event"] {
            if items.component_registry.ids(registry).is_empty() {
                return Err(format!(
                    "builtin data maps require {registry} in command-registries.json"
                )
                .into());
            }
        }
        let builtin = neoforge_mob_maps::load(
            &overlay,
            |directory| match directory {
                "item" => Some(&items.item_tags),
                "entity_type" => Some(items.entity_type_tags.members()),
                _ => pack_tags.get(directory),
            },
            |registry, id| match registry {
                "minecraft:item" => items.items.contains_key(id),
                "minecraft:villager_profession" => stations.professions.resolve(id).is_some(),
                "minecraft:worldgen/biome" => self
                    .common
                    .world_biomes
                    .as_ref()
                    .unwrap_or_else(|| self.common.res.biomes())
                    .contains(id),
                "minecraft:entity_type" => {
                    self.common.res.mob_types().contains_key(id)
                        || items
                            .component_registry
                            .ids(registry)
                            .binary_search_by(|known| known.as_str().cmp(id))
                            .is_ok()
                }
                _ => items
                    .component_registry
                    .ids(registry)
                    .binary_search_by(|known| known.as_str().cmp(id))
                    .is_ok(),
            },
        )?;
        let data_maps = DataMaps::load(&data_map_types, &overlay, &registries)
            .with_builtins(builtin)
            .with_biome_keys(mod_api::__private::BiomeKeys::new(
                next_biome_keys_session(),
                self.common
                    .world_biomes
                    .as_ref()
                    .unwrap_or_else(|| self.common.res.biomes())
                    .registry_order(),
            ));
        drop(registries);
        // The API's key order is live state as well: do not change it for a
        // candidate refused by the condition readers.
        api_values::resolve_enchantment_registry(&items)?;
        let context = lan::sync::server_data_load(root, &self.common.res.core.item_data)?;
        if !mod_api::__private::mods().is_empty() {
            loot.apply_native_table_loads(&context)?;
        }
        let serializers = self
            .common
            .mods
            .loot_modifier_serializers
            .get_or_init(global_loot_modifiers::Serializers::capture);
        let (modifiers, loot_modifier_manager) = serializers.prepare(
            global_loot_modifiers::files(root, stack, modifier_files),
            &conditions,
            &context,
        )?;
        loot.set_global_modifiers(modifiers);
        items.set_loot_data(&loot);
        let loot = Arc::new(loot);
        Ok(super::condition_reload_state::StagedWorldTables {
            tables: Some(WorldTables {
                conditions,
                suffocation,
                files,
                priority_layers,
                items: Arc::new(items),
                packed_registries: packed_registry_definitions,
                providers,
                loot,
                loot_modifier_manager,
                recipes,
                stations,
                reload_sources: reload.then_some(sources),
                data_maps,
                pack_tags,
            }),
            advancements,
            health,
            datapack_registries,
        })
    }

    /// The entries of the mods' datapack registries in the winning pack files: decoded with
    /// each registry's schema and checked as one candidate, so a bad entry fails the load
    /// before any table is published. `None` when no mod declared a registry: nothing is
    /// scanned. A file whose `neoforge:conditions` fail is absent; the key is dropped before
    /// the schema sees the file.
    pub(in crate::game) fn datapack_candidate(
        &self,
        stack: &sunrise_gameplay_common::game::function_pack_stack::PackStack,
        conditions: &sunrise_world::conditions::LoadConditions,
    ) -> Result<Option<mod_api::DatapackCandidate>> {
        let declarations = mod_api::__private::datapack_registry_declarations();
        if declarations.is_empty() {
            return Ok(None);
        }
        let keys: Vec<(&str, &str)> = declarations
            .iter()
            .filter_map(|declaration| declaration.key.split_once(':'))
            .collect();
        // Content files come before the world's enabled packs, lowest priority first.
        let packs: Vec<_> = std::iter::once(&stack.content)
            .chain(stack.enabled_packs())
            .collect();
        let files: Vec<_> =
            sunrise_level::datapack::registry_files(&packs, &keys, |path, bytes| {
                // Malformed JSON is kept for the registry's own decode diagnostic.
                serde_json::from_slice::<serde_json::Value>(bytes)
                    .map_or(true, |json| conditions.allows_file(&json, path))
            })
            .map_err(|error| {
                format!(
                    "datapack registry {} entry {} (pack {}, file {}): {:?}",
                    error.registry, error.entry, error.pack, error.file, error.reason
                )
            })?
            .into_iter()
            .collect();
        mod_api::DatapackCandidate::build(declarations, &files)
            .map(Some)
            .map_err(|error| {
                format!(
                    "datapack registry {} entry {} (pack {}, file {}): {:?}",
                    error.registry, error.entry, error.pack, error.file, error.reason
                )
                .into()
            })
    }

    /// Sends each joined Sunrise client the snapshot of the set just published, built from the
    /// offer owned by its connection. A refused replacement ends that connection with its reason.
    pub(in crate::game) fn send_datapack_snapshots(&mut self) {
        let Some(host) = self.server.lan.as_ref() else {
            return;
        };
        for guest in host
            .hub
            .guests
            .iter()
            .filter(|g| g.placed && g.participates_online() && g.brand == "sunrise")
        {
            sunrise_sim::datapack_net::reload_connection(&guest.sender);
        }
    }

    /// Close guest stonecutter windows before publishing a reloaded recipe generation.
    pub(in crate::game) fn close_stonecutters_for_reload(&mut self) {
        let Some(host) = self.server.lan.as_ref() else {
            return;
        };
        let open: Vec<_> = host
            .menus
            .iter()
            .filter_map(|(&conn, menus)| {
                menus
                    .station
                    .as_ref()
                    .filter(|menu| menu.kind == super::stations_common::Kind::Stonecutter)
                    .map(|_| conn)
            })
            .collect();
        for conn in open {
            self.guest_close_menus_and_tell(conn);
        }
    }
}

use sunrise_gameplay_common::game::tables::GameItems as _;

use sunrise_gameplay_common::game::tables::WorldTablesExt as _;

use sunrise_gameplay_common::game::primary_items::PrimaryItemsGameExt1 as _;
