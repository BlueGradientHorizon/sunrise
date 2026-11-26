//! A native mod's general rows as a Sunrise client sees them (#792, stage 5): the server's
//! `sunrise:general_entity` spawn adds one immediately, the vanilla packets keyed by its network
//! id move and remove it as they do any tracked entity, and its declared fields are its synced
//! attachments (`Remote::attachments`). Initial non-empty synced attachments arrive atomically
//! with the spawn inside a `BundleDelimiter` pairing bundle (`ServerEntity.addPairing` +
//! `ClientPacketListener.handleBundlePacket`, matching NeoForge `AttachmentSync`). Render
//! extraction retains the published generation, not the live synced holder. The row's behaviour
//! is the server's.
use super::{Game, Lerp, Remote};
use crate::game::general::general_type_catalog;
use indexmap::IndexMap;
use std::collections::HashMap;
use sunrise_core::geometry::{Aabb, Vec3};
#[allow(unused_imports)]
use sunrise_protocol::general_entity::{self, Message};
use sunrise_sim::join_seq::JoinSeq;

/// One general row the server shows the window.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) struct RemoteGeneral {
    /// Admission to the level, even if the first attachment snapshot has yet to arrive.
    pub join: JoinSeq,
    /// The client's frozen catalog slot for the row's type.
    pub type_slot: usize,
    pub uuid: u128,
    pub position: [f64; 3],
    /// Where it was when the last client tick began, for a frame's partial tick.
    pub previous: [f64; 3],
    /// Yaw, then pitch, in degrees.
    pub rotation: [f32; 2],
    pub previous_rotation: [f32; 2],
    /// Client ticks since the spawn: `Entity.tickCount`, which wraps as Java's `int` does
    /// (read as `i32`).
    pub age: u32,
    /// Frozen synced fields, initialized on spawn and rebuilt at each attachment sync boundary.
    pub render_attachments: mod_api::EntityRenderAttachments,
}

impl RemoteGeneral {
    pub(in crate::game) fn bounds(&self) -> Aabb {
        let [x, y, z] = self.position;
        general_type_catalog().bounds(self.type_slot, Vec3::new(x, y, z))
    }

    pub(in crate::game) fn kind(&self) -> &'static str {
        general_type_catalog().key(self.type_slot)
    }
}

/// The rows the server shows the window, by network id, and the warnings already given.
#[derive(Default)]
pub(in crate::game) struct RemoteGenerals {
    /// Client entity lookup order: inserts append; removing one keeps the others in order.
    pub rows: IndexMap<i32, RemoteGeneral>,
    /// Type keys this client's catalog does not hold, each warned about once.
    pub(in crate::game) unknown: std::collections::HashSet<String>,
    /// Whether a spawn for an id already held was refused before, warned about once.
    pub(in crate::game) duplicate: bool,
    // Reuse the parent-before-child placement order while the window ticks.
    pub(in crate::game) riders: Vec<(usize, i32, u32, usize)>,
}

impl Remote {
    /// Discard the synced attachment mirror.
    pub(in crate::game) fn clear_attachments(&mut self) {
        self.attachments.clear();
    }
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// The declared type key of general row `id`: the level's own row, or on a client the
    /// one its server shows it.
    pub(in crate::game) fn general_kind(&self, id: u32) -> Option<&'static str> {
        let slot = match self.resident_entities.general.row(id) {
            Some(row) => row.type_slot,
            None => {
                self.remote
                    .as_ref()?
                    .general
                    .rows
                    .get(&(id as i32))?
                    .type_slot
            }
        };
        Some(crate::game::general::general_type_catalog().key(slot))
    }
}

/// One client tick of the rows: `setOldPosAndRot`, then a step of the non-living
/// `InterpolationHandler` (`Entity.moveOrInterpolateTo`, 3 steps) toward the server's last move.
pub(in crate::game) fn tick_remote_general(
    rows: &mut IndexMap<i32, RemoteGeneral>,
    lerps: &mut HashMap<i32, Lerp>,
) {
    for (id, row) in rows.iter_mut() {
        row.previous = row.position;
        row.previous_rotation = row.rotation;
        row.age = row.age.wrapping_add(1);
        let Some(l) = lerps.get_mut(id).filter(|l| l.steps > 0) else {
            continue;
        };
        let n = f64::from(l.steps);
        for k in 0..3 {
            row.position[k] += (l.target[k] - row.position[k]) / n;
        }
        if let Some((yaw, pitch)) = l.rotation {
            row.rotation[0] =
                crate::game::remote_objects::lerp_yaw_step(row.rotation[0], yaw, l.steps);
            row.rotation[1] += (pitch - row.rotation[1]) / l.steps as f32;
        }
        l.steps -= 1;
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;
    use sunrise_mod_runtime::attachments::Holder;
    use sunrise_protocol::packets::play::{self, Clientbound as C};

    pub(in crate::game) static FUEL: mod_api::AttachmentType<i32> = mod_api::AttachmentType {
        namespace: "mymod",
        name: "fuel",
        default: || 0,
        codec: Some(mod_api::AttachmentCodec {
            encode: |n| Ok([("n".into(), sunrise_core::nbt::Tag::Int(*n))].into()),
            decode: |c| match c.get(&"n".into()) {
                Some(sunrise_core::nbt::Tag::Int(n)) => Ok(*n),
                _ => Err(mod_api::AttachmentError::Decode),
            },
        }),
        copy_on_death: false,
        sync: true,
    };

    #[test]
    fn a_move_is_reached_in_three_ticks_and_each_tick_keeps_the_one_before() {
        let row = RemoteGeneral::new(
            0,
            1,
            [0.0, 70.0, 0.0],
            [0.0, 0.0],
            mod_api::EntityRenderAttachments::default(),
        );
        let mut rows = IndexMap::from([(5, row)]);
        let mut lerps = HashMap::from([(
            5,
            Lerp {
                target: [3.0, 70.0, 0.0],
                rotation: Some((90.0, 30.0)),
                steps: 3,
            },
        )]);
        let mut seen = Vec::new();
        for _ in 0..4 {
            tick_remote_general(&mut rows, &mut lerps);
            let r = &rows[&5];
            seen.push((r.previous[0], r.position[0], r.rotation, r.age));
        }
        assert_eq!(seen[0], (0.0, 1.0, [30.0, 10.0], 1));
        assert_eq!(seen[1], (1.0, 2.0, [60.0, 20.0], 2));
        assert_eq!(seen[2], (2.0, 3.0, [90.0, 30.0], 3));
        // Reached: it stays, and only ages.
        assert_eq!(seen[3], (3.0, 3.0, [90.0, 30.0], 4));
    }
    pub(in crate::game) struct Declare;
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Declare {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            let types = [
                ("missile", 0.5, Some([1.0, 1.25, 0.0]), false),
                ("package", 2.0, None, true),
            ];
            for (name, height, passenger_attachment, pickable) in types {
                r.declare_general_entity(
                    name,
                    mod_api::GeneralEntityCategory::Misc,
                    0.5,
                    height,
                    passenger_attachment,
                    mod_api::GeneralSavePolicy::Save,
                    true,
                    pickable,
                    (!pickable).then_some(mod_api::PushReaction::IgnoreEntity),
                    8,
                    1,
                    1,
                    &[],
                )
                .unwrap();
            }
        }
    }

    pub(in crate::game) fn spawn(id: i32, key: &str, at: [f64; 3]) -> C {
        spawn_facing(id, key, at, 0.0)
    }

    pub(in crate::game) fn spawn_facing(id: i32, key: &str, at: [f64; 3], yaw: f32) -> C {
        C::CustomPayload(sunrise_protocol::packets::configuration::CustomPayload {
            channel: general_entity::CHANNEL.into(),
            data: general_entity::encode(&Message::Spawn {
                id,
                uuid: 0x77,
                type_key: key.into(),
                position: at,
                rotation: [yaw, 0.0],
                velocity: [0.0; 3],
            })
            .unwrap(),
        })
    }

    pub(in crate::game) fn attachment_message(
        id: u32,
        replace: bool,
        value: Option<i32>,
    ) -> sunrise_protocol::attachment_sync::Message {
        use sunrise_core::nbt::Tag;
        use sunrise_mod_runtime::attachments::AttachmentSync;
        let changes = value.map_or_else(Vec::new, |n| {
            vec![AttachmentSync {
                namespace: "mymod".into(),
                name: "fuel".into(),
                value: Some([("n".into(), Tag::Int(n))].into()),
            }]
        });
        sunrise_protocol::attachment_sync::Message {
            holder: Holder::Entity(id),
            replace,
            changes,
        }
    }

    /// The ids of the rows present on the window.
    pub(in crate::game) fn row_ids(game: &crate::game::ClientGame<'_>) -> Vec<i32> {
        let mut ids: Vec<i32> = game
            .remote
            .as_ref()
            .unwrap()
            .general
            .rows
            .keys()
            .copied()
            .collect();
        ids.sort_unstable();
        ids
    }

    pub(in crate::game) fn rows(
        game: &crate::game::ClientGame<'_>,
    ) -> IndexMap<i32, RemoteGeneral> {
        game.remote.as_ref().unwrap().general.rows.clone()
    }

    #[test]
    #[ignore = "daily"]
    fn riding_reads_follow_the_server_passenger_lists() {
        use crate::game::native_entity_refs::tests::run_fresh_cases;
        use mod_api::{ClientHost, EntityRef, WorldEpoch};

        if run_fresh_cases(
            "game::remote::general::tests::riding_reads_follow_the_server_passenger_lists",
            "SUNRISE_CLIENT_RIDING_TEST_CHILD",
            &["riding"],
        ) {
            return;
        }
        mod_platform::__private::initialize().unwrap();
        let _phase = mod_platform::__private::RegistrationGuard::open();
        let declare = || vec![mod_api::ModEntry::new("mymod", Box::new(Declare))];
        mod_platform::testing::with_mods(declare().into(), || {
            mod_platform::__private::part_tests::install_parts_fixture(&declare()).unwrap();
            crate::game::item_entities::thrown_items_tests::with_remote_game(|game| {
                let level = || play::SpawnInfo {
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
                };
                let position = |teleport| {
                    C::PlayerPosition(play::PlayerPosition {
                        teleport,
                        position: [0.5, 70.0, 0.5],
                        delta: [0.0; 3],
                        yaw: 0.0,
                        pitch: 0.0,
                        relatives: 0,
                    })
                };
                game.handle_packet(C::Login(play::Login {
                    player_id: 7,
                    hardcore: false,
                    levels: vec!["minecraft:overworld".into()],
                    max_players: 20,
                    chunk_radius: 8,
                    simulation_distance: 8,
                    reduced_debug_info: false,
                    show_death_screen: true,
                    limited_crafting: false,
                    spawn: level(),
                    online_mode: false,
                    enforces_secure_chat: false,
                }))
                .unwrap();
                game.handle_packet(position(1)).unwrap();
                game.handle_packet(spawn_facing(9, "mymod:missile", [2.5, 70.0, 0.5], 90.0))
                    .unwrap();
                game.handle_packet(spawn(12, "mymod:missile", [0.5, 70.0, 0.5]))
                    .unwrap();
                game.spawn_entity(4, "minecraft:area_effect_cloud", [0.5, 70.0, 0.5], [0.0; 3]);
                game.spawn_entity(5, "minecraft:marker", [0.5, 70.0, 0.5], [0.0; 3]);
                game.spawn_entity(
                    6,
                    "minecraft:ominous_item_spawner",
                    [0.5, 70.0, 0.5],
                    [0.0; 3],
                );
                let set = |vehicle, passengers| {
                    C::SetPassengers(play::SetPassengers {
                        vehicle,
                        passengers,
                    })
                };
                let vehicle = game.entity(9).unwrap();
                let row = game.entity(12).unwrap();
                let cloud = game.entity(4).unwrap();
                let me = game.entity(7).unwrap();
                let unknown = EntityRef::new(vehicle.epoch(), 99);
                let stale = EntityRef::new(
                    WorldEpoch::new(vehicle.epoch().get().wrapping_add(1)),
                    vehicle.id(),
                );
                game.handle_packet(set(9, vec![99])).unwrap();
                let host: &dyn ClientHost = game;
                assert_eq!(vehicle.passengers(host), Some(vec![]));
                assert_eq!(vehicle.is_vehicle(host), Some(false));

                game.handle_packet(set(9, vec![12, 99, 7])).unwrap();
                game.handle_packet(set(12, vec![4])).unwrap();
                game.position_riders();
                assert_eq!(row.position(game), Some([2.5, 71.25, 1.5]));
                let host: &dyn ClientHost = game;
                assert_eq!(vehicle.passengers(host), Some(vec![row, me]));
                assert_eq!(vehicle.is_vehicle(host), Some(true));
                assert_eq!(row.vehicle(host), Some(vehicle));
                assert_eq!(me.vehicle(host), Some(vehicle));
                assert_eq!(cloud.is_passenger(host), Some(true));
                assert_eq!(cloud.root_vehicle(host), Some(vehicle));
                assert_eq!(unknown.vehicle(host), None);
                assert_eq!(unknown.passengers(host), None);
                assert_eq!(stale.passengers(host), None);
                assert_eq!(stale.is_vehicle(host), None);

                // An unknown vehicle can't displace a real relationship. Later tracking
                // doesn't apply either of its old packets or its mention as a passenger.
                game.handle_packet(set(99, vec![12])).unwrap();
                assert_eq!(row.vehicle(game), Some(vehicle));
                game.spawn_entity(99, "minecraft:marker", [0.5, 70.0, 0.5], [0.0; 3]);
                let late = game.entity(99).unwrap();
                assert_eq!(late.is_passenger(game), Some(false));
                assert_eq!(late.is_vehicle(game), Some(false));
                assert_eq!(vehicle.passengers(game), Some(vec![row, me]));

                game.handle_packet(set(4, vec![9])).unwrap();
                assert_eq!(vehicle.vehicle(game), None);
                assert_eq!(cloud.is_vehicle(game), Some(false));
                assert_eq!(cloud.root_vehicle(game), Some(vehicle));

                // Known entities that cannot accept a passenger do not displace
                // an existing ride either, even on a forced client mount.
                for refused in [5, 6] {
                    let cannot_carry = game.entity(refused).unwrap();
                    game.handle_packet(set(refused as i32, vec![12])).unwrap();
                    assert_eq!(cannot_carry.passengers(game), Some(vec![]));
                    assert_eq!(row.vehicle(game), Some(vehicle));
                }

                game.spawn_entity(20, "minecraft:happy_ghast", [2.5, 70.0, 0.5], [0.0; 3]);
                let ghast = game.entity(20).unwrap();
                game.handle_packet(set(20, vec![7, 12])).unwrap();
                game.position_riders();
                assert_eq!(row.position(game), Some([2.5 - 1.7, 74.0, 0.5]));
                game.handle_packet(set(9, vec![12])).unwrap();
                assert_eq!(me.vehicle(game), Some(ghast));
                game.handle_packet(set(7, vec![99])).unwrap();
                assert_eq!(late.vehicle(game), Some(me));
                // Replace the local player in the same level while it is both a rider and
                // a vehicle. A death never plays the ghast's voluntary dismount.
                game.handle_packet(C::Respawn(play::Respawn {
                    spawn: level(),
                    keep: 0,
                }))
                .unwrap();
                assert_eq!(game.remote.as_ref().unwrap().ghast_dismount, None);
                game.handle_packet(position(2)).unwrap();
                assert_eq!(game.entity(7), Some(me));
                assert_eq!(ghast.position(game), Some([2.5, 70.0, 0.5]));
                assert_eq!(
                    game.mobs
                        .iter()
                        .find(|mob| mob.id == 20)
                        .unwrap()
                        .extra
                        .counter,
                    0
                );
                let host: &dyn ClientHost = game;
                assert_eq!(me.is_passenger(host), Some(false));
                assert_eq!(me.passengers(host), Some(vec![]));
                assert_eq!(late.is_passenger(host), Some(false));
                assert_eq!(vehicle.passengers(host), Some(vec![row]));
                assert_eq!(cloud.root_vehicle(host), Some(vehicle));

                game.handle_packet(set(9, vec![])).unwrap();
                assert_eq!(vehicle.passengers(game), Some(vec![]));
                assert_eq!(cloud.root_vehicle(game), Some(row));
            })
        });
    }

    /// The window's side of a general row: a spawn adds it immediately in the client's catalog
    /// slot with empty `EntityRenderAttachments`, an unknown key and a second spawn of one id
    /// change nothing, attachment syncs for unknown entities are ignored while updates for known
    /// rows publish immediately regardless of `replace`, a position sync is reached over three
    /// ticks with nothing of the mod's run here, and a removal drops it with its synced
    /// attachments.
    #[test]
    #[ignore = "daily"]
    fn the_window_keeps_moves_and_drops_the_rows_it_is_shown() {
        mod_platform::__private::initialize().unwrap();
        let _phase = mod_platform::__private::RegistrationGuard::open();
        let declare = || vec![mod_api::ModEntry::new("mymod", Box::new(Declare))];
        mod_platform::testing::with_mods(declare().into(), || {
            mod_platform::__private::part_tests::install_parts_fixture(&declare()).unwrap();
            crate::game::item_entities::thrown_items_tests::with_remote_game(|game| {
                // An attachment sync for an unknown entity is ignored without storing anything.
                game.remote_attachment_sync(attachment_message(5, false, Some(99)));
                assert!(
                    !game
                        .remote
                        .as_ref()
                        .unwrap()
                        .attachments
                        .contains_key(&Holder::Entity(5))
                );
                game.handle_packet(spawn(5, "mymod:missile", [0.5, 70.0, 0.5]))
                    .unwrap();
                game.handle_packet(spawn(6, "other:unknown", [0.5, 70.0, 0.5]))
                    .unwrap();
                game.handle_packet(spawn(5, "mymod:missile", [9.5, 70.0, 0.5]))
                    .unwrap();
                let held = rows(game);
                assert_eq!(held.keys().collect::<Vec<_>>(), [&5]);
                assert_eq!(held[&5].position, [0.5, 70.0, 0.5]);
                assert_eq!(held[&5].uuid, 0x77);
                // Immediately visible with empty render attachments even without attachment_sync.
                assert_eq!(
                    held[&5].render_attachments,
                    mod_api::EntityRenderAttachments::default()
                );
                assert_eq!(held[&5].render_attachments.get(&FUEL), Ok(None));
                // Both deltas and whole snapshots publish to render_attachments for a known row.
                game.remote_attachment_sync(attachment_message(5, false, Some(12)));
                assert_eq!(rows(game)[&5].render_attachments.get(&FUEL), Ok(Some(&12)));
                game.remote_attachment_sync(attachment_message(5, true, Some(9)));
                assert_eq!(row_ids(game), [5]);
                assert_eq!(rows(game)[&5].render_attachments.get(&FUEL), Ok(Some(&9)));
                let fuel = |game: &crate::game::ClientGame<'_>, id: u32| {
                    let held = game.remote.as_ref().unwrap();
                    let map = held.attachments.get(&Holder::Entity(id))?;
                    let value = map
                        .synced()
                        .into_iter()
                        .find(|entry| {
                            entry.namespace == "mymod".into() && entry.name == "fuel".into()
                        })?
                        .value?;
                    match value.get(&"n".into()) {
                        Some(sunrise_core::nbt::Tag::Int(n)) => Some(*n),
                        _ => None,
                    }
                };
                assert_eq!(fuel(game, 5), Some(9));
                // A second row spawned with no attachment_sync at all is immediately visible with
                // empty render attachments.
                game.handle_packet(spawn(8, "mymod:missile", [0.5, 70.0, 0.5]))
                    .unwrap();
                assert_eq!(row_ids(game), [5, 8]);
                assert_eq!(
                    rows(game)[&8].render_attachments,
                    mod_api::EntityRenderAttachments::default()
                );
                game.handle_packet(C::EntityPositionSync(play::EntityPositionSync {
                    id: 5,
                    path: play::PositionPath::Linear([3.5, 70.0, 0.5]),
                    yaw: 0.0,
                    pitch: 0.0,
                    on_ground: false,
                }))
                .unwrap();
                let mut xs = Vec::new();
                for _ in 0..3 {
                    game.tick_remote_entities(true);
                    xs.push(rows(game)[&5].position[0]);
                }
                assert_eq!(xs, [1.5, 2.5, 3.5]);
                assert_eq!(rows(game)[&5].previous[0], 2.5);
                // A client holds rows only here: none is a resident row, ticked by mod code.
                assert!(game.resident_entities.general.rows.is_empty());
                let holder = Holder::Entity(5);
                game.remote
                    .as_mut()
                    .unwrap()
                    .attachments
                    .insert(holder, Default::default());
                game.handle_packet(C::RemoveEntities(play::RemoveEntities { ids: vec![5, 8] }))
                    .unwrap();
                assert!(rows(game).is_empty());
                assert!(
                    !game
                        .remote
                        .as_ref()
                        .unwrap()
                        .attachments
                        .contains_key(&holder)
                );
            })
        });
    }

    /// Matching vanilla `ServerEntity.addPairing` + `ClientPacketListener.handleBundlePacket` and
    /// NeoForge `AttachmentSync`: an atomic pairing bundle `[general_entity::Spawn, attachment_sync]`
    /// dispatches both packets together so the spawned row immediately holds its synced attachments.
    #[test]
    #[ignore = "daily"]
    fn atomic_pairing_bundle_spawns_general_row_with_synced_attachments() {
        use super::super::tests::{connect_play_test, offer_test_payloads, spawn_menu_test};
        use sunrise_client::net::Event;
        use sunrise_protocol::{
            attachment_sync,
            packets::{Empty, configuration::CustomPayload},
        };
        use sunrise_sim::raw_payloads;
        mod_platform::__private::initialize().unwrap();
        let _phase = mod_platform::__private::RegistrationGuard::open();
        let declare = || vec![mod_api::ModEntry::new("mymod", Box::new(Declare))];
        mod_platform::testing::with_mods(declare().into(), || {
            mod_platform::__private::part_tests::install_parts_fixture(&declare()).unwrap();
            crate::game::native_entity_refs::tests::with_client_game(|game| {
                let local = raw_payloads::Local::capture(raw_payloads::Side::Server).unwrap();
                let (_reader, mut writer) = connect_play_test(game, |reader, writer| {
                    offer_test_payloads(reader, writer, &local.manifest);
                });
                spawn_menu_test(game);
                let sync = C::CustomPayload(CustomPayload {
                    channel: attachment_sync::CHANNEL.into(),
                    data: attachment_sync::encode(&attachment_message(15, true, Some(42))).unwrap(),
                });
                for packet in [
                    C::BundleDelimiter(Empty),
                    spawn(15, "mymod:missile", [1.5, 70.0, 2.5]),
                    sync,
                    C::BundleDelimiter(Empty),
                ] {
                    writer.send(&packet.encode()).unwrap();
                }
                game.wait_for_test_remote_event(|event| matches!(event, Event::Bundle(_)));
                game.poll_remote_events(true).unwrap();

                let held = rows(game);
                assert_eq!(held.keys().collect::<Vec<_>>(), [&15]);
                assert_eq!(held[&15].position, [1.5, 70.0, 2.5]);
                assert_eq!(held[&15].render_attachments.get(&FUEL), Ok(Some(&42)));
            })
        });
    }

    #[test]
    #[ignore = "daily"]
    fn general_spawn_captures_existing_entity_attachments_or_empty() {
        mod_platform::__private::initialize().unwrap();
        let _phase = mod_platform::__private::RegistrationGuard::open();
        let declare = || vec![mod_api::ModEntry::new("mymod", Box::new(Declare))];
        mod_platform::testing::with_mods(declare().into(), || {
            mod_platform::__private::part_tests::install_parts_fixture(&declare()).unwrap();
            crate::game::item_entities::thrown_items_tests::with_remote_game(|game| {
                // Path 1: no prior Holder::Entity entry -> empty initial snapshot.
                game.handle_packet(spawn(21, "mymod:missile", [0.0, 70.0, 0.0]))
                    .unwrap();
                assert_eq!(rows(game)[&21].render_attachments.get(&FUEL), Ok(None));

                // Path 2: existing Holder::Entity map present before spawn -> captured on spawn.
                let mut existing = sunrise_mod_runtime::attachments::ResidentAttachments::default();
                existing.apply_sync(sunrise_mod_runtime::attachments::AttachmentSync {
                    namespace: "mymod".into(),
                    name: "fuel".into(),
                    value: Some([("n".into(), sunrise_core::nbt::Tag::Int(77))].into()),
                });
                game.remote
                    .as_mut()
                    .unwrap()
                    .attachments
                    .insert(Holder::Entity(22), existing);
                game.handle_packet(spawn(22, "mymod:missile", [1.0, 70.0, 0.0]))
                    .unwrap();
                assert_eq!(rows(game)[&22].render_attachments.get(&FUEL), Ok(Some(&77)));
            })
        });
    }
}
