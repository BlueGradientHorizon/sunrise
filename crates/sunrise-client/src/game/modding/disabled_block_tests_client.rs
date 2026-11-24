#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::*;

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::super::{
        Dim, Game, Hit,
        api_values::{TryIntoApi, TryIntoEngine},
        native_entity_refs::tests as fixture,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::CARPET;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::ConflictingCode;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::DISPENSER;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::DISPENSER_SLOT;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::DispenseNotices;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::Earlier;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::EnabledPrefix;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::HydratesWithAnyFluid;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::HydratesWithLava;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::HydratesWithWater;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::Machine;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::Observer;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::Owner;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::POS;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::Plant;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::Probe;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::Soil;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::Survival;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::ToolProbe;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::place_dispenser;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::with_inert_block;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::disabled_block_tests::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use mod_api::{
        BlockPart, BlockToolModificationEvent, CanBeHydratedEvent, DispenseMethodEvent, Dispensed,
        Flow, Hooks, Host, LevelWriteHost, Registrar, RegistrationContext, ServerHost,
        TickBlockEvent, ToolAbility, ToolModifiedStateEvent,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_client::{chunk::Chunk, items::ItemStack};
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_core::{position::BlockPos, ticks::TickPriority};
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_world::{
        level::{BlockMapValue, BlockMaps, Landing, LevelAccess},
        registry::BlockStateId,
    };

    #[test]
    #[ignore = "daily"]
    fn inert_dispense_keeps_a_legacy_over_max_stack_without_events() {
        with_inert_block(
            "inert_dispense_keeps_a_legacy_over_max_stack_without_events",
            |game, _, probe| {
                assert!(mod_api::__private::set_mod_enabled("enabled_prefix", false));
                let dispenser = place_dispenser(game);
                game.world
                    .block_entities
                    .get_mut(&DISPENSER_SLOT)
                    .unwrap()
                    .slots_mut()[0] = Some(ItemStack::new("minecraft:stick", 65));
                game.audio.0.capture = Some(Vec::new());
                let particles = game.particles.0.len();
                let notices = probe.dispenser_changes.load(Ordering::Relaxed);
                for (enabled, count, writes) in [(false, 65, 0), (true, 64, 1)] {
                    assert!(mod_api::__private::set_mod_enabled("inert_test", enabled));
                    game.dispense_from(DISPENSER, dispenser).unwrap();
                    assert_eq!(
                        game.world
                            .block_entities
                            .get(&DISPENSER_SLOT)
                            .unwrap()
                            .slots()[0],
                        Some(ItemStack::new("minecraft:stick", count))
                    );
                    assert_eq!(
                        probe.dispenser_changes.load(Ordering::Relaxed),
                        notices + writes
                    );
                    assert!(game.dropped.is_empty());
                    assert!(game.audio.0.capture.as_ref().unwrap().is_empty());
                    assert_eq!(game.particles.0.len(), particles);
                }
            },
        );
    }

    #[test]
    #[ignore = "daily"]
    fn rejected_dispense_reply_preserves_the_disabled_owners_fallback() {
        with_inert_block(
            "rejected_dispense_reply_preserves_the_disabled_owners_fallback",
            |game, _, probe| {
                let dispenser = place_dispenser(game);
                game.audio.0.capture = Some(Vec::new());
                let particles = game.particles.0.len();
                for enabled in [false, true] {
                    assert!(mod_api::__private::set_mod_enabled("inert_test", enabled));
                    for (input, remaining, writes) in [(65, 65, 0), (4, 2, 1)] {
                        let notices = probe.dispenser_changes.load(Ordering::Relaxed);
                        game.world
                            .block_entities
                            .get_mut(&DISPENSER_SLOT)
                            .unwrap()
                            .slots_mut()[0] = Some(ItemStack::new("minecraft:stick", input));
                        game.dispense_from(DISPENSER, dispenser).unwrap();
                        assert_eq!(
                            game.world
                                .block_entities
                                .get(&DISPENSER_SLOT)
                                .unwrap()
                                .slots()[0],
                            Some(ItemStack::new("minecraft:stick", remaining))
                        );
                        assert_eq!(
                            probe.dispenser_changes.load(Ordering::Relaxed),
                            notices + writes
                        );
                        assert!(
                            game.dropped.is_empty(),
                            "vanilla must not run a second behaviour"
                        );
                        assert!(game.audio.0.capture.as_ref().unwrap().is_empty());
                        assert_eq!(game.particles.0.len(), particles);
                    }
                }
                assert_eq!(probe.earlier_dispenses.load(Ordering::Relaxed), 4);
            },
        );
    }
}
