#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_entity_refs::tests::with_game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_guest_turn_tests::with_guests;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::ASKS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::Listener;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::PLAYER;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::Rules;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::SCRIPT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::SEEN;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::Script;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::api_stack;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::click;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::count;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::dispatches;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::reset_local;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::script;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::with_local;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::with_mod;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::stacked_on_other_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::item::menu::{ClickedStackResult, StackResult};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    Flow, Hooks, HostSide, HostTransaction, ItemPlace, ItemResource, ItemStackedOnOtherEvent,
    ModEntry, Mods,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::cell::RefCell;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{items::ItemStack, menu::Input};

#[test]
#[ignore = "daily"]
fn a_lan_hub_click_is_decided_the_same_way() {
    use sunrise_client::lan::menu::Slots;
    use sunrise_protocol::packets::play::{ContainerClick, Serverbound};
    with_mod(|| {
        with_guests(|game| {
            let pos = (1, 64, 1);
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
            );
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            game.world.set(pos.0, pos.1, pos.2, stone).unwrap();
            PLAYER.set(12);
            let open = |game: &mut crate::game::Game<'_>| {
                game.world.block_entities.insert(
                    pos,
                    sunrise_client::containers::BlockEntity::Chest(vec![None; 27]),
                );
                let host = game.lan.as_mut().unwrap();
                host.hub.open_container(
                    12,
                    "minecraft:generic_9x3",
                    sunrise_core::nbt::Tag::String("test".into()),
                    pos,
                    Slots::Block(pos),
                    vec![None; 27],
                    None,
                );
                let window = host.hub.guests[0].menu.as_ref().unwrap().window;
                host.hub.guests.update(0, |guest| {
                    guest.inventory = vec![None; guest.inventory.len()];
                    guest.carried = Some(ItemStack::new("minecraft:stone", 3));
                });
                window
            };
            let click = |game: &mut crate::game::Game<'_>, window: i32| {
                game.lan_guest_packet(
                    12,
                    Serverbound::ContainerClick(ContainerClick {
                        container: window,
                        state: 0,
                        slot: 0,
                        button: 0,
                        input: 0,
                        changed: Vec::new(),
                        carried: None,
                    }),
                );
                let slot = game.world.block_entities.get(&pos).unwrap().slots()[0]
                    .as_ref()
                    .map_or(0, |s| s.count);
                let host = game.lan.as_ref().unwrap();
                let guest = &host.hub.guests[0];
                (slot, count(&guest.carried), count(&guest.inventory[20]))
            };
            let window = open(game);
            script(Script::default());
            ASKS.set(0);
            // Row 72 on the LAN route: stone 3 into the empty slot, one dispatch with the
            // frozen stacks, and the LAN host is back.
            assert_eq!(click(game, window), (3, 0, 0));
            // Placement and capacity are separate live slot queries.
            assert_eq!(ASKS.get(), 2);
            assert_eq!(SEEN.with(|s| s.borrow().clone()), [(3, 0)]);
            assert!(game.lan.is_some());

            // The listener proposes dirt, not the stone a plain click would place. Both a
            // permission refusal and an undersized limit keep the cursor and the container.
            for (may_place, limit, count) in [(false, 64, 0), (true, 1, 0), (true, 2, 2)] {
                let window = open(game);
                script(Script {
                    cancel: true,
                    handled: true,
                    set_carried: Some(None),
                    set_clicked: Some(Some(("minecraft:dirt", 2))),
                    dirt_rule: Some((may_place, limit)),
                    write_scoreboard: true,
                    ..Script::default()
                });
                ASKS.set(0);
                assert_eq!(
                    click(game, window),
                    (count, if count == 0 { 3 } else { 0 }, 0)
                );
                assert_eq!(dispatches(), 1);
                // A placement refusal short-circuits before the capacity query.
                assert_eq!(ASKS.get(), if may_place { 2 } else { 1 });
                assert_eq!(game.score.board.objective("pickup").is_some(), count != 0);
            }

            // Rows 73 and 84: a write and then a panic are undone, and the guest swap and
            // host are restored. Run this last: the failing callback is now disabled.
            let window = open(game);
            script(Script {
                write_dirt: Some(20),
                panic: true,
                ..Script::default()
            });
            assert_eq!(click(game, window), (0, 3, 0));
            assert_eq!(dispatches(), 1);
            assert!(game.lan.is_some());
            assert!(game.server.lan_guest_swaps.is_empty());
            let window = open(game);
            script(Script::default());
            assert_eq!(click(game, window), (3, 0, 0));
            assert_eq!(dispatches(), 0);
        });
    });
}
