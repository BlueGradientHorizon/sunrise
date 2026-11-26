use super::super::Game;
use crate::game::remote::interaction_event_tests::*;
use mod_api::{self, LogicalSide};
use std::sync::{Arc, Mutex};
use sunrise_core::geometry::Vec3;
use sunrise_sim::first_person_sim::Hand;

#[path = "mapping_press_tests_client.rs"]
pub(in crate::game) mod mapping_press_tests;

impl InteractionSide for crate::game::client_side::ClientState {
    const IS_CLIENT: bool = true;
    const LOGICAL_SIDE: LogicalSide = LogicalSide::Client;
    fn set_interaction_mode(game: &mut Game<'_, Self>, mode: sunrise_command::args::GameMode) {
        game.set_local_mode(mode);
    }
    fn interaction_use_block_hand(game: &mut Game<'_, Self>, hand: Hand) -> bool {
        game.remote_use_item_on(hand, mod_api::InteractionAttempt::Press, None)
    }
    fn interaction_use_air_hand(game: &mut Game<'_, Self>, hand: Hand) -> bool {
        game.remote_use_item(hand)
    }
    fn interaction_use_item_as_player(game: &mut Game<'_, Self>) -> bool {
        game.remote_use_item(Hand::Main)
    }
    fn interaction_set_sneak(game: &mut Game<'_, Self>, sneak: bool) {
        game.set_mapping_held("key.sneak", sneak);
        if let Some(remote) = game.side.remote.as_mut() {
            if sneak {
                remote.last_input |= crate::game::remote::SHIFT_INPUT;
            } else {
                remote.last_input &= !crate::game::remote::SHIFT_INPUT;
            }
        }
    }
    fn interaction_clear_guest_use(_game: &mut Game<'_, Self>) {}
    fn interaction_reset_use_failed(_game: &mut Game<'_, Self>) {}
}
