#[allow(unused_imports)]
pub(in crate::game::client_side) use super::stats;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::villager_client::unhappy_counter;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game,
    native_living_effects::{active_effect, mob_effect},
    shield::OFFHAND_SLOT,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::api_values::TryIntoApi;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::HELP;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::LanHost;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::LanSettings;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::Owner;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::baby_index;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::client_info;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::event_timings::{
    EventTimings, UseDetail, describe_burst, event_kind, note_event,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::living_flags;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::mob_freeze_meta;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::mob_meta;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::mob_raider_meta;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::*;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::client_side) use level_event_targets::level_event_except;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{collections::HashMap, sync::Arc, time::Instant};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    effects::{
        EffectInstance, ModifiedEffectParticles, ShownEffect, effect_particle,
        visible_effect_particles,
    },
    lan::{
        self, NetConfig,
        chunks::ChunkEncoder,
        configuration_tasks::{Configured, PreparedSpawn},
        data::{BIOME_REGISTRY, SyncData},
        entity::{EntityState, States, avatar_main_arm_meta},
        hub::{HostEvent, Hub, HubConfig, TickCtx, stuck_meta},
        playerdata,
    },
    mobs::{Mob, Species, Who, leash::Holder},
    text::Text,
    world::World,
};
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::client_side) use sunrise_gameplay::lan::level_event_targets;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::{
    entity_data::{Particles, Value, serializer},
    packets::play::{self, Clientbound as C},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::block_collision::BlockCollisionRegistry;

impl Game<'_> {
    /// `sunrise-ctl lan ...`.
    pub(in crate::game::client_side) fn lan_control(
        &mut self,
        words: &[&str],
    ) -> Result<Option<serde_json::Value>> {
        use serde_json::json;
        if words[0] != "lan" {
            return Ok(None);
        }
        if let Some(server) = &self.integrated
            && words.get(1) == Some(&"status")
        {
            // The hub is the server thread's.
            let status = server
                .ask(super::owner_link::OwnerRequest::LanStatus)
                .ok_or(super::integrated::NO_ANSWER)?;
            return Ok(Some(status));
        }
        match words.get(1).copied() {
            Some("open") => {
                if self.title.0.is_some() {
                    return Err("open a world first".into());
                }
                let port = words.get(2).map_or(Ok(0), |p| p.parse::<u16>())?;
                self.lan_open(LanSettings {
                    port: if port == 0 {
                        lan::available_port()
                    } else {
                        port
                    },
                    guest_commands: words.contains(&"commands"),
                    force_game_mode: words.contains(&"force"),
                })?;
                Ok(Some(json!({ "ok": true, "port": self.lan_port() })))
            }
            Some("close") => {
                self.lan_close(true);
                Ok(Some(json!({ "ok": true })))
            }
            Some("status") => Ok(Some(self.lan_status())),
            _ => Err("lan open|close|status|op".into()),
        }
    }
    pub(in crate::game) fn lan_port(&self) -> Option<u16> {
        if let Some(server) = &self.integrated {
            return server.published();
        }
        if self.serving() {
            return self.published_port();
        }
        self.lan.as_ref().map(|h| h.settings.port)
    }
}

#[cfg(test)]
pub(in crate::game) mod merchant_packet_tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::lan::merchant_packet_tests::ReplaceOnTradeVoice;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::lan::merchant_packet_tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use mod_api::HostTransaction;

    #[test]
    #[ignore = "daily"]
    fn merchant_take_keeps_packet_snapshot_until_price_refresh() {
        use crate::game::native_entity_refs::tests::{
            bind_local_player, run_fresh_cases, with_game,
        };
        use std::sync::{Arc, atomic::AtomicI32, mpsc};
        use sunrise_client::{
            merchant::TradeFeedback,
            mobs::{Damage, Mob},
        };
        use sunrise_core::{
            geometry::Vec3,
            random::{LegacyRandom, RandomSource},
        };
        use sunrise_protocol::packets::play::Clientbound;

        const CHILD: &str = "SUNRISE_MERCHANT_HURT_PACKETS_CHILD";
        if run_fresh_cases(
            "game::client_side::lan_client::merchant_packet_tests::merchant_take_keeps_packet_snapshot_until_price_refresh",
            CHILD,
            &[
                "nonbuyer-first",
                "buyer-first",
                "sound-replaces-menu",
                "sound-replaces-suspended-menu",
                "sound-replaces-suspended-menu-abort",
            ],
        ) {
            return;
        }
        let case = std::env::var(CHILD).unwrap();
        let buyer_first = case == "buyer-first";
        use sunrise_client::lan::{Joined, NetConfig, PlayerSender, guest::Guest};

        with_game(|game| {
            game.build_registries();
            game.set_allow_commands(true);

            let mut sync = sunrise_client::lan::data::SyncData::empty();
            for (registry, names) in [
                (
                    "minecraft:menu",
                    vec![
                        "minecraft:merchant".to_owned(),
                        "minecraft:generic_9x1".to_owned(),
                    ],
                ),
                (
                    "minecraft:entity_type",
                    vec!["minecraft:villager".to_owned()],
                ),
                (
                    "minecraft:damage_type",
                    vec!["minecraft:player_attack".to_owned()],
                ),
                (
                    "minecraft:item",
                    vec![
                        "minecraft:air".to_owned(),
                        "minecraft:emerald".to_owned(),
                        "minecraft:glass".to_owned(),
                        "minecraft:rabbit_foot".to_owned(),
                        "minecraft:glowstone".to_owned(),
                    ],
                ),
            ] {
                sync.builtin.registries.insert(
                    registry.to_owned(),
                    sunrise_protocol::registry::Ids::from_names(names),
                );
            }
            let sync = Arc::new(sync);
            let net = NetConfig {
                listen: false,
                announce: false,
                ..NetConfig::loopback(0, String::new(), 2)
            };
            let lan = sunrise_client::lan::Lan::start_with(sync.clone(), net).unwrap();
            let encoder = sunrise_client::lan::chunks::ChunkEncoder::new(
                [] as [(Arc<str>, bool); 0],
                &sunrise_protocol::registry::Builtin::default(),
                &[],
                &[],
                Default::default(),
            );
            let config = sunrise_client::lan::hub::HubConfig {
                default_game_mode: 0,
                guest_commands: false,
                force_game_mode: false,
                hardcore: false,
                view_distance: 10,
                simulation_distance: 10,
                dimension: "minecraft:overworld".into(),
                allow_flight: false,
                seed: 0,
                max_players: 20,
                integrated: true,
                levels: Default::default(),
                level_seeds: Default::default(),
            };
            let mut hub = sunrise_client::lan::hub::Hub::new(
                lan,
                sync,
                encoder,
                config,
                game.loop_context.clone(),
            );
            let (tx, rx) = mpsc::channel();
            let mut guest = Guest::new(
                Joined {
                    conn: 1,
                    uuid: 2,
                    name: "Test".into(),
                    properties: Vec::new(),
                    info: sunrise_protocol::packets::configuration::ClientInformation::new(8),
                    brand: String::new(),
                    sender: PlayerSender::new(tx, Arc::default()),
                    latency: Arc::new(AtomicI32::new(0)),
                    address: "127.0.0.1:1".parse().unwrap(),
                    expected: false,
                },
                1,
                [0.5, 4.0, 0.5],
                8,
            );
            guest.placed = true;
            guest.dimension = "minecraft:overworld".into();
            guest.sent_chunks.insert((0, 0));
            guest.inventory[0] = Some(sunrise_client::items::ItemStack::new(
                "minecraft:emerald",
                8,
            ));
            hub.guests.push(guest);
            game.lan = Some(super::LanHost {
                hub,
                spawn_joins: Vec::new(),
                players_told_left: false,
                settings: super::LanSettings {
                    port: 0,
                    guest_commands: false,
                    force_game_mode: false,
                },
                play: Default::default(),
                menus: Default::default(),
                broken: Default::default(),
                mob_state: Default::default(),
                poses: Default::default(),
                mob_players: Default::default(),
                owner: None,
                waypoints: Default::default(),
                waypoint_dead: Default::default(),
                border_sent: None,
                commands: Default::default(),
                quiet: Default::default(),
                far: None,
                native_commands_sent: None,
                sleep_ignored: Default::default(),
                states: Default::default(),
                states_next_entity: None,
                idle_states: Default::default(),
            });

            use crate::game::trading::MerchantState;
            use sunrise_client::{
                items::ItemStack,
                merchant::{ItemCost, MerchantOffer, Session},
            };
            let offer = MerchantOffer {
                base_cost_a: ItemCost {
                    stack: ItemStack::new("minecraft:emerald", 4),
                },
                cost_b: None,
                result: ItemStack::new("minecraft:glass", 3),
                uses: 0,
                max_uses: 100,
                reward_exp: false,
                special_price_diff: 0,
                demand: 0,
                price_multiplier: 0.0,
                xp: 1,
            };
            let mut session = Session::new(vec![offer], true);
            session.xp = 20;
            session.level = 2;
            let kind = game
                .res
                .mob_types()
                .get("minecraft:villager")
                .unwrap()
                .clone();
            let mut mob = sunrise_client::mobs::Mob::new(
                77,
                kind,
                sunrise_core::geometry::Vec3::new(0.5, 4.0, 2.5),
                0.0,
            );
            mob.extra.trade = Some(Box::new(sunrise_client::merchant::Trader {
                xp: 20,
                offers: session.offers.clone(),
                ..Default::default()
            }));
            game.mobs.push(mob);
            game.lan_open_merchant(
                1,
                MerchantState {
                    session: Box::new(session),
                    scroll: 0,
                    dragging: false,
                    focused: None,
                    mob: Some(77),
                    name: Some(crate::text::Text::literal("Packet timing")),
                },
            );
            fn packets(
                rx: &std::sync::mpsc::Receiver<sunrise_client::lan::Out>,
            ) -> (
                Vec<sunrise_protocol::packets::play::MerchantOffers>,
                Vec<sunrise_protocol::packets::play::ContainerSetContent>,
            ) {
                let mut offers = Vec::new();
                let mut contents = Vec::new();
                for out in rx.try_iter() {
                    let sunrise_client::lan::Out::Packet(body) = out else {
                        continue;
                    };
                    let mut reader = sunrise_protocol::buf::Reader::new(&body);
                    let id = reader.varint().unwrap();
                    match Clientbound::decode(id, reader.rest()).unwrap() {
                        sunrise_protocol::packets::play::Clientbound::MerchantOffers(packet) => {
                            offers.push(packet)
                        }
                        sunrise_protocol::packets::play::Clientbound::ContainerSetContent(
                            packet,
                        ) => contents.push(packet),
                        _ => {}
                    }
                }
                (offers, contents)
            }
            let initial = packets(&rx).0;
            assert_eq!(initial.len(), 1);
            assert_eq!(initial[0].offers[0].uses, 0);
            game.lan_select_trade(1, 0);
            assert!(
                packets(&rx).0.is_empty(),
                "selecting does not resend offers"
            );
            let window = initial[0].container;
            // Real ContainerClick on the merchant result slot, through the same
            // native path as the parity harness. Vanilla notifyTrade increments
            // uses/XP but sends no offers until the later reputation AI phase.
            let click = sunrise_protocol::packets::play::ContainerClick {
                container: window,
                state: 0,
                slot: 2,
                button: 0,
                input: 0,
                changed: Vec::new(),
                carried: None,
            };
            assert!(game.lan_merchant_click(1, &click));
            let state = &game.lan.as_ref().unwrap().menus[&1]
                .merchant
                .as_ref()
                .unwrap()
                .merchant
                .session;
            assert_eq!(state.offers[0].uses, 1);
            assert_eq!(state.xp, 21);
            let (offers, contents) = packets(&rx);
            assert!(
                offers.is_empty(),
                "a take leaves the last offer packet untouched"
            );
            let content = contents.last().expect("take sends real inventory contents");
            assert_eq!(content.items[0].as_ref().unwrap().count, 4);
            assert_eq!(content.items[2].as_ref().unwrap().count, 3);
            assert_eq!(content.carried.as_ref().unwrap().count, 3);
            let second = sunrise_protocol::packets::play::ContainerClick {
                state: content.state,
                ..click
            };
            assert!(game.lan_merchant_click(1, &second));
            let (offers, contents) = packets(&rx);
            assert!(offers.is_empty(), "the second take also keeps the snapshot");
            let content = contents.last().expect("second take sends contents");
            assert!(content.items[0].is_none());
            assert!(content.items[2].is_none());
            assert_eq!(content.carried.as_ref().unwrap().count, 6);
            let state = &game.lan.as_ref().unwrap().menus[&1]
                .merchant
                .as_ref()
                .unwrap()
                .merchant
                .session;
            assert_eq!(state.offers[0].uses, 2);
            assert_eq!(state.xp, 22);
            // An actual price refresh sends the snapshot even if only uses/XP
            // changed, and even a repeated refresh must reach the client.
            game.lan_merchant_reputation(0, None, TradeFeedback::Current);
            assert!(
                packets(&rx).0.is_empty(),
                "unrelated merchants do not refresh this menu"
            );
            game.lan_merchant_reputation(77, None, TradeFeedback::Current);
            let refreshed = packets(&rx).0;
            assert_eq!(refreshed.len(), 1);
            assert_eq!(refreshed[0].offers[0].uses, 2);
            assert_eq!(refreshed[0].xp, 22);
            game.lan_merchant_reputation(77, None, TradeFeedback::Current);
            assert_eq!(
                packets(&rx).0.len(),
                1,
                "price refresh sends even an unchanged offer"
            );
            assert!(game.lan_merchant_closed(1, Some(window)));
            assert!(packets(&rx).0.is_empty(), "closing does not resend offers");

            // Crossing the real level-two boundary runs updateSpecialPrices while
            // taking the result, so the new level's offers must reach the guest.
            let mut session = Session::new(
                game.mobs[0].extra.trade.as_ref().unwrap().offers.clone(),
                true,
            );
            session.offers[0].uses = 0;
            session.xp = 69;
            session.level = 2;
            session.profession = "minecraft:cleric".into();
            let sequences = Session::sequences_needed(
                &game.stations().trades,
                &session.profession,
                session.level,
            );
            assert!(!sequences.is_empty(), "prepared cleric trade sets exist");
            for id in sequences {
                let random = sunrise_client::merchant::sequence_random(0, &id);
                session.sequences.insert(id, random);
            }
            game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                g.inventory[0] = Some(ItemStack::new("minecraft:emerald", 8));
                g.carried = None;
            });
            game.lan_open_merchant(
                1,
                MerchantState {
                    session: Box::new(session),
                    scroll: 0,
                    dragging: false,
                    focused: None,
                    mob: Some(77),
                    name: Some(crate::text::Text::literal("Level-up packet timing")),
                },
            );
            let opened = packets(&rx).0;
            assert_eq!(opened.len(), 1);
            game.lan_select_trade(1, 0);
            assert!(packets(&rx).0.is_empty());
            let click = sunrise_protocol::packets::play::ContainerClick {
                container: opened[0].container,
                state: 0,
                ..second
            };
            assert!(game.lan_merchant_click(1, &click));
            let (offers, contents) = packets(&rx);
            assert_eq!(offers.len(), 1, "level-up price refresh sends new offers");
            assert_eq!(offers[0].level, 3);
            assert_eq!(offers[0].xp, 70);
            assert_eq!(offers[0].offers[0].uses, 1);
            assert!(offers[0].offers.len() > opened[0].offers.len());
            let content = contents.last().expect("level-up take sends contents");
            assert_eq!(content.items[0].as_ref().unwrap().count, 4);
            assert_eq!(content.carried.as_ref().unwrap().count, 3);

            // `setLastHurtByMob` refreshes the matching buyer before event 13 and the
            // damage packet. Another player gets anger but no offer; mobs and lethal
            // blows show no anger. Keep the victim's same-tick cooldown throughout.
            let gm = game
                .lan
                .as_mut()
                .unwrap()
                .menus
                .get_mut(&1)
                .unwrap()
                .merchant
                .as_mut()
                .unwrap();
            assert!(
                gm.slots[2].is_some(),
                "the buyer could still afford the old price"
            );
            assert_eq!(gm.slots[0].as_ref().unwrap().count, 4);
            gm.merchant.session.offers[0].price_multiplier = 0.2;
            let merchant_window = gm.window;
            game.audio.0.capture = Some(Vec::new());
            let mut states = super::States::default();
            game.lan_entities(&mut states, false, 0, None);
            let hub = &mut game.lan.as_mut().unwrap().hub;
            hub.note_joined(states.as_slice(), None);
            assert!(hub.guests[0].tracker.is_tracking(77));
            rx.try_iter().for_each(drop);

            let at = [0.5, 4.0, 0.5];
            game.player = Some(
                sunrise_client::motion::PlayerMotion::new(
                    game.motion,
                    Vec3::new(at[0], at[1], at[2]),
                )
                .unwrap(),
            );
            bind_local_player(game, 300, 3);
            game.mobs[0].ambient_timer = 0;
            game.mobs[0].random = LegacyRandom::new(42);
            let mut random = LegacyRandom::new(42);
            let pitch = (random.next_float() - random.next_float()) * 0.2 + 1.0;
            if buyer_first {
                // The yes/no voice consumes the first pitch pair, then hurt consumes the second.
                random.next_float();
                random.next_float();
            }
            let record = || {
                let mut actual = Vec::new();
                for out in rx.try_iter() {
                    let sunrise_client::lan::Out::Packet(body) = out else {
                        continue;
                    };
                    let mut reader = sunrise_protocol::buf::Reader::new(&body);
                    let id = reader.varint().unwrap();
                    match Clientbound::decode(id, reader.rest()).unwrap() {
                        Clientbound::MerchantOffers(packet) => {
                            let offer = &packet.offers[0];
                            actual.push(format!(
                                "offer:{}",
                                offer.cost_a.count + offer.special_price
                            ));
                        }
                        Clientbound::EntityEvent(packet)
                            if packet.id == 77 && packet.event == 13 =>
                        {
                            actual.push("entity:13".to_owned());
                        }
                        Clientbound::DamageEvent(packet) if packet.id == 77 => {
                            actual.push("damage".to_owned());
                        }
                        Clientbound::Sound(packet)
                            if packet.sound.1.as_ref().is_some_and(|(name, _)| {
                                name == "minecraft:entity.villager.no"
                            }) =>
                        {
                            assert_eq!(packet.pitch.to_bits(), pitch.to_bits());
                            actual.push("no".to_owned());
                        }
                        Clientbound::Sound(packet)
                            if packet.sound.1.as_ref().is_some_and(|(name, _)| {
                                name == "minecraft:entity.villager.hurt"
                            }) =>
                        {
                            actual.push("sound".to_owned());
                        }
                        Clientbound::ContainerSetSlot(packet)
                            if packet.container == merchant_window =>
                        {
                            assert_eq!(packet.slot, 2, "only the result slot changed");
                            assert!(packet.item.is_none());
                            actual.push("result:empty".to_owned());
                        }
                        Clientbound::ContainerSetContent(packet)
                            if packet.container == merchant_window =>
                        {
                            panic!("a price refresh must not send another player's inventory");
                        }
                        _ => {}
                    }
                }
                actual
            };
            let other_hit = |game: &mut super::Game<'_>, amount| {
                assert_eq!(
                    game.hurt_by_player(0, amount, Damage::player(at)),
                    (true, false)
                );
            };
            let buyer_hit = |game: &mut super::Game<'_>, amount| {
                assert_eq!(
                    game.as_guest(1, |g| g.hurt_by_player(0, amount, Damage::player(at))),
                    Some((true, false))
                );
            };
            if case.starts_with("sound-replaces-") {
                let abort = case.ends_with("-abort");
                let mods = vec![mod_api::ModEntry::new(
                    "replace_on_trade_voice",
                    Box::new(ReplaceOnTradeVoice { abort }),
                )]
                .into();
                mod_platform::testing::with_mods(mods, || {
                    if case.starts_with("sound-replaces-suspended-menu") {
                        let mut inner = crate::game::native_item_places::tests::guest(2);
                        inner.dimension = "minecraft:overworld".into();
                        game.lan.as_mut().unwrap().hub.guests.push(inner);
                        game.as_guest(1, |game| {
                            game.as_guest(2, |game| {
                                // A player-owned event (such as a cloud) can refresh prices
                                // even when that player isn't the current actor.
                                sunrise_client::mobs::villager::ext_mut(&mut game.mobs[0])
                                    .gossips
                                    .add(
                                        [0, 0, 0, 2],
                                        sunrise_client::gossip::GossipType::MinorNegative,
                                        sunrise_client::gossip::REPUTATION_CHANGE_PER_EVENT,
                                    );
                                game.lan_merchant_reputation(
                                    77,
                                    Some([0, 0, 0, 2]),
                                    TradeFeedback::AtHurt(Some(1.0)),
                                );
                            })
                            .unwrap();
                        })
                        .unwrap();
                    } else {
                        buyer_hit(game, 1.0);
                    }
                });
                let host = game.lan.as_ref().unwrap();
                let buyer = host.hub.guest(1).unwrap();
                let emeralds = buyer
                    .inventory
                    .iter()
                    .flatten()
                    .filter(|item| item.item.as_ref() == "minecraft:emerald")
                    .map(|item| item.count)
                    .sum::<u32>();
                let (offers, contents) = packets(&rx);
                if abort {
                    assert!(
                        buyer.menu.is_none(),
                        "the aborted replacement did not remain open"
                    );
                    assert_eq!(
                        emeralds, 0,
                        "aborting did not duplicate the payment into inventory"
                    );
                    assert_eq!(
                        buyer.carried.as_ref().unwrap().count,
                        3,
                        "the old cursor was restored"
                    );
                    let gm = host.menus[&1]
                        .merchant
                        .as_ref()
                        .expect("merchant restored on abort");
                    assert_eq!(gm.window, merchant_window);
                    assert_eq!(gm.slots[0].as_ref().unwrap().count, 4);
                    assert!(
                        gm.slots[2].is_none(),
                        "the hit's earlier reprice still applies"
                    );
                    assert!(
                        contents.is_empty(),
                        "no tentative replacement snapshot escaped"
                    );
                    assert_eq!(
                        offers.len(),
                        1,
                        "the restored merchant window still receives its reprice"
                    );
                    assert_eq!(offers[0].container, merchant_window);
                    return;
                }
                let replacement = buyer.menu.as_ref().expect("sound hook opened a new window");
                assert_ne!(replacement.window, merchant_window);
                assert!(
                    host.menus[&1].merchant.is_none(),
                    "the old merchant window was retired"
                );
                assert_eq!(emeralds, 4, "the payment went back to the buyer");
                assert!(offers.is_empty(), "no offer packet for the retired window");
                let content = contents
                    .iter()
                    .find(|packet| packet.container == replacement.window)
                    .expect("replacement inventory snapshot");
                let emerald = host
                    .hub
                    .registry_id("minecraft:item", "minecraft:emerald")
                    .unwrap();
                assert_eq!(
                    content.items[9..]
                        .iter()
                        .flatten()
                        .filter(|item| item.item == emerald)
                        .map(|item| item.count)
                        .sum::<i32>(),
                    4
                );
                return;
            }
            // Use two accepted hits in one turn with no flush or tick between them.
            if buyer_first {
                buyer_hit(game, 1.0);
                other_hit(game, 3.0);
            } else {
                other_hit(game, 1.0);
                buyer_hit(game, 3.0);
            }
            assert_eq!(game.mobs[0].ambient_timer, -80);
            assert_eq!(game.mobs[0].random.clone().next_long(), random.next_long());
            let mut actual = record();
            assert!(!actual.iter().any(|event| event == "result:empty"));
            game.lan_flush_effects();
            assert!(
                record().is_empty(),
                "no hurt or slot packet waited for the effects flush"
            );
            game.lan_merchant_tick();
            let sync = record();
            assert_eq!(
                sync,
                ["result:empty"],
                "the result must reach the client at menu broadcast"
            );
            actual.extend(sync);
            game.lan_merchant_tick();
            assert!(record().is_empty(), "an unchanged result is not resent");
            let hub = &game.lan.as_ref().unwrap().hub;
            assert_eq!(hub.guests[0].carried.as_ref().unwrap().count, 3);
            let gm = game.lan.as_ref().unwrap().menus[&1]
                .merchant
                .as_ref()
                .unwrap();
            assert!(
                gm.slots[2].is_none(),
                "the buyer cannot afford the refreshed offer"
            );
            assert_eq!(
                gm.slots[0].as_ref().unwrap().count,
                4,
                "payment stayed in the menu"
            );
            let kind = game.mobs[0].kind.clone();
            game.mobs
                .push(Mob::new(78, kind.clone(), Vec3::new(1.0, 4.0, 1.0), 0.0));
            game.mobs
                .push(Mob::new(79, kind, Vec3::new(1.5, 4.0, 1.0), 0.0));
            game.native_mobs_joined(0, false);
            assert!(game.mob_hits_mob(77, 5.0, Damage::mob(78, at)));
            assert!(!game.mob_hits_mob(77, 2.0, Damage::mob(79, at)));
            assert!(game.mob_hits_mob(77, 7.0, Damage::mob(78, at)));
            bind_local_player(game, 300, 5);
            assert_eq!(
                game.hurt_by_player(0, 1000.0, Damage::player(at)),
                (true, true)
            );
            game.lan_flush_effects();
            actual.extend(record());
            let expected = match std::env::var_os("SUNRISE_REPUTATION_ORACLE") {
                Some(oracle) => std::fs::read_to_string(oracle)
                    .unwrap()
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix(if buyer_first {
                            "hurt_order_buyer "
                        } else {
                            "hurt_order "
                        })
                    })
                    .unwrap()
                    .to_owned(),
                None if buyer_first => {
                    "no,offer:9,entity:13,damage,sound,entity:13,result:empty".to_owned()
                }
                None => "entity:13,damage,sound,offer:9,entity:13,result:empty".to_owned(),
            };
            assert_eq!(actual.join(","), expected);
        });
    }
}

#[cfg(test)]
#[path = "tracking_hook_tests_client.rs"]
pub(in crate::game) mod tracking_hook_tests;
