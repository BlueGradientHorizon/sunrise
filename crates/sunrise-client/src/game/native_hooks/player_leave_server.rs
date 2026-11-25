#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::lan::guest::Guest;

#[cfg(test)]
pub(in crate::game) mod player_leave_tests {
    #[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{
        Dim, Game, native_entity_refs::tests::run_fresh_cases,
        native_guest_turn_tests::with_guests, native_item_places::tests::guest,
    };
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
        Hooks, ModEntry, PlayerBedLeaveEvent, PlayerLoggedOutEvent, Registrar, ServerHost,
    };
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::lan::{NetEvent, playerdata};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::{Compound, Tag};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_mod_runtime::attachments::{
        AttachmentCodec, AttachmentError, AttachmentType, Holder,
    };
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::FLAG;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::SAVED;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::ASLEEP;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::VALUE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::Heard;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::Leave;

    /// Two guests, 12 then 13, a listener on both hooks, and guest 12's connection closing as
    /// the hub hears it (`NetEvent::Left`, `Hub::remove_guest`). `prepare` sets the guests up
    /// first; `check` gets the game and what the listener heard.
    pub(in crate::game) fn first_of_two_leaves(
        prepare: impl FnOnce(&mut Game<'_>),
        check: impl FnOnce(&mut Game<'_>, Vec<Heard>),
    ) {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![ModEntry::new("leave_test", Box::new(Leave(heard.clone())))].into();
        mod_platform::testing::with_mods(mods, || {
            with_guests(|game| {
                game.server.world_root = game.save.0.root.clone();
                game.options.dedicated = Some(Vec::new());
                game.native_level_opened(Dim::Overworld);
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
                prepare(game);
                let shared = &game.lan.as_ref().unwrap().hub.lan.shared;
                shared
                    .events
                    .send(NetEvent::Left(12, "Disconnected".into()))
                    .unwrap();
                game.tick_lan_packets();
                let told = std::mem::take(&mut *heard.lock().unwrap());
                check(game, told);
            });
        });
    }

    /// NeoForge's `PlayerLoggedOutEvent` at the head of `PlayerList.remove`: a listener finds
    /// the leaving player still listed in their place in join order, changes them, and the save
    /// that follows keeps it. One who quit from the End credits (out of every level) is listed,
    /// found and keeps the attachment too.
    #[test]
    #[ignore = "daily"]
    fn what_a_listener_changes_on_a_leaving_player_is_saved() {
        if run_fresh_cases(SAVED, FLAG, &["placed", "end_credits"]) {
            return;
        }
        let credits = std::env::var(FLAG).unwrap() == "end_credits";
        first_of_two_leaves(
            |game| {
                if credits {
                    // ServerPlayer.showEndCredits: out of the level, still joined.
                    game.lan.as_mut().unwrap().hub.remove_player_for_respawn(0);
                }
            },
            |game, told| {
                let [
                    Heard::LoggedOut {
                        listed,
                        found,
                        asleep: false,
                        attached,
                        healed,
                    },
                ] = &told[..]
                else {
                    panic!("one logout expected: {told:?}");
                };
                assert_eq!(*listed, [12, 13]);
                assert!(*found);
                assert_eq!(*attached, Ok(()));
                assert_eq!(*healed, !credits, "{told:?}");
                assert_eq!(game.guest_index(12), None);
                let mut saved = guest(12);
                saved.uuid = 12;
                assert!(playerdata::load(&mut saved, &game.server.world_root));
                assert_eq!(VALUE.get(&mut saved.attachments, Holder::Level), Ok(77));
                if !credits {
                    assert_eq!(saved.health, 7.0);
                }
            },
        );
    }
}
