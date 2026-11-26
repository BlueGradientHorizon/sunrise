//! The client hand's bone meal interaction.
use super::{Game, native_hooks::BonemealAsk};
use sunrise_core::position::BlockPos;
use sunrise_items::items::ItemStack;

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    /// `BonemealEvent` for the acting hand's bone meal `stack` on `pos`:
    /// the event's answer, and the stack as the mods left it, which goes
    /// back in the hand (`useItemOn` puts a creative player's count back).
    pub(in crate::game) fn held_bonemeal(
        &mut self,
        pos: BlockPos,
        stack: &ItemStack,
    ) -> (BonemealAsk, ItemStack) {
        let mut held = stack.clone();
        let ask = self.bonemeal(pos, true, &mut held);
        if held != *stack {
            if self.creative {
                held.count = stack.count;
            }
            *self.held_mut() = (held.count > 0).then(|| held.clone());
        }
        (ask, held)
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
pub(in crate::game) mod tests {
    use crate::game::growth_features::{build_live_features, install_live_features};
    use mod_api::{self, BonemealEvent, BonemealSource, Flow, Hooks, HostSide};
    use std::sync::Arc;
    use std::sync::{Mutex, MutexGuard, PoisonError};
    use sunrise_core::{geometry::Direction, position::BlockPos};
    use sunrise_items::items::ItemStack;

    /// What each `bonemeal` listener heard: the player, the target check
    /// and the source. The test that reads it runs in a process of its own,
    /// so no other test's events cross into it; a failed assertion doesn't
    /// hide the next one behind poison.
    static HEARD: Mutex<Vec<(Option<u128>, bool, BonemealSource)>> = Mutex::new(Vec::new());

    /// Sets the stack's count to `count`, then cancels with `successful`.
    struct Meal {
        count: Option<u32>,
        cancel: Option<bool>,
    }
    #[mod_api::__subscriptions]
    impl Hooks for Meal {
        fn bonemeal(&self, _: HostSide, event: &mut BonemealEvent<'_>) -> Flow {
            heard().push((event.player, event.valid_bonemeal_target, event.source));
            if let Some(count) = self.count {
                if count == 0 {
                    *event.stack = None;
                } else if let Some(stack) = event.stack.as_mut() {
                    stack.set_count(count).unwrap();
                }
            }
            let Some(successful) = self.cancel else {
                return Flow::Continue;
            };
            event.successful = successful;
            Flow::Cancel
        }
    }

    fn heard() -> MutexGuard<'static, Vec<(Option<u128>, bool, BonemealSource)>> {
        HEARD.lock().unwrap_or_else(PoisonError::into_inner)
    }

    const CROP: BlockPos = BlockPos { x: 0, y: 64, z: 0 };

    /// A dispenser at (0, 64, 1) facing young wheat, with `meal` listening:
    /// what the slot holds after one dispense, and the wheat's age.
    fn dispense(meal: Meal, target: &str) -> (Option<ItemStack>, i32) {
        heard().clear();
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new("meal", Box::new(meal))].into();
        let mut out = None;
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_game(|game| {
                game.options.dedicated = Some(Vec::new());
                game.world.chunks.insert(
                    (0, 0),
                    std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
                );
                let registry = game.pack.assets.registry;
                let farmland = registry.default_state("minecraft:farmland").unwrap();
                game.world.set(0, 63, 0, farmland).unwrap();
                let block = registry.default_state(target).unwrap();
                game.world.set(CROP.x, CROP.y, CROP.z, block).unwrap();
                let left = game
                    .dispense_item(
                        (0, 64, 1),
                        Direction::North,
                        ItemStack::new("minecraft:bone_meal", 3),
                    )
                    .unwrap()
                    .continue_value()
                    .unwrap();
                let b = game.res.behaviours();
                let state = game.world.block(CROP.x, CROP.y, CROP.z);
                out = Some((left, b.table.int(state, b.props.age)));
            });
        });
        out.unwrap()
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

    /// The live features follow the world being opened: a world with a data
    /// pack's feature sees it, the next world without one does not (it used to
    /// keep the first world's registry for the whole process), and re-opening
    /// the same packs reuses the registry built for them.
    #[test]
    #[ignore = "daily"]
    fn the_live_features_follow_the_world_being_opened() {
        // Other feature fixtures can evict the process-wide last-built cache.
        if super::super::native_entity_refs::tests::run_fresh_cases(
            "game::client_side::growth::tests::the_live_features_follow_the_world_being_opened",
            "SUNRISE_LIVE_FEATURE_CACHE_CHILD",
            &["reopen"],
        ) {
            return;
        }
        use sunrise_world::worldgen::overrides::WorldgenOverrides;
        super::super::native_entity_refs::tests::with_game(|game| {
            let mut with_tree = WorldgenOverrides::default();
            with_tree.add_file("mymod", "data/mymod/worldgen/feature/tree.json", || {
                Some(r#"{"type":"minecraft:no_op","config":{}}"#.to_owned())
            });
            let has_tree = |game: &crate::game::ServerGame<'_>| {
                let features = game.res.behaviours().live_features().unwrap();
                assert!(features.configured("minecraft:oak").is_some());
                features.configured("mymod:tree").is_some()
            };
            let open = |game: &crate::game::ServerGame<'_>, overrides: &WorldgenOverrides| {
                install_live_features(&game.res, &game.pack, &game.options, overrides)
            };
            open(game, &WorldgenOverrides::default());
            assert!(!has_tree(game));
            open(game, &with_tree);
            assert!(has_tree(game));
            let first = game.res.behaviours().live_features().unwrap();
            open(game, &with_tree);
            assert!(Arc::ptr_eq(
                &first,
                &game.res.behaviours().live_features().unwrap()
            ));
            open(game, &WorldgenOverrides::default());
            assert!(!has_tree(game));
        });
    }
}
