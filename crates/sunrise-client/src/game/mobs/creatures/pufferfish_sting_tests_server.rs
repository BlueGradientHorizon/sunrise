#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::mobs::Mob;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::motion::PlayerMotion;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::play::Clientbound;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pufferfish_player::pufferfish_sting_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pufferfish_player::pufferfish_sting_tests::health;

/// The game events the stung player's client was sent, as `(event, parameter)`.
pub(in crate::game) fn stings(game: &crate::game::ServerGame<'_>) -> Vec<(u8, f32)> {
    game.side
        .guest_packets
        .iter()
        .filter_map(|packet| match packet {
            Clientbound::GameEvent(e) => Some((e.event, e.param)),
            _ => None,
        })
        .collect()
}

/// A game whose window player touches one puffed (state 2) pufferfish. The
/// server acts for the guest `lan_acting`: `playerTouch` runs there, and the
/// event it sends reaches that player's own client.
pub(in crate::game) fn with_fish(silent: bool, test: impl FnOnce(&mut crate::game::ServerGame<'_>)) {
    crate::game::native_entity_refs::tests::with_game(|game| {
        game.creative = false;
        // The acting body is the guest's, not a second local player with the same ID.
        let mut body = PlayerMotion::new(game.motion, Vec3::new(8.5, 66.0, 8.5)).unwrap();
        body.entity_id = 1;
        game.player = Some(body);
        game.side.lan_acting_entity = Some(1);
        game.side.lan_acting_uuid = Some(1);
        game.side.lan_acting = Some(7);
        game.side.lan_acting_name = Some("PufferfishTest".into());
        crate::game::native_entity_refs::tests::bind_acting_guest(game);
        let kind = game
            .res
            .mob_types()
            .get("minecraft:pufferfish")
            .unwrap()
            .clone();
        let mut fish = Mob::new(9001, kind, Vec3::new(8.5, 66.0, 8.5), 0.0);
        fish.extra.size = 2;
        fish.silent = silent;
        game.mobs.push(fish);
        test(game);
    });
}
