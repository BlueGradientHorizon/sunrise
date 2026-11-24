#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Text};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::enchant_tests::RIPTIDE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::enchant_tests::TRIDENT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::enchant_tests::riptide;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::enchant_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{
    Screen,
    lan::LanHost,
    lan_particles::tests::test_hub,
    native_entity_refs::tests::{fixture_root, run_fresh_cases, with_client_game, with_game},
    native_hotbar_slot::hotbar_slot_tests::{logged_in, sent},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    creative::Creative,
    items::ItemStack,
    lan::{Out, data::SyncData},
    menu::Input,
    mobs::finalize::{MAINHAND, item_stack},
    scoreboard::{comp_text, tag_to_json},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::{
    buf::Reader,
    packets::play::{Clientbound as C, GameEvent, Serverbound as S},
    registry::Builtin,
};

pub(in crate::game) fn enchant_outcome(
    game: &mut Game<'_>,
    target: &str,
    level: u32,
    success: bool,
    count: i32,
    key: &str,
    first_arg: Option<&str>,
) {
    let command = format!("enchant {target} {RIPTIDE} {level}");
    game.server.chat_capture = Some(Vec::new());
    let (ok, outcome) =
        game.recording_command_outcome(|g| g.as_console(|g| g.run_command(&command)));
    let feedback = game.server.chat_capture.take().unwrap();
    assert_eq!(
        (ok, outcome),
        (success, Some((success, count))),
        "{command}: {feedback:?}"
    );
    let key = format!("commands.enchant.{key}");
    let component = feedback
        .iter()
        .find_map(|line: &Text| {
            let component = line.component();
            if component["translate"] == key {
                return Some(component);
            }
            component["extra"]
                .as_array()?
                .iter()
                .find(|part| part["translate"] == key)
                .cloned()
        })
        .unwrap_or_else(|| panic!("{command}: expected {key}, got {feedback:?}"));
    if let Some(first_arg) = first_arg {
        assert_eq!(comp_text(&component["with"][0]).plain(), first_arg);
    }
}
