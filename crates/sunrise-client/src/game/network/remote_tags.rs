//! The server's tags on this client (`ClientPacketListener.handleUpdateTags`, and configuration's
//! `RegistryDataCollector`): install gameplay tags, then notify native mods.
use super::Game;
use std::sync::Arc;
use sunrise_items::items::{Items, TagRegistry};
use sunrise_protocol::{item::Names, packets::configuration::UpdateTags, registry::SyncedNames};

/// The registries whose tags the tag helpers read.
pub(in crate::game) const READ_REGISTRIES: [(&str, TagRegistry); 3] = [
    ("minecraft:block", TagRegistry::Block),
    ("minecraft:item", TagRegistry::Item),
    ("minecraft:fluid", TagRegistry::Fluid),
];
/// The synced registry whose tags `Tags::damage_type_in_tag` reads.
pub(in crate::game) const DAMAGE_TYPE: &str = "minecraft:damage_type";

impl Game<'_, crate::game::client_side::ClientState> {
    /// `ClientboundUpdateTagsPacket` in play.
    pub(in crate::game) fn remote_update_tags(&mut self, tags: &UpdateTags) {
        if let Some(r) = self.remote.as_mut() {
            Arc::make_mut(&mut r.registries).add_tags(tags);
        }
        self.install_server_items();
        self.client_refresh_infiniburn();
        self.refresh_creative_search_tags();
        self.install_remote_tags();
        if self.recipe_tables.server_stations.is_some()
            && tags
                .registries
                .iter()
                .any(|tags| tags.registry == "minecraft:item")
        {
            self.refresh_remote_stonecutter();
        }
    }

    pub(in crate::game::client_side) fn refresh_creative_search_tags(&mut self) {
        let tags = self.remote.as_ref().and_then(|r| {
            let ids = r.builtin.get("minecraft:item")?;
            let tags = r.registries.tags.get("minecraft:item")?;
            Some(
                tags.iter()
                    .map(|(tag, members)| {
                        let items = members
                            .iter()
                            .filter_map(|&id| ids.name(id).map(str::to_owned))
                            .collect();
                        (tag.clone(), items)
                    })
                    .collect(),
            )
        });
        self.side.creative_screen.0.set_item_tags(tags.as_ref());
    }

    /// Make the server's tags what native mods read, then tell them. Snapshot client
    /// enchant answers only after the tags, rebound parts and tag listeners are published.
    pub(in crate::game) fn install_remote_tags(&mut self) {
        if mod_api::__private::mods().is_empty() {
            self.resolve_enchant_particles();
            return;
        }
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        let damage_types = r.registries.registries.get(DAMAGE_TYPE);
        if r.registries.tags.is_empty() && damage_types.is_none() {
            r.tags = None;
            self.refresh_recipe_property_context();
            return;
        }
        let names = SyncedNames {
            builtin: &r.builtin,
            dynamic: &r.registries,
        };
        let mut items = Items::empty();
        for (key, registry) in READ_REGISTRIES {
            let Some(tags) = r.registries.tags.get(key) else {
                continue;
            };
            let members = |ids: &[i32]| ids.iter().filter_map(|&id| names.name(key, id)).collect();
            let tags = tags.iter().map(|(tag, ids)| (tag.clone(), members(ids)));
            items.set_tags(registry, tags.collect());
        }
        // The damage type registry and its tags, for `Tags::damage_type_in_tag`. A registry
        // sent with no tag group binds its types with no tags, as vanilla binds received
        // elements whose tag payload was omitted.
        if let Some(types) = damage_types {
            let members = |ids: &[i32]| {
                ids.iter()
                    .filter_map(|&id| names.name(DAMAGE_TYPE, id))
                    .collect()
            };
            items.bind_damage_registry(
                types.iter().map(|(name, _)| name.clone()).collect(),
                r.registries
                    .tags
                    .get(DAMAGE_TYPE)
                    .into_iter()
                    .flatten()
                    .map(|(tag, ids)| (tag.clone(), members(ids)))
                    .collect(),
            );
        }
        super::class_tags::install(&mut items, self.res.facts());
        // Rebuild scoped parts against the received tags before refreshing state-only facts.
        let mut part_items = self.items().clone();
        let names = items.block_state_names();
        part_items.block_tags = items.block_tags.clone();
        part_items.rebuild_block_tag_flags(names);
        part_items.item_tags = items.item_tags.clone();
        let parts = match self.prepare_parts(&part_items) {
            Ok(parts) => parts,
            Err(error) => {
                log::warn!("Sunrise: client tag parts refused: {error}");
                return;
            }
        };
        if let Some(r) = self.remote.as_mut() {
            r.tags = Some(items);
        }
        self.bind_parts(parts);
        self.refresh_recipe_property_context();
        self.announce_data_bound(mod_api::TagsUpdateCause::ClientPacketReceived {
            integrated_server: self.integrated.is_some(),
        });
        self.resolve_primary_items();
        self.resolve_enchant_particles();
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use crate::game::native_entity_refs::tests::with_client_game;
    use mod_api::{Hooks, Identifier, ModEntry, TagMembers, TagsUpdateCause, TagsUpdatedEvent};
    use mod_platform::__private::part_tests::block_key;
    use std::sync::{Arc, Mutex};
    use sunrise_protocol::packets::{
        configuration::{RegistryTags, TagEntry, UpdateTags},
        play::{self, Clientbound as C},
    };
    use sunrise_protocol::registry::Builtin;

    /// What a listener read of the tags during its notice.
    #[derive(Debug, PartialEq)]
    pub(in crate::game) struct Seen {
        pub(in crate::game) cause: TagsUpdateCause,
        pub(in crate::game) items: TagMembers<mod_api::ItemKey>,
        pub(in crate::game) empty_items: TagMembers<mod_api::ItemKey>,
        pub(in crate::game) fluids: TagMembers<mod_api::FluidKey>,
        pub(in crate::game) stone_in_block_tag: Option<bool>,
        pub(in crate::game) dirt_in_block_tag: Option<bool>,
        pub(in crate::game) stick_in_item_tag: Option<bool>,
        pub(in crate::game) lava_in_fluid_tag: Option<bool>,
    }

    pub(in crate::game) struct Probe(pub(in crate::game) Arc<Mutex<Vec<Seen>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Probe {
        fn tags_updated(&self, context: &dyn mod_api::ReloadContext, event: &mut TagsUpdatedEvent) {
            assert!(
                matches!(event.cause, TagsUpdateCause::ClientPacketReceived { .. }),
                "the server's tags are a client's to apply"
            );
            let tags = context.tags();
            let key = |tag| tags.key(&Identifier::parse(tag).unwrap()).unwrap();
            let stick = context.item_key(&Identifier::parse("minecraft:stick").unwrap());
            self.0.lock().unwrap().push(Seen {
                cause: event.cause,
                items: tags.items(key("test:items")),
                empty_items: tags.items(key("test:empty_items")),
                fluids: tags.fluids(key("test:fluids")),
                stone_in_block_tag: context
                    .block_in_tag(block_key("minecraft:stone"), "test:blocks"),
                dirt_in_block_tag: context.block_in_tag(block_key("minecraft:dirt"), "test:blocks"),
                stick_in_item_tag: tags.is_in_tag(stick.unwrap(), key("test:items")),
                lava_in_fluid_tag: context.fluid_in_tag("minecraft:lava", "test:fluids"),
            });
        }
    }

    pub(in crate::game::client_side) fn update(
        builtin: &Builtin,
        tags: &[(&str, &str, &[&str])],
    ) -> UpdateTags {
        let mut registries = std::collections::BTreeMap::<_, Vec<_>>::new();
        for &(registry, tag, members) in tags {
            let names = builtin.get(registry).unwrap();
            registries.entry(registry).or_default().push(TagEntry {
                name: tag.into(),
                ids: members.iter().map(|name| names.id(name).unwrap()).collect(),
            });
        }
        UpdateTags {
            registries: registries
                .into_iter()
                .map(|(registry, tags)| RegistryTags {
                    registry: registry.into(),
                    tags,
                })
                .collect(),
        }
    }

    pub(in crate::game) fn login() -> C {
        C::Login(play::Login {
            player_id: 7,
            hardcore: false,
            levels: vec!["minecraft:overworld".into()],
            max_players: 20,
            chunk_radius: 8,
            simulation_distance: 8,
            reduced_debug_info: false,
            show_death_screen: true,
            limited_crafting: false,
            spawn: play::SpawnInfo {
                dimension_type: 0,
                dimension: "minecraft:overworld".into(),
                seed: 1,
                game_type: 0,
                previous_game_type: None,
                debug: false,
                flat: false,
                last_death: None,
                portal_cooldown: 0,
                sea_level: 63,
            },
            online_mode: false,
            enforces_secure_chat: false,
        })
    }

    #[test]
    #[ignore = "daily"]
    fn item_tag_lists_follow_server_reload_and_client_packets() {
        use crate::game::{
            lan::sync,
            native_entity_refs::tests::{run_fresh_cases, with_game},
            world_loading::WorldRequest,
        };
        use mod_api::Tags;

        if run_fresh_cases(
            "game::client_side::remote_tags::tests::item_tag_lists_follow_server_reload_and_client_packets",
            "SUNRISE_ITEM_TAG_LIST_FIXTURE",
            &["reload"],
        ) {
            return;
        }

        struct ItemTagProbe(Arc<Mutex<Vec<TagsUpdateCause>>>);
        #[mod_api::__subscriptions]
        impl Hooks for ItemTagProbe {
            fn tags_updated(
                &self,
                context: &dyn mod_api::ReloadContext,
                event: &mut TagsUpdatedEvent,
            ) {
                let mut seen = self.0.lock().unwrap();
                let tags = context.tags();
                let id = |path| Identifier::from_parts("item_tag_listing", path).unwrap();
                // Allocating a key for an unbound name must not make it discoverable.
                tags.key(&id("never_bound")).unwrap();
                let stick = context
                    .item_key(&Identifier::parse("minecraft:stick").unwrap())
                    .unwrap();
                let scoped = |names: Option<Vec<Identifier>>| {
                    let mut names: Vec<_> = names
                        .unwrap()
                        .into_iter()
                        .filter(|id| id.namespace() == "item_tag_listing")
                        .collect();
                    names.sort_unstable();
                    names
                };
                let reloaded = seen.len() % 2 == 1;
                let current = if reloaded { "new" } else { "old" };
                assert_eq!(scoped(tags.item_tags(stick)), [id("always"), id(current)]);
                assert_eq!(
                    scoped(tags.item_tag_names()),
                    [id("always"), id("empty"), id(current)]
                );
                let key = |path| tags.key(&id(path)).unwrap();
                assert_eq!(tags.is_in_tag(stick, key(current)), Some(true));
                if reloaded {
                    assert_eq!(tags.is_in_tag(stick, key("old")), Some(false));
                }
                assert_eq!(tags.items(key("empty")), TagMembers::Members(vec![]));
                seen.push(event.cause);
            }
        }

        let save = tempfile::tempdir().unwrap();
        let pack = save.path().join("datapacks/test");
        let tags = pack.join("data/item_tag_listing/tags/item");
        std::fs::create_dir_all(&tags).unwrap();
        std::fs::write(
            save.path().join("datapacks.json"),
            br#"{"enabled":["vanilla","file/test"],"disabled":[]}"#,
        )
        .unwrap();
        std::fs::write(
            pack.join("pack.mcmeta"),
            br#"{"pack":{"description":"Item tag listing fixture","min_format":94,"max_format":94}}"#,
        )
        .unwrap();
        let write = |path: &str, values: &[&str]| {
            std::fs::write(
                tags.join(path),
                serde_json::json!({"values": values}).to_string(),
            )
            .unwrap();
        };
        write("old.json", &["minecraft:stick"]);
        write("always.json", &["minecraft:stick"]);
        write("empty.json", &[]);

        let seen = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![ModEntry::new(
            "item_tag_listing",
            Box::new(ItemTagProbe(seen.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            let mut packets = Vec::new();
            with_game(|game| {
                let mut request = WorldRequest::new(
                    "test",
                    Some("test"),
                    Some(1),
                    true,
                    None,
                    Arc::new(sunrise_client_core::gamerules::RuleRegistry::vanilla()),
                )
                .unwrap();
                request.save_root = save.path().to_owned();
                game.load_world(request).unwrap();
                let packet = |game: &crate::game::ServerGame<'_>| {
                    sync::sync_data(
                        &game.options.resources,
                        &game.res.core.item_data,
                        game.items(),
                        &game.pack_files(),
                    )
                    .unwrap()
                    .tags
                    .clone()
                };
                packets.push(packet(game));
                std::fs::remove_file(tags.join("old.json")).unwrap();
                write("new.json", &["minecraft:stick"]);
                game.reload_datapacks(true, &game.level.enabled_features.clone())
                    .unwrap();
                packets.push(packet(game));
            });
            with_client_game(|game| {
                let (pipe, _server) = sunrise_protocol::connection::Pipe::pair();
                game.connect_local(pipe, "Steve").unwrap();
                game.handle_packet(login()).unwrap();
                let stick = mod_api::__private::item_key("minecraft:stick").unwrap();
                assert_eq!(game.item_tags(stick), None);
                assert_eq!(game.item_tag_names(), None);
                for packet in packets {
                    game.handle_packet(C::UpdateTags(packet)).unwrap();
                }
            });
        });
        let received = TagsUpdateCause::ClientPacketReceived {
            integrated_server: false,
        };
        assert_eq!(
            *seen.lock().unwrap(),
            [
                TagsUpdateCause::ServerDataLoad { reload: false },
                TagsUpdateCause::ServerDataLoad { reload: true },
                received,
                received,
            ]
        );
    }

    #[test]
    #[ignore = "daily"]
    fn a_client_notice_reads_each_applied_set_of_server_tags() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![ModEntry::new("tags_probe", Box::new(Probe(seen.clone())))].into();
        mod_platform::testing::with_mods(mods, || {
            with_client_game(|game| {
                let (pipe, _server) = sunrise_protocol::connection::Pipe::pair();
                game.connect_local(pipe, "Steve").unwrap();
                let builtin = game.remote.as_ref().unwrap().builtin.clone();

                use mod_api::LoopContext;
                use sunrise_level::api_context::LoopReader;
                let tag =
                    mod_api::__private::tag_key(&Identifier::parse("test:items").unwrap()).unwrap();
                let stick = mod_api::__private::item_key("minecraft:stick").unwrap();
                let potion = mod_api::__private::item_key("minecraft:potion").unwrap();
                let empty_tag =
                    mod_api::__private::tag_key(&Identifier::parse("test:empty_items").unwrap())
                        .unwrap();
                assert_eq!(mod_api::Tags::items(game, tag), TagMembers::Unavailable);
                let item_reads = |game: &crate::game::ClientGame<'_>| {
                    let reader = LoopReader(&game.loop_context);
                    let view = reader.context();
                    (
                        view.is_in_tag(stick, tag),
                        view.items_in_tag(tag).map(<[_]>::to_vec),
                    )
                };

                // Configuration's tags wait for the login, which binds the client's blocks.
                let registries = &mut game.remote.as_mut().unwrap().registries;
                Arc::make_mut(registries).add_tags(&update(
                    &builtin,
                    &[
                        (
                            "minecraft:block",
                            "test:blocks",
                            &["minecraft:stone", "minecraft:dirt"],
                        ),
                        ("minecraft:fluid", "test:fluids", &["minecraft:lava"]),
                    ],
                ));
                assert!(seen.lock().unwrap().is_empty());
                assert!(game.native_tag_items().is_none());

                game.handle_packet(login()).unwrap();
                let lava = mod_api::__private::fluid_key("minecraft:lava").unwrap();
                let received = TagsUpdateCause::ClientPacketReceived {
                    integrated_server: false,
                };
                assert_eq!(seen.lock().unwrap()[0].stone_in_block_tag, Some(true));
                assert_eq!(seen.lock().unwrap()[0].items, TagMembers::Unavailable);
                assert_eq!(seen.lock().unwrap()[0].stick_in_item_tag, None);
                assert_eq!(item_reads(game), (None, None));

                game.handle_packet(C::UpdateTags(UpdateTags {
                    registries: vec![RegistryTags {
                        registry: "minecraft:item".into(),
                        tags: Vec::new(),
                    }],
                }))
                .unwrap();
                assert_eq!(item_reads(game), (Some(false), Some(vec![])));
                assert_eq!(seen.lock().unwrap()[1].items, TagMembers::Missing);
                assert_eq!(seen.lock().unwrap()[1].stick_in_item_tag, Some(false));

                let mut files: Vec<_> = crate::game::native_tags::PACK_BINDING_CASES
                    .iter()
                    .map(|(path, bytes, _)| {
                        (format!("tags/binding:item/{path}"), vec![bytes.to_vec()])
                    })
                    .collect();
                files.extend([
                    (
                        "tags/test:item/items".into(),
                        vec![br#"{"values":["minecraft:potion","minecraft:stick"]}"#.to_vec()],
                    ),
                    (
                        "tags/test:item/empty_items".into(),
                        vec![br#"{"values":[]}"#.to_vec()],
                    ),
                ]);
                let overlay = sunrise_world::loot::PackOverlay::new(&files);
                let root = &game.options.resources;
                let item_data = &game.res.core.item_data;
                // Build the server's item catalog from its packs, not the client table whose
                // item tags were intentionally replaced by the preceding empty update.
                let server_items =
                    sunrise_items::items::Items::catalog(root, &overlay, item_data.defaults())
                        .unwrap();
                let sources = crate::game::lan::sync::server_sources(
                    root,
                    item_data,
                    &server_items,
                    &overlay,
                )
                .unwrap();
                let wire = sunrise_sim::lan::data::SyncData::load(
                    root,
                    &overlay,
                    item_data.defaults(),
                    &sources,
                )
                .unwrap();
                // Deliver just the item table: the block/fluid snapshots above are independent.
                let registries = wire
                    .tags
                    .registries
                    .into_iter()
                    .filter(|tags| tags.registry == "minecraft:item")
                    .collect();
                game.handle_packet(C::UpdateTags(UpdateTags { registries }))
                    .unwrap();
                for (path, _, is_bound) in crate::game::native_tags::PACK_BINDING_CASES {
                    let key = mod_api::__private::tag_key(
                        &Identifier::parse(&format!("binding:{path}")).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(
                        mod_api::Tags::items(game, key),
                        if is_bound {
                            TagMembers::Members(vec![])
                        } else {
                            TagMembers::Missing
                        }
                    );
                }
                assert_eq!(item_reads(game), (Some(true), Some(vec![potion, stick])));
                assert_eq!(
                    seen.lock().unwrap()[2],
                    Seen {
                        cause: received,
                        items: TagMembers::Members(vec![potion, stick]),
                        empty_items: TagMembers::Members(vec![]),
                        fluids: TagMembers::Members(vec![lava]),
                        stone_in_block_tag: Some(true),
                        dirt_in_block_tag: Some(true),
                        stick_in_item_tag: Some(true),
                        lava_in_fluid_tag: Some(true),
                    }
                );

                game.handle_packet(C::UpdateTags(update(
                    &builtin,
                    &[("minecraft:block", "test:blocks", &["minecraft:stone"])],
                )))
                .unwrap();
                assert_eq!(item_reads(game), (Some(true), Some(vec![potion, stick])));

                // An update in play replaces the registries it names and keeps the others.
                game.handle_packet(C::UpdateTags(update(
                    &builtin,
                    &[
                        ("minecraft:block", "test:blocks", &["minecraft:dirt"]),
                        ("minecraft:item", "test:other", &["minecraft:stick"]),
                    ],
                )))
                .unwrap();
                assert_eq!(item_reads(game), (Some(false), Some(vec![])));
                assert_eq!(mod_api::Tags::items(game, empty_tag), TagMembers::Missing);
                let seen = seen.lock().unwrap();
                assert_eq!(seen.len(), 5);
                assert_eq!(
                    seen[4],
                    Seen {
                        cause: received,
                        items: TagMembers::Missing,
                        empty_items: TagMembers::Missing,
                        fluids: TagMembers::Members(vec![lava]),
                        stone_in_block_tag: Some(false),
                        dirt_in_block_tag: Some(true),
                        stick_in_item_tag: Some(false),
                        lava_in_fluid_tag: Some(true),
                    }
                );
            });
        });
    }

    pub(in crate::game::client_side) fn power_update(
        builtin: &Builtin,
        tags: &[(&str, &[&str])],
    ) -> UpdateTags {
        let blocks = builtin.get("minecraft:block").unwrap();
        UpdateTags {
            registries: vec![RegistryTags {
                registry: "minecraft:block".into(),
                tags: tags
                    .iter()
                    .map(|(name, members)| TagEntry {
                        name: (*name).into(),
                        ids: members.iter().map(|id| blocks.id(id).unwrap()).collect(),
                    })
                    .collect(),
            }],
        }
    }

    #[test]
    #[ignore = "daily"]
    fn remote_tag_packets_rebuild_vanilla_enchant_providers_and_transmitters_without_mods() {
        use crate::game::enchant_power_beam_tests::{enchant_particles, place};
        mod_platform::testing::with_mods(Vec::new().into(), || {
            with_client_game(|game| {
                let (pipe, _server) = sunrise_protocol::connection::Pipe::pair();
                game.connect_local(pipe, "PowerTags").unwrap();
                game.handle_packet(login()).unwrap();
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)));
                place(game, [0, 0, 0], "minecraft:enchanting_table");
                place(game, [-2, 0, -1], "minecraft:bookshelf");
                let builtin = game.remote.as_ref().unwrap().builtin.clone();
                let mut send = |providers: &[&str], transmitters: &[&str]| {
                    game.handle_packet(C::UpdateTags(power_update(
                        &builtin,
                        &[
                            ("minecraft:enchantment_power_provider", providers),
                            ("minecraft:enchantment_power_transmitter", transmitters),
                        ],
                    )))
                    .unwrap();
                    enchant_particles(game)
                };
                assert!(send(&["minecraft:bookshelf"], &["minecraft:air"]) > 0);
                assert_eq!(send(&[], &["minecraft:air"]), 0, "provider tag removed");
                assert!(send(&["minecraft:bookshelf"], &["minecraft:air"]) > 0);
                assert_eq!(
                    send(&["minecraft:bookshelf"], &[]),
                    0,
                    "transmitter tag removed"
                );
                assert!(send(&["minecraft:bookshelf"], &["minecraft:air"]) > 0);
            });
        });
    }

    #[test]
    #[ignore = "daily"]
    fn remote_enchant_snapshot_follows_tag_rebinding_and_tags_updated_callbacks() {
        use crate::game::enchant_power_beam_tests::{enchant_particles, place};
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        static ENABLED: AtomicBool = AtomicBool::new(false);
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static NOTICES: AtomicUsize = AtomicUsize::new(0);
        struct Provider;
        #[mod_api::sunrise_part]
        impl mod_api::BlockPart for Provider {
            fn enchant_power_bonus(
                &self,
                host: &dyn mod_api::Host,
                event: &mut mod_api::EnchantPowerBonusEvent,
            ) {
                CALLS.fetch_add(1, Ordering::Relaxed);
                let received =
                    host.block_in_tag(event.state.block(), "power_test:providers") == Some(true);
                event.power = if received && ENABLED.load(Ordering::Relaxed) {
                    2.0
                } else {
                    0.0
                };
            }
        }
        struct PowerMod;
        #[mod_api::__subscriptions]
        impl Hooks for PowerMod {
            fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
                r.block_key("minecraft:glass");
                r.block_part_tag("power_test:providers", Provider);
            }
            fn tags_updated(
                &self,
                context: &dyn mod_api::ReloadContext,
                event: &mut TagsUpdatedEvent,
            ) {
                if !matches!(event.cause, TagsUpdateCause::ClientPacketReceived { .. }) {
                    return;
                }
                ENABLED.store(
                    context.block_in_tag(block_key("minecraft:glass"), "power_test:enabled")
                        == Some(true),
                    Ordering::Relaxed,
                );
                NOTICES.fetch_add(1, Ordering::Relaxed);
            }
        }
        let mods: mod_api::Mods = vec![ModEntry::new("power_test", Box::new(PowerMod))].into();
        crate::game::api_values::install_parts(&mods).unwrap();
        crate::entry::install_client_hooks();
        mod_platform::testing::with_mods(mods, || {
            with_client_game(|game| {
                let (pipe, _server) = sunrise_protocol::connection::Pipe::pair();
                game.connect_local(pipe, "PowerCallbacks").unwrap();
                game.handle_packet(login()).unwrap();
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)));
                place(game, [0, 0, 0], "minecraft:enchanting_table");
                place(game, [-2, 0, -1], "minecraft:glass");
                let builtin = game.remote.as_ref().unwrap().builtin.clone();
                let mut send = |attached: bool, enabled: bool| {
                    let calls = CALLS.load(Ordering::Relaxed);
                    let notices = NOTICES.load(Ordering::Relaxed);
                    game.handle_packet(C::UpdateTags(power_update(
                        &builtin,
                        &[
                            ("minecraft:enchantment_power_provider", &[]),
                            (
                                "minecraft:enchantment_power_transmitter",
                                &["minecraft:air"],
                            ),
                            (
                                "power_test:providers",
                                if attached { &["minecraft:glass"] } else { &[] },
                            ),
                            (
                                "power_test:enabled",
                                if enabled { &["minecraft:glass"] } else { &[] },
                            ),
                        ],
                    )))
                    .unwrap();
                    assert_eq!(NOTICES.load(Ordering::Relaxed), notices + 1);
                    assert_eq!(
                        CALLS.load(Ordering::Relaxed),
                        calls + usize::from(attached),
                        "one snapshot after publication, never before the callback"
                    );
                    let particles = enchant_particles(game);
                    assert_eq!(
                        CALLS.load(Ordering::Relaxed),
                        calls + usize::from(attached),
                        "particle frames must not query parts"
                    );
                    particles
                };
                assert!(
                    send(true, true) > 0,
                    "new tags visible to both callback and part"
                );
                assert_eq!(
                    send(true, false),
                    0,
                    "callback answer changed before snapshot"
                );
                assert!(send(true, true) > 0);
                assert_eq!(
                    send(false, true),
                    0,
                    "tag-bound part detached before snapshot"
                );
            });
        });
    }

    /// A client's damage type tags are the ones its server sent, at join and after a tag
    /// update, for its own tag service, a tag listener and the hit host of a mount it predicts.
    #[test]
    #[ignore = "daily"]
    fn a_client_reads_the_damage_type_tags_its_server_sent() {
        use crate::game::native_entity_refs::tests::run_fresh_cases;
        use mod_api::{LevelHost, Tags};
        if run_fresh_cases(
            "game::client_side::remote_tags::tests::a_client_reads_the_damage_type_tags_its_server_sent",
            "SUNRISE_TEST_CLIENT_DAMAGE_TYPE_TAGS",
            &["client", "no_group"],
        ) {
            return;
        }
        let case = std::env::var("SUNRISE_TEST_CLIENT_DAMAGE_TYPE_TAGS").unwrap();
        const TYPES: [&str; 3] = [
            "minecraft:arrow",
            "minecraft:mob_attack",
            "minecraft:in_fire",
        ];
        /// Whether `arrow` and `mob_attack` are in `minecraft:is_projectile`, and what an
        /// unknown damage type answers.
        type Reads = [Option<bool>; 3];
        fn reads(tags: &dyn Tags) -> Reads {
            let key = tags
                .key(&Identifier::parse("minecraft:is_projectile").unwrap())
                .unwrap();
            [
                tags.damage_type_in_tag("minecraft:arrow", key),
                tags.damage_type_in_tag("minecraft:mob_attack", key),
                tags.damage_type_in_tag("test:no_such_damage_type", key),
            ]
        }
        struct DamageTagProbe(Arc<Mutex<Vec<Reads>>>);
        #[mod_api::__subscriptions]
        impl Hooks for DamageTagProbe {
            fn tags_updated(&self, context: &dyn mod_api::ReloadContext, _: &mut TagsUpdatedEvent) {
                self.0.lock().unwrap().push(reads(context.tags()));
            }
        }
        // `is_projectile` holding the damage types at these registry ids.
        let projectiles = |ids: Vec<i32>| UpdateTags {
            registries: vec![RegistryTags {
                registry: "minecraft:damage_type".into(),
                tags: vec![TagEntry {
                    name: "minecraft:is_projectile".into(),
                    ids,
                }],
            }],
        };
        let predicting = |game: &mut crate::game::ClientGame<'_>| {
            let hooks = game.damage_hooks.client_side();
            let mut lent = sunrise_sim::health::LentLevel::take(&mut game.world);
            let seen = {
                let hit = crate::game::native_hooks::HitLevel(
                    game.predicting_damage_level(&mut lent, &hooks),
                );
                reads(hit.tags())
            };
            lent.give_back(&mut game.world);
            seen
        };
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![ModEntry::new(
            "damage_tag_probe",
            Box::new(DamageTagProbe(seen.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_client_game(|game| {
                let (pipe, _server) = sunrise_protocol::connection::Pipe::pair();
                game.connect_local(pipe, "Steve").unwrap();
                let registries = Arc::make_mut(&mut game.remote.as_mut().unwrap().registries);
                registries.registries.insert(
                    "minecraft:damage_type".into(),
                    TYPES.map(|name| (name.to_owned(), None)).to_vec(),
                );
                if case == "no_group" {
                    // A registry sent with no damage type tags: its types are known and
                    // in no tag; an unknown type stays unanswered.
                    game.handle_packet(login()).unwrap();
                    let joined = [Some(false), Some(false), None];
                    assert_eq!(reads(game), joined, "at join with no tag group");
                    assert_eq!(predicting(game), joined, "the predicting hit host");
                    assert_eq!(*seen.lock().unwrap(), [joined]);
                    return;
                }
                registries.add_tags(&projectiles(vec![0]));
                assert_eq!(reads(game), [None; 3], "before the login installs them");
                assert_eq!(predicting(game), [None; 3]);

                game.handle_packet(login()).unwrap();
                let joined = [Some(true), Some(false), None];
                assert_eq!(reads(game), joined, "at join");
                assert_eq!(predicting(game), joined, "the predicting hit host at join");

                game.handle_packet(C::UpdateTags(projectiles(vec![1])))
                    .unwrap();
                let updated = [Some(false), Some(true), None];
                assert_eq!(reads(game), updated, "after a tag update");
                assert_eq!(
                    predicting(game),
                    updated,
                    "the predicting hit host after it"
                );
                assert_eq!(*seen.lock().unwrap(), [joined, updated]);
            });
        });
    }
}
