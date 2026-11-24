#[allow(unused_imports)]
pub(in crate::game::client_side) use super::super::native_entity_refs::tests::with_game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::super::native_item_places::tests::{
    guest, open_to_lan,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::player_tick_tests::Captain;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::player_tick_tests::Log;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::player_tick_tests::Snapshot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::player_tick_tests::tags_over_steve;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::player_tick_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{Host, HostSide, PlayerRef};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::items::ItemStack;

#[test]
#[ignore = "daily"]
fn client_player_ticks_on_the_death_screen_too() {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let mods = vec![mod_api::ModEntry::new(
        "log",
        Box::new(Log(Arc::clone(&heard))),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_game(|game| {
            game.options.execution_role = super::super::execution_role::ExecutionRole::Client;
            let (client, server) = sunrise_protocol::connection::Pipe::pair();
            game.connect_local(client, "tick-client").unwrap();
            let remote = game.remote.as_mut().unwrap();
            remote.logged_in = true;
            remote.profile = 456;
            remote.entity_id = 42;
            game.player = sunrise_client::motion::PlayerMotion::new(
                game.motion,
                sunrise_core::geometry::Vec3::ZERO,
            )
            .ok();
            game.remote.as_mut().unwrap().level_players.insert(66, 789);
            let kind = game
                .remote
                .as_ref()
                .unwrap()
                .player_type
                .as_ref()
                .unwrap()
                .clone();
            game.mobs.push(sunrise_client::mobs::Mob::new(
                66,
                kind,
                sunrise_core::geometry::Vec3::ZERO,
                0.0,
            ));
            game.cooldowns.add("minecraft:ender_pearl".into(), 3);
            game.sleep_timer = 4;
            // LocalPlayer.tick waits for the client load: only
            // the other player's pair runs, and the level pass still ticks.
            game.tick_remote().unwrap();
            // Player.tick owns both counters, so they wait with it.
            assert_eq!(game.cooldowns.remaining().next().unwrap().1, 3);
            assert_eq!(game.sleep_timer, 4);
            let phases = || {
                std::mem::take(&mut *heard.lock().unwrap())
                    .iter()
                    .map(|s| (s.phase, s.side, s.uuid))
                    .collect::<Vec<_>>()
            };
            assert_eq!(phases(), [("pre", "client", 789), ("post", "client", 789)]);
            game.remote.as_mut().unwrap().loaded_sent = true;
            game.streaming
                .insert_remote(&mut game.world, sunrise_client::chunk::Chunk::empty(0, 0));
            // The other player's tick ends before the local player's starts.
            game.tick_remote().unwrap();
            assert_eq!(game.cooldowns.remaining().next().unwrap().1, 2);
            assert_eq!(game.sleep_timer, 5);
            game.client.screen = super::super::Screen::Dead;
            game.player
                .as_mut()
                .unwrap()
                .set_health(0.0, false)
                .unwrap();
            game.tick_remote().unwrap();
            assert_eq!(
                phases(),
                [
                    ("pre", "client", 789),
                    ("post", "client", 789),
                    ("pre", "client", 456),
                    ("post", "client", 456),
                    ("pre", "client", 789),
                    ("post", "client", 789),
                    ("pre", "client", 456),
                    ("post", "client", 456)
                ]
            );
            server.shutdown();
        })
    });
}
