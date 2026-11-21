use super::*;
use crate::game::native_entity_refs::tests::{
    bind_acting_guest, bind_local_player, with_client_game, with_game,
};
use mod_api::{Collider, ColliderBox};
use std::sync::Arc;
use sunrise_client::{chunk::Chunk, entities::ItemEntity, items::ItemStack, motion::PlayerMotion};
use sunrise_core::nbt::Tag;

const PLAYER: u32 = 42;
const CONN: u32 = 1;
const PIG_Y: f64 = 73.0;

fn empty_chunk(game: &mut Game<'_>) {
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
}

fn shape() -> Collider {
    Collider {
        boxes: vec![ColliderBox {
            min: [-0.25, -3.0, -0.75],
            max: [0.25, -1.0, 0.75],
        }],
        ..Collider::default()
    }
}

fn holder(game: &mut Game<'_>, x: f64) -> u32 {
    game.native_level_opened(game.dim);
    let id = game
        .summon(
            game.dim.id(),
            "minecraft:pig",
            [x, PIG_Y, 4.5],
            [
                ("NoGravity".into(), Tag::Byte(1)),
                ("NoAI".into(), Tag::Byte(1)),
            ]
            .into_iter()
            .collect(),
        )
        .unwrap();
    game.entity(id)
        .unwrap()
        .set_collider(game, Some(shape()))
        .unwrap();
    id
}

fn shove(game: &mut Game<'_>, holder: u32, x: f64) {
    let start = game.placed_moving_colliders();
    let i = game.mobs.iter().position(|m| m.id == holder).unwrap();
    game.mobs.update(i, |m| m.position.x = x);
    game.carry_pass(&start);
}

fn item(id: u32, x: f64, z: f64, velocity: Vec3) -> ItemEntity {
    ItemEntity::new(
        id,
        ItemStack::new("minecraft:stone", 1),
        Vec3::new(x, 70.25, z),
        velocity,
        0.0,
    )
}

#[test]
#[ignore = "daily"]
fn a_collider_shoves_a_body_crossed_only_mid_sweep() {
    crate::game::in_own_process(
        "game::piston_push::carrying::shoving::tests::a_collider_shoves_a_body_crossed_only_mid_sweep",
        || {
            with_game(|game| {
                empty_chunk(game);
                let holder = holder(game, 2.5);
                let velocity = Vec3::new(-0.125, 0.0, 0.0625);
                game.dropped.push(item(80, 4.5, 4.5, velocity));
                // The other item only grazes the sweep in Z.
                game.dropped.push(item(81, 4.5, 5.375, velocity));
                shove(game, holder, 6.5);
                // The box ends at x=6.75; the item's half width is 0.125.
                assert_eq!(
                    game.dropped[0].position,
                    Vec3::new(6.75 + 0.125 + 0.01, 70.25, 4.5)
                );
                assert_eq!(game.dropped[0].velocity, velocity);
                assert_eq!(game.dropped[1].position, Vec3::new(4.5, 70.25, 5.375));
            });
        },
    );
}

#[test]
#[ignore = "daily"]
fn a_collider_shove_stops_at_a_wall_without_impact_damage() {
    crate::game::in_own_process(
        "game::piston_push::carrying::shoving::tests::a_collider_shove_stops_at_a_wall_without_impact_damage",
        || {
            with_game(|game| {
                empty_chunk(game);
                let holder = holder(game, 2.5);
                let stone = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:stone")
                    .unwrap();
                game.world.set(6, 70, 4, stone).unwrap();
                let item = item(80, 4.5, 4.5, Vec3::ZERO);
                let health = item.health;
                game.dropped.push(item);
                shove(game, holder, 6.5);
                assert_eq!(game.dropped[0].position, Vec3::new(6.0 - 0.125, 70.25, 4.5));
                assert_eq!(game.dropped[0].health, health);
                assert_eq!(game.collider_origin(holder).unwrap().x, 6.5);
            });
        },
    );
}

#[test]
#[ignore = "daily"]
fn a_predicted_collider_shove_passes_the_server_move_check() {
    crate::game::in_own_process(
        "game::piston_push::carrying::shoving::tests::a_predicted_collider_shove_passes_the_server_move_check",
        || {
            let from = Vec3::new(5.0, 70.25, 4.5);
            let (packet, to) = {
                let mut report = None;
                with_client_game(|game| {
                    game.init_remote_for_replay().unwrap();
                    empty_chunk(game);
                    let mut player = PlayerMotion::new(game.motion, from).unwrap();
                    player.entity_id = PLAYER as i32;
                    player.client_side = true;
                    game.player = Some(player);
                    let source = 80;
                    game.spawn_entity(source, "minecraft:pig", [4.0, PIG_Y, 4.5], [0.0; 3]);
                    let remote = game.remote.as_mut().unwrap();
                    remote.entity_id = PLAYER as i32;
                    remote.colliders.insert(
                        source as u32,
                        sunrise_client::colliders::ColliderState::from_api(&shape()).unwrap(),
                    );
                    let start = game.placed_moving_colliders();
                    game.remote
                        .as_mut()
                        .unwrap()
                        .entities
                        .get_mut(&source)
                        .unwrap()
                        .position[0] = 4.75;
                    game.carry_pass(&start);
                    let player = game.player.as_ref().unwrap();
                    let to = player.position.components();
                    assert_eq!(player.bounds().min_position().x, 5.0 + 0.01);
                    let mut sender = sunrise_client::player_sync::PositionSender::default();
                    let _ = sender.update(from.components(), (0.0, 0.0), false, false);
                    let packet = sender
                        .update(
                            to,
                            (0.0, 0.0),
                            player.ground.on_ground,
                            player.collision.horizontal,
                        )
                        .unwrap();
                    report = Some((packet, to));
                });
                report.unwrap()
            };
            with_game(|game| {
                empty_chunk(game);
                holder(game, 4.75);
                game.player = Some(PlayerMotion::new(game.motion, from).unwrap());
                bind_local_player(game, PLAYER, 11);
                game.lan_acting = Some(CONN);
                game.lan_acting_name = Some("pushed-player".into());
                bind_acting_guest(game);
                game.lan_acting = None;
                let hub = &mut game.lan.as_mut().unwrap().hub;
                hub.update_guest(CONN, |g| {
                    g.loaded = true;
                    g.load_timeout = 0;
                });
                hub.guests
                    .inbox_mut(CONN)
                    .unwrap()
                    .push_back(sunrise_client::lan::guest::InboxItem::uncredited(packet));
                let placed = game.placed_moving_colliders();
                moving_colliders::in_level(placed, || game.tick_lan_packets());
                let guest = game.lan.as_ref().unwrap().hub.guest(CONN).unwrap();
                assert_eq!(
                    guest.received_moves, 1,
                    "the normal movement handler read the report"
                );
                assert_eq!(
                    guest.awaiting_teleport, None,
                    "the normal movement check did not correct it"
                );
                assert_eq!(guest.position, to);
            });
        },
    );
}
