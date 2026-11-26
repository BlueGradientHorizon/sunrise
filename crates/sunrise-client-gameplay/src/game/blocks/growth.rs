//! The client hand's bone meal interaction.
use super::{Game, native_hooks::BonemealAsk};
use sunrise_core::position::BlockPos;
use sunrise_items::items::ItemStack;

#[cfg(feature = "client")]
impl crate::game::ClientGame<'_> {
    /// `BonemealEvent` for the acting hand's bone meal `stack` on `pos`:
    /// the event's answer, and the stack as the mods left it, which goes
    /// back in the hand (`useItemOn` puts a creative player's count back).
    pub(in crate::game) fn held_bonemeal(
        &mut self,
        pos: BlockPos,
        stack: &ItemStack,
    ) -> (BonemealAsk, ItemStack) {
        let mut held = stack.clone();
        let ask = self.common.bonemeal(pos, true, &mut held);
        if held != *stack {
            if self.common.creative {
                held.count = stack.count;
            }
            *self.common.held_mut() = (held.count > 0).then(|| held.clone());
        }
        (ask, held)
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
pub(in crate::game) mod tests {
use sunrise_gameplay_common::game::client_side::growth::tests::CROP;

use sunrise_gameplay_common::game::client_side::growth::tests::dispense;

use sunrise_gameplay_common::game::client_side::growth::tests::heard;

    use sunrise_gameplay_common::game::growth_features::{build_live_features, install_live_features};
    use mod_api::{self, BonemealEvent, BonemealSource, Flow, Hooks, HostSide};
    use std::sync::Arc;
    use std::sync::{Mutex, MutexGuard, PoisonError};
    use sunrise_core::{geometry::Direction, position::BlockPos};
    use sunrise_items::items::ItemStack;

    

    /// Sets the stack's count to `count`, then cancels with `successful`.
    struct Meal {
        count: Option<u32>,
        cancel: Option<bool>,
    }
    

    

    

    

    /// A successful cancel keeps the meal and grows nothing; an unsuccessful
    /// one fails; an untouched use grows the stack the mod edited. One test,
    /// run in order in a fresh process, since every case reads `HEARD`.
    #[test]
    #[ignore = "daily"]
    fn a_dispensers_bonemeal_follows_the_cancel_and_the_edited_stack() {
        if super::super::native_entity_refs::tests::run_fresh_cases(
            "game::client_side::growth::tests::a_dispensers_bonemeal_follows_the_cancel_and_the_edited_stack",
            "SUNRISE_BONEMEAL_DISPENSE_CHILD",
            &["dispense"],
        ) {
            return;
        }
        let (left, age) = dispense(
            Meal {
                count: None,
                cancel: Some(true),
            },
            "minecraft:wheat",
        );
        assert_eq!(left.map(|s| s.count), Some(3));
        assert_eq!(age, 0);
        // A dispenser has no player; the young wheat is a target.
        assert_eq!(*heard(), [(None, true, BonemealSource::Interaction)]);

        let (left, age) = dispense(
            Meal {
                count: None,
                cancel: Some(false),
            },
            "minecraft:wheat",
        );
        assert_eq!((left.map(|s| s.count), age), (Some(3), 0));
        // Left to vanilla, growCrop uses one of the stack the mod edited.
        let (left, age) = dispense(
            Meal {
                count: Some(5),
                cancel: None,
            },
            "minecraft:wheat",
        );
        assert_eq!(left.map(|s| s.count), Some(4));
        assert!(age > 0, "the wheat grew");
        // Stone is no target, and the dispense fails with the meal kept.
        let (left, _) = dispense(
            Meal {
                count: None,
                cancel: None,
            },
            "minecraft:stone",
        );
        assert_eq!(left.map(|s| s.count), Some(3));
        assert_eq!(*heard(), [(None, false, BonemealSource::Interaction)]);
    }

    #[test]
    #[ignore = "daily"]
    fn the_hand_keeps_the_mods_stack_and_a_creative_count() {
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "meal",
            Box::new(Meal {
                count: Some(1),
                cancel: Some(true),
            }),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_client_game(|game| {
                let stack = ItemStack::new("minecraft:bone_meal", 3);
                *game.held_mut() = Some(stack.clone());
                assert_eq!(game.held_bonemeal(CROP, &stack).0.crop, Some(true));
                assert_eq!(game.held().map(|s| s.count), Some(1));
                game.creative = true;
                *game.held_mut() = Some(stack.clone());
                let (ask, held) = game.held_bonemeal(CROP, &stack);
                assert_eq!((ask.crop, held.count), (Some(true), 3));
                assert_eq!(game.held().map(|s| s.count), Some(3));
            });
        });
    }

    /// A data pack's configured feature (a mod sapling's tree) is in the live
    /// registry when the world's worldgen entries are given, and a vanilla one
    /// stays; without them the pack's feature is unknown and its growth dropped.
    #[test]
    #[ignore = "daily"]
    fn a_data_packs_feature_is_in_the_live_registry() {
        use sunrise_world::worldgen::overrides::WorldgenOverrides;
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let registry = sunrise_level::assets::Assets::load(&root).unwrap().registry;
        let source = sunrise_assets::resource_source::ResourceSource::directory(root.clone());
        let mut overrides = WorldgenOverrides::default();
        let file = "data/mymod/worldgen/feature/tree.json";
        overrides.add_file("mymod", file, || {
            Some(r#"{"type":"minecraft:no_op","config":{}}"#.to_owned())
        });
        let live = |overrides| {
            build_live_features(&root, registry, &source, overrides)
                .unwrap()
                .unwrap()
        };
        let with = live(&overrides);
        assert!(with.configured("mymod:tree").is_some());
        assert!(with.configured("minecraft:oak").is_some());
        assert!(
            live(&WorldgenOverrides::default())
                .configured("mymod:tree")
                .is_none()
        );
    }

    
}
