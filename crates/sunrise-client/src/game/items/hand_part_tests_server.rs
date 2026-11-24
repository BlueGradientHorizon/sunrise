#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand_part_tests::Parts;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand_part_tests::REEQUIP;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand_part_tests::REEQUIP_ASKED;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand_part_tests::RESET;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand_part_tests::RESET_ASKED;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand_part_tests::Stick;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand_part_tests::close;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand_part_tests::dip;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand_part_tests::settle;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand_part_tests::with_stick_part;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand_part_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    ClientHost, Hooks, ItemPart, Registrar, RegistrationContext, ShouldCauseBlockBreakResetEvent,
    ShouldCauseReequipAnimationEvent,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Mutex;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::items::ItemStack;

#[test]
#[ignore = "daily"]
fn remote_mining_component_update_restarts_unless_the_part_refuses() {
    crate::game::in_own_process(
        "game::server_side::hand_part_tests_server::remote_mining_component_update_restarts_unless_the_part_refuses",
        || {
            for keep in [false, true] {
                with_stick_part(|game| {
                    use std::sync::Arc;
                    use sunrise_client::{chunk::Chunk, motion::PlayerMotion};
                    use sunrise_core::geometry::Vec3;
                    use sunrise_protocol::packets::play;
                    *RESET.lock().unwrap() = keep.then_some(false);
                    // Play packets are suppressed until login/configuration finishes.
                    let (mut peer, _writer) = crate::game::remote::tests::connect_menu_test(game);
                    peer.set_read_timeout(Some(std::time::Duration::from_millis(50)))
                        .unwrap();
                    game.set_local_mode(sunrise_client::command::args::GameMode::Survival);
                    let mut player =
                        PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 3.5)).unwrap();
                    player.ground.on_ground = true;
                    game.player = Some(player);
                    game.inventory[0] = Some(ItemStack::new("minecraft:stick", 1));
                    game.selected = 0;
                    game.world
                        .chunks
                        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                    let stone = game
                        .pack
                        .assets
                        .registry
                        .default_state("minecraft:stone")
                        .unwrap();
                    game.world.set(0, 64, 0, stone).unwrap();
                    game.server.forced_hit = Some(crate::game::Hit {
                        pos: [0, 64, 0],
                        face: [0, 1, 0],
                        point: [0.5, 65.0, 0.5],
                        inside: false,
                    });
                    game.refresh_client_pick(1.0);
                    game.server.forced_hit = None;
                    for _ in 0..4 {
                        game.remote_continue_attack(true);
                    }
                    let before = game.mining.as_ref().unwrap().progress;
                    while peer.recv().is_ok() {}
                    game.inventory[0].as_mut().unwrap().data_mut().custom_name =
                        Some(sunrise_core::nbt::Tag::String("Wand".into()));
                    game.remote_continue_attack(true);
                    let mut actions = Vec::new();
                    while let Ok((id, bytes)) = peer.recv() {
                        if let Ok(play::Serverbound::PlayerAction(action)) =
                            play::Serverbound::decode(id, &bytes)
                        {
                            actions.push(action.action);
                        }
                    }
                    let after = game.mining.as_ref().unwrap().progress;
                    if keep {
                        assert!(actions.is_empty());
                        assert!((after - before - 1.0 / 150.0).abs() < 1e-6);
                    } else {
                        assert_eq!(actions, [2, 0]);
                        assert_eq!(after, 0.0);
                    }
                });
            }
        },
    );
}
