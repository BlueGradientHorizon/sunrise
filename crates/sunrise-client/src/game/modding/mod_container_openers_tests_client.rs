#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mod_container_openers_tests::*;

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
pub(in crate::game::client_side) use super::super::{Game, lan_actions::MenuOpen, native_entity_refs::tests::run_fresh_cases};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
        BlockState, BlockUpdateFlags, Flow, Hooks, HostBlockStates, MenuReplace,
        ModContainerViewersEvent, ModEntry, PlayerContainerOpenEvent, ServerHost, Text,
        VanillaGameEventEvent,
    };
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::containers::{
        BlockEntity, ModBlockEntity, ModBlockEntityFields, ModContainer,
    };
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::lan::menu::Slots;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mod_container_openers_tests::tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mod_container_openers_tests::tests::POS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mod_container_openers_tests::tests::AT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mod_container_openers_tests::tests::with_container;

    #[test]
    #[ignore = "daily"]
    fn a_shulker_close_sounds_at_its_centre_when_the_listener_replaces_it() {
        if run_fresh_cases(
            "game::client_side::mod_container_openers_tests_client::tests::a_shulker_close_sounds_at_its_centre_when_the_listener_replaces_it",
            "SUNRISE_GUEST_MENU_REENTRY_CASE",
            &["shulker_close"],
        ) {
            return;
        }
        struct Replace;
        #[mod_api::__subscriptions]
        impl Hooks for Replace {
            fn vanilla_game_event(
                &self,
                host: &mut dyn ServerHost,
                event: &mut VanillaGameEventEvent<'_>,
            ) -> Flow {
                if event.event == "minecraft:container_close" {
                    let level = host.level(event.dimension).unwrap();
                    let barrel = BlockState::of(
                        &host.block_states(),
                        mod_api::__private::block_key("minecraft:barrel").unwrap(),
                        &[],
                    )
                    .unwrap();
                    assert_eq!(
                        level.set_block(host, AT, barrel, BlockUpdateFlags::NEIGHBORS),
                        Ok(true)
                    );
                }
                Flow::Continue
            }
        }
        with_container("minecraft:shulker_box", |game| {
            game.open_block_menu(12, POS);
            assert!(
                game.lan
                    .as_ref()
                    .unwrap()
                    .hub
                    .guest(12)
                    .unwrap()
                    .menu
                    .is_some()
            );
            let barrel = game
                .pack
                .assets
                .registry
                .default_state("minecraft:barrel")
                .unwrap();
            game.audio.0.capture = Some(Vec::new());
            mod_platform::testing::with_mods(
                vec![ModEntry::new("replace_shulker", Box::new(Replace))].into(),
                || game.guest_close_all(12),
            );
            assert_eq!(game.world.block(POS.0, POS.1, POS.2), barrel);
            let captured = game.audio.0.capture.as_ref().unwrap();
            let sounds = captured
                .iter()
                .map(|s| (s.event.as_str(), s.position))
                .collect::<Vec<_>>();
            assert_eq!(
                sounds,
                [("minecraft:block.shulker_box.close", [3.5, 64.5, 4.5])]
            );
        });
    }
}
