#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::world_entry::BIOME_KEYS_SESSION;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::world_entry::next_biome_keys_session;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::world_entry::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::atomic::{AtomicU64, Ordering};

impl Game<'_, crate::game::server_side::ServerState> {
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
                &self.options.resources,
                &self.res.core.item_data,
                &overlay,
                sources.clone(),
            );
        }
        self.mods.datapack_registries = datapack_registries.map(Arc::new);
        self.health_resources = health;
        self.side.world_tables.0 = tables;
        // Publish the paired item and world evaluators before any callbacks.
        self.load_server_loot();
        self.side.data_map_registries = self
            .side
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
        self.world.pois.clear();
        for world in self.side.levels.0.parked_worlds() {
            world.pois.clear();
        }
        sunrise_client::datapack_net::serve(self.mods.datapack_registries.clone());
        self.send_datapack_snapshots();
        self.refresh_recipe_property_context();
        self.side.structure_spawns = structure_spawns;
        self.refresh_infiniburn();
        self.install_parts(parts);
        self.damage_tag_generation = self.damage_tag_generation.wrapping_add(1);
        // Before the players are sent the reloaded data below.
        self.sync_recipe_content();
        // PlayerList.reloadResources: the tags and recipes go to the players
        // and their advancements reload. The synchronized registries (the
        // dialogs, and the commands' ids of them) stay the hub's.
        lan::sync::install_wire_reload(
            &self.options.resources,
            &self.res.core.item_data,
            &self.pack_files(),
            wire.clone(),
            advancements.clone(),
            wire_tags,
        );
        if self.lan.is_some()
            && let Some(sync) = wire
        {
            self.with_advancement_hub(|hub, owners| hub.reload_resources(sync, owners));
        }
        self.adv.tree = advancements;
        self.adv.reset_tree();
        self.announce_data_bound(mod_api::TagsUpdateCause::ServerDataLoad { reload: true });
        self.announce_data_maps_updated(mod_api::DataMapsUpdateCause::ServerReload);
    }
    /// Build and publish one server snapshot for this world load or reload.
    pub(in crate::game) fn load_server_loot(&mut self) {
        if let Some(tables) = &self.side.world_tables.0 {
            self.server_items = Some(tables.items.clone());
            self.copy_loot_enchantment_answers(&tables.loot);
            self.side.server_loot.0.set(Some(tables.loot.clone()));
            global_loot_modifiers::bind_retained(
                &self.side.reload_resources.0,
                &tables.loot_modifier_manager,
            );
            return;
        }
        self.server_items = None;
        let overlay = pack_files(&self.side.world_tables.0);
        let loot = match sunrise_world::loot::LootData::load_with_bound_tags(
            &self.options.resources,
            &overlay,
            self.res.item_defaults(),
            &self.health_resources.damage,
            &self.items().ordered_tags(),
        ) {
            Ok(mut loot) => {
                if !mod_api::__private::mods().is_empty() {
                    let context = match lan::sync::server_data_load(
                        &self.options.resources,
                        &self.res.core.item_data,
                    ) {
                        Ok(context) => context,
                        Err(error) => {
                            log::warn!("Sunrise: server loot inventory unavailable: {error}");
                            self.side.server_loot.0.set(None);
                            return;
                        }
                    };
                    if let Err(error) = loot.apply_native_table_loads(&context) {
                        log::warn!("Sunrise: native loot tables refused: {error}");
                        self.side.server_loot.0.set(None);
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
            self.server_items = Some(Arc::new(items));
        }
        if let Some(loot) = &loot {
            self.copy_loot_enchantment_answers(loot);
        }
        self.side.server_loot.0.set(loot);
    }
    /// A world open's tables, and the first generation of the registries the mods declared.
    pub(in crate::game) fn open_world_tables(
        &mut self,
        staged: super::condition_reload_state::StagedWorldTables,
    ) -> Result<()> {
        self.side.world_tables.0 = staged.tables;
        self.health_resources = staged.health;
        self.mods.datapack_registries = staged.datapack_registries.map(Arc::new);
        self.adv.tree = staged.advancements;
        self.adv.reset_tree();
        self.load_server_loot();
        self.side.data_map_registries = self
            .side
            .world_tables
            .0
            .as_ref()
            .map_or_else(Vec::new, |t| t.data_maps.registries());
        self.install_block_maps();
        self.install_fluid_interactions()?;
        self.install_profession_save_snapshots();
        self.share_mob_maps();
        self.share_biome_keys();
        sunrise_client::datapack_net::serve(self.mods.datapack_registries.clone());
        Ok(())
    }
    /// Reload the content tables with the world's packs, as
    /// `/reload` reloads a server's (`ReloadableServerResources`).
    pub(in crate::game) fn prepare_world_reload(
        &mut self,
        stack: &functions::PackStack,
        root: &std::path::Path,
    ) -> Result<PreparedWorldReload> {
        let super::condition_reload_state::StagedWorldTables {
            tables,
            advancements,
            health,
            datapack_registries,
            ..
        } = self.stage_world_tables(stack, true, root)?;
        let items = tables
            .as_ref()
            .map_or_else(|| self.res.shared_items(), |tables| tables.items.clone());
        let parts = self.prepare_parts(&items)?;
        let wire_tags = sunrise_client::lan::data::WorldTags::new(items.clone(), &health.damage);
        let overlay = pack_files(&tables);
        let kept = self
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
                        &self.res.core.item_data,
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
                    &self.res.core.item_data,
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
            structure_spawns: sunrise_client::mobs::spawn_selection::StructureSpawns::load(root),
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
        self.side.lan_acting_uuid
    }
    #[inline(always)]
    pub(in crate::game) fn server_clear_random_tick_sections(&mut self) {
        self.side.random_tick_sections.0.clear();
    }
    pub(in crate::game) fn server_resolve_block_paths(&self) {
        use mod_api::{HookId, PathComputationType};
        use sunrise_client::mobs::data::PartAnswers;
        let data = self.res.mob_data();
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
            self.res.facts(),
            &PartAnswers {
                pathfindable: pathfindable.as_deref(),
            },
        );
    }
    #[inline(always)]
    pub(in crate::game) fn server_tags_ready(&self) -> bool {
        self.side.session_lock.0.is_some()
    }
    #[inline(always)]
    pub(in crate::game) fn server_world_tables(&self) -> Option<&WorldTables> {
        self.side.world_tables.0.as_ref()
    }
    /// Load the world's data packs' content tables (`None`: the world has no
    /// packs, and `Resources.rest` stays in use). This re-reads the tables,
    /// so it runs once per world open and on `/reload`.
    pub(in crate::game) fn stage_world_tables(
        &mut self,
        stack: &functions::PackStack,
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
        let conditions = self.registered_load_conditions(&inventory, stack, root, &files)?;
        let overlay = sunrise_world::loot::PackOverlay {
            conditions: &conditions,
            files: &files,
            ..sunrise_world::loot::PackOverlay::default()
        };
        let mut data_map_types = Vec::new();
        self.native_data_map_types(&mut data_map_types);
        let base = if reload {
            &self.health_resources
        } else {
            &self.res.motion().health
        };
        let health = base.with_packs(root, &overlay, reload)?;
        // With no pack, `res` holds the tables, unless content mods switched
        // live since it was loaded: their data is in the new root.
        let mut items = Items::load_registry(
            root,
            self.pack.assets.registry,
            self.res.facts(),
            &overlay,
            self.res.item_defaults(),
            if reload {
                sunrise_client::items::LoadEntityTags::Reload(&self.items().entity_type_tags)
            } else {
                sunrise_client::items::LoadEntityTags::Frozen(
                    self.res.items().entity_type_tags.ids(),
                )
            },
        )?;
        if reload {
            items.retain_unbound_tags(self.items());
        }
        class_tags::install(&mut items, self.res.facts());
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
            if let Some(tables) = self.side.world_tables.0.as_ref().filter(|_| reload) {
                Ok(tables.packed_registries.clone())
            } else {
                // Admit the frozen pack definitions with this candidate's bound
                // tags, even if no peer joins and triggers the full wire load.
                sunrise_client::lan::data::SyncData::stage_registry_definitions(
                    root,
                    self.res.item_defaults(),
                    &overlay,
                    &read_errors,
                    &self.ui.registries,
                    |id| {
                        self.ui
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
            self.res.core.item_data.defaults(),
            &health.damage,
            &items.ordered_tags(),
        );
        let advancements = Arc::new(sunrise_client::advancements::Tree::load_with(
            &root.join("data"),
            &overlay,
        ));
        let datapack_candidate = self.datapack_candidate(stack, &conditions);
        let install_items_result = match &packed_registry_definitions {
            Ok(packed) => {
                packed.install_items(&mut items, root, &overlay, self.res.item_defaults())
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
        let _recipe_files = sunrise_client::json_folder::read(&root.join("data/minecraft/recipe"));
        let sources = if reload {
            lan::sync::prepare_server_sources(root, &self.res.core.item_data, &items, &overlay)
        } else {
            lan::sync::load_sources(
                root,
                &self.res.core.item_data,
                &items,
                self.options.execution_role.data_load_origin(),
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
        let mut stations = sunrise_client::stations::StationData::from_sources(
            root,
            &items,
            &overlay,
            self.res.item_defaults(),
            (&sources, &recipes),
        );
        self.prepare_professions(root, &items, &mut stations)?;
        // The world's loaded datapack registries are a mod-declared registry's
        // authority for the data maps' id checks, as the command ids are for
        // the registries they list.
        let mut datapack_candidate = datapack_candidate?;
        if let Some(candidate) = &mut datapack_candidate {
            native_registry_entries::bind_datapack_tags(root, &tag_files, candidate)?;
        }
        let datapack_registries =
            datapack_candidate.map(mod_api::DatapackRegistrySet::from_candidate);
        let received = sunrise_client::datapack_net::received();
        let datapack_set = if self.is_logical_server() {
            datapack_registries.as_ref()
        } else {
            received.as_deref()
        };
        let registries = native_data_maps::WorldRegistries::new(
            self.ui
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
        let suffocation = sunrise_client::mobs::suffocation::bind(
            self.res.core.collision.clone(),
            &causes_suffocation,
        )?;
        // A registry whose pack tags don't load keeps the game's own tags
        // (`in_tag` falls back to them), rather than matching no tag at all.
        let pack_tags: HashMap<_, _> = WorldTables::PACK_TAG_REGISTRIES
            .into_iter()
            .chain(sunrise_client::neoforge_mob_maps::MAPS.map(|definition| definition.directory))
            .filter(|directory| !matches!(*directory, "item" | "entity_type" | "damage_type"))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter_map(|registry| {
                match sunrise_client::items::load_tags(root, registry, &tag_files) {
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
                    .world_biomes
                    .as_ref()
                    .unwrap_or_else(|| self.res.biomes())
                    .contains(id),
                "minecraft:entity_type" => {
                    self.res.mob_types().contains_key(id)
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
        let data_maps = native_data_maps::DataMaps::load(&data_map_types, &overlay, &registries)
            .with_builtins(builtin)
            .with_biome_keys(mod_api::__private::BiomeKeys::new(
                next_biome_keys_session(),
                self.world_biomes
                    .as_ref()
                    .unwrap_or_else(|| self.res.biomes())
                    .registry_order(),
            ));
        drop(registries);
        // The API's key order is live state as well: do not change it for a
        // candidate refused by the condition readers.
        api_values::resolve_enchantment_registry(&items)?;
        let context = lan::sync::server_data_load(root, &self.res.core.item_data)?;
        if !mod_api::__private::mods().is_empty() {
            loot.apply_native_table_loads(&context)?;
        }
        let serializers = self
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
        stack: &functions::PackStack,
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
            sunrise_client::datapack::registry_files(&packs, &keys, |path, bytes| {
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
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        for guest in host
            .hub
            .guests
            .iter()
            .filter(|g| g.placed && g.participates_online() && g.brand == "sunrise")
        {
            sunrise_client::datapack_net::reload_connection(&guest.sender);
        }
    }

    /// Close guest stonecutter windows before publishing a reloaded recipe generation.
    pub(in crate::game) fn close_stonecutters_for_reload(&mut self) {
        let Some(host) = self.lan.as_ref() else {
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
