//! First End exit is a PlayerList respawn; later exits are teleports of the same player.
use super::{
    Dim,
    credits_respawn_tests::{
        ANCHOR, charge_anchor, light_spawn_chunks, placed_guest, received_packets,
        server_with_a_nether_anchor,
    },
};
use crate::game::{Game, native_entity_refs::tests::run_with_game_stack};
use mod_api::{
    EntityTravelToDimensionEvent, Flow, Hooks, ModEntry, PlayerChangedDimensionEvent,
    PlayerCloneEvent, PlayerRespawnPositionEvent, PlayerRespawnedEvent, PlayerSetSpawnEvent,
    ServerHost,
};
use std::sync::{Arc, Mutex, mpsc};
use sunrise_client::{
    chunk::Chunk,
    lan::{Out, guest::InboxItem, hub::game_event},
};
use sunrise_core::{dimension::Dimension, random::LegacyRandom};
use sunrise_protocol::packets::play::{self, Clientbound as C, Serverbound as S};

fn put_guest_in_end_exit(game: &mut Game<'_>) {
    game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
        g.dimension = Dim::End.id().into();
        g.seen_credits = true;
    });
    game.enter_level(Dim::End).unwrap();
    game.streaming
        .insert_remote(&mut game.world, Chunk::empty(0, 0));
    assert!(
        game.set_state_named([0, 100, 0], "minecraft:end_portal", &[])
            .unwrap()
    );
}

#[derive(Debug, PartialEq, Eq)]
enum EndHook {
    Travel(String),
    Changed(String, String),
    Position(bool),
    Clone(bool),
    Completed(bool),
}

type Heard = Arc<Mutex<Vec<EndHook>>>;

struct EndTravel {
    veto: bool,
    heard: Heard,
}

#[mod_api::__subscriptions]
impl Hooks for EndTravel {
    fn entity_travel_to_dimension(
        &self,
        _: &mut dyn ServerHost,
        event: &mut EntityTravelToDimensionEvent,
    ) -> Flow {
        self.heard
            .lock()
            .unwrap()
            .push(EndHook::Travel(event.cause.to_owned()));
        if self.veto {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn player_changed_dimension(
        &self,
        _: &mut dyn ServerHost,
        event: &mut PlayerChangedDimensionEvent<'_>,
    ) {
        self.heard
            .lock()
            .unwrap()
            .push(EndHook::Changed(event.from.to_owned(), event.to.to_owned()));
    }

    fn player_respawn_position(
        &self,
        _: &mut dyn ServerHost,
        event: &mut PlayerRespawnPositionEvent<'_>,
    ) {
        self.heard
            .lock()
            .unwrap()
            .push(EndHook::Position(event.from_end_fight));
        *event.position = [4.5, 98.0, 4.5];
        *event.copy_original_spawn_position = false;
    }

    fn player_clone(&self, _: &mut dyn ServerHost, event: &mut PlayerCloneEvent<'_>) {
        self.heard
            .lock()
            .unwrap()
            .push(EndHook::Clone(event.was_death));
    }

    fn player_respawned(&self, _: &mut dyn ServerHost, event: &mut PlayerRespawnedEvent<'_>) {
        self.heard
            .lock()
            .unwrap()
            .push(EndHook::Completed(event.end_conquered));
    }
}

fn with_end_travel(
    veto: bool,
    test: impl FnOnce(&mut Game<'_>, &mpsc::Receiver<Out>, &Heard) + Send + 'static,
) {
    run_with_game_stack(move || {
        let heard = Heard::default();
        let mods = vec![ModEntry::new(
            "end-travel",
            Box::new(EndTravel {
                veto,
                heard: heard.clone(),
            }),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            let mut resources = None;
            let (_scratch, mut game, rx) = server_with_a_nether_anchor(&mut resources);
            test(&mut game, &rx, &heard);
        });
    });
}

#[test]
#[ignore = "daily"]
fn credits_respawn_is_not_subject_to_the_end_travel_veto() {
    with_end_travel(true, |game, rx, heard| {
        game.enter_level(Dim::Nether).unwrap();
        light_spawn_chunks(game);
        charge_anchor(game);
        put_guest_in_end_exit(game);

        // As a control, the same listener can veto an ordinary teleport through this portal.
        game.handle_server_portal(2);
        assert_eq!(
            std::mem::take(&mut *heard.lock().unwrap()),
            [EndHook::Travel("end_portal".into())]
        );
        assert!(!game.on_server_trip(2));

        game.lan
            .as_mut()
            .unwrap()
            .hub
            .guests
            .update(0, |g| g.seen_credits = false);
        game.handle_server_portal(2);
        let host = game.lan.as_mut().unwrap();
        assert!(host.play[&2].won_game);
        assert!(!host.hub.guests[0].placed);
        assert!(
            received_packets(rx)
                .any(|packet| matches!(packet, C::GameEvent(event) if event.event == 4))
        );
        host.hub.guests.update(0, |g| {
            g.inbox.push_back(InboxItem::Packet(
                S::ClientCommand(play::ClientCommand { action: 0 }),
                None,
            ));
        });
        game.tick_lan_packets();
        let host = game.lan.as_ref().unwrap();
        let player = &host.hub.guests[0];
        assert!(!host.play[&2].won_game);
        assert!(player.placed);
        assert_eq!(player.dimension, Dim::Nether.id());
        assert_eq!(player.position, [4.5, 98.0, 4.5]);
        assert_eq!(
            *heard.lock().unwrap(),
            [
                EndHook::Position(true),
                EndHook::Clone(false),
                EndHook::Completed(true)
            ]
        );
    });
}

fn ordinary_end_exit(available: bool) {
    with_end_travel(false, move |game, rx, heard| {
        light_spawn_chunks(game);
        assert!(game.ui.rules.set_string("respawn_radius", "0"));
        assert!(
            game.set_state_named([0, 99, 0], "minecraft:stone", &[])
                .unwrap()
        );
        put_guest_in_end_exit(game);
        game.handle_server_portal(2);

        game.enter_level(Dim::Nether).unwrap();
        light_spawn_chunks(game);
        let charged = available.then(|| charge_anchor(game));
        game.tick_server_travel();
        if let Some(charged) = charged {
            let [x, y, z] = ANCHOR;
            assert_eq!(
                game.world.block(x, y, z),
                charged,
                "teleporting does not consume a charge"
            );
        } else {
            game.enter_level(Dim::Overworld).unwrap();
            game.tick_server_travel();
        }
        assert!(!game.on_server_trip(2));
        let player = &game.lan.as_ref().unwrap().hub.guests[0];
        let (dim, at) = if available {
            (Dim::Nether, [10.5, 101.0, 10.5])
        } else {
            (Dim::Overworld, [0.5, 100.0, 0.5])
        };
        assert_eq!(player.dimension, dim.id());
        assert_eq!(player.position, at);
        assert_eq!(
            player
                .respawn_elsewhere
                .as_ref()
                .map(|point| (point.0.as_str(), point.1)),
            Some((Dim::Nether.id(), ANCHOR)),
            "even a missing point still belongs to the same player after a teleport",
        );
        let unavailable = received_packets(rx)
            .filter(|packet| {
                matches!(packet,
                    C::GameEvent(event) if event.event == game_event::NO_RESPAWN_BLOCK_AVAILABLE
                )
            })
            .count();
        assert_eq!(unavailable, usize::from(!available));
        assert_eq!(
            *heard.lock().unwrap(),
            [
                EndHook::Travel("end_portal".into()),
                EndHook::Changed(Dim::End.id().into(), dim.id().into())
            ]
        );
    });
}

#[test]
#[ignore = "daily"]
fn ordinary_end_exit_to_a_valid_point_only_teleports() {
    ordinary_end_exit(true);
}

#[test]
#[ignore = "daily"]
fn ordinary_end_exit_with_a_missing_point_only_teleports() {
    ordinary_end_exit(false);
}

const ANCHOR_DEPLETE: &str = "minecraft:block.respawn_anchor.deplete";
const REDIRECTED: [f64; 3] = [200.5, 72.0, 200.5];
const CLONE_POINT: [i32; 3] = [44, 75, 21];

struct AnchorRules {
    veto_copy: bool,
}

#[mod_api::__subscriptions]
impl Hooks for AnchorRules {
    fn player_respawn_position(
        &self,
        _: &mut dyn ServerHost,
        event: &mut PlayerRespawnPositionEvent<'_>,
    ) {
        if !self.veto_copy {
            *event.level = Dimension::Overworld;
            *event.position = REDIRECTED;
        }
    }

    fn player_clone(&self, host: &mut dyn ServerHost, event: &mut PlayerCloneEvent<'_>) {
        if self.veto_copy {
            use mod_api::{BlockPos, GlobalPos, Identifier, PlayerRespawnConfig};
            let [x, y, z] = CLONE_POINT;
            let point = PlayerRespawnConfig {
                position: GlobalPos::new(
                    Identifier::parse(Dim::Overworld.id()).unwrap(),
                    BlockPos::new(x, y, z),
                ),
                yaw: 0.0,
                forced: true,
            };
            host.player(event.player)
                .unwrap()
                .set_respawn_config(host, Some(point));
        }
    }

    fn player_set_spawn(&self, _: &mut dyn ServerHost, event: &mut PlayerSetSpawnEvent) -> Flow {
        if self.veto_copy && event.pos == Some(ANCHOR) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }
}

fn with_anchor_death(
    veto_copy: bool,
    test: impl FnOnce(&Game<'_>, &mpsc::Receiver<Out>, &mpsc::Receiver<Out>) + Send + 'static,
) {
    run_with_game_stack(move || {
        let mods = vec![ModEntry::new(
            "anchor-rules",
            Box::new(AnchorRules { veto_copy }),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            let mut resources = None;
            let (_scratch, mut game, rx) = server_with_a_nether_anchor(&mut resources);
            *game.world.random = LegacyRandom::new(12345);
            game.enter_level(Dim::Nether).unwrap();
            light_spawn_chunks(&mut game);
            charge_anchor(&mut game);
            let (bystander, other) = placed_guest(3, Dim::Nether, [11.5, 101.0, 10.5]);
            game.lan.as_mut().unwrap().hub.guests.push(bystander);
            game.enter_level(Dim::Overworld).unwrap();
            game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                g.dead = true;
                g.health = 0.0;
            });
            game.guest_respawn(2);
            assert!(!game.on_server_trip(2));
            assert!(game.lan.as_ref().unwrap().hub.guests[0].placed);
            test(&game, &rx, &other);
        });
    });
}

fn anchor_sounds(game: &Game<'_>, rx: &mpsc::Receiver<Out>) -> Vec<play::Sound> {
    let registry_id = game
        .lan
        .as_ref()
        .unwrap()
        .hub
        .registry_id("minecraft:sound_event", ANCHOR_DEPLETE);
    received_packets(rx)
        .filter_map(|packet| {
            let C::Sound(sound) = packet else { return None };
            let is_deplete = match &sound.sound.1 {
                Some((name, _)) => name.as_str() == ANCHOR_DEPLETE,
                None => registry_id == Some(sound.sound.0),
            };
            is_deplete.then_some(sound)
        })
        .collect()
}

#[test]
#[ignore = "daily"]
fn redirected_respawn_sends_the_anchor_sound_only_to_its_player() {
    with_anchor_death(false, |game, rx, other| {
        let player = &game.lan.as_ref().unwrap().hub.guests[0];
        assert_eq!(player.dimension, Dim::Overworld.id());
        assert_eq!(player.position, REDIRECTED);
        let sounds = anchor_sounds(game, rx);
        assert_eq!(sounds.len(), 1);
        let sound = &sounds[0];
        assert_eq!([sound.x, sound.y, sound.z], [80, 800, 80]);
        assert_eq!((sound.source, sound.volume, sound.pitch), (4, 1.0, 1.0));
        // Java Random(12345).nextLong(), from the placement level, not the anchor's level.
        assert_eq!(sound.seed, 6_674_089_274_190_705_457);
        assert!(
            anchor_sounds(game, other).is_empty(),
            "nearby players do not hear this packet"
        );
    });
}

#[test]
#[ignore = "daily"]
fn vetoing_the_spawn_copy_keeps_the_clone_point_and_suppresses_anchor_sound() {
    with_anchor_death(true, |game, rx, other| {
        let player = &game.lan.as_ref().unwrap().hub.guests[0];
        assert_eq!(player.dimension, Dim::Nether.id());
        assert_eq!(player.position, [10.5, 101.0, 10.5]);
        assert_eq!(
            player
                .respawn_elsewhere
                .as_ref()
                .map(|point| (point.0.as_str(), point.1)),
            Some((Dim::Overworld.id(), CLONE_POINT)),
        );
        assert!(anchor_sounds(game, rx).is_empty());
        assert!(anchor_sounds(game, other).is_empty());
    });
}
