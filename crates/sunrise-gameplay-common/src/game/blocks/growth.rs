

#[cfg(any(test, feature = "test-support"))]
#[cfg(feature = "client")]
pub mod tests {

use mod_api::{self, BonemealEvent, BonemealSource, Flow, Hooks, HostSide};

use std::sync::{Mutex, MutexGuard, PoisonError};

use sunrise_core::{geometry::Direction, position::BlockPos};

use sunrise_items::items::ItemStack;

/// What each `bonemeal` listener heard: the player, the target check
    /// and the source. The test that reads it runs in a process of its own,
    /// so no other test's events cross into it; a failed assertion doesn't
    /// hide the next one behind poison.
    static HEARD: Mutex<Vec<(Option<u128>, bool, BonemealSource)>> = Mutex::new(Vec::new());
#[cfg(test)]
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
pub fn heard() -> MutexGuard<'static, Vec<(Option<u128>, bool, BonemealSource)>> {
        HEARD.lock().unwrap_or_else(PoisonError::into_inner)
    }
pub const CROP: BlockPos = BlockPos { x: 0, y: 64, z: 0 };
/// A dispenser at (0, 64, 1) facing young wheat, with `meal` listening:
    /// what the slot holds after one dispense, and the wheat's age.
    pub fn dispense(meal: Meal, target: &str) -> (Option<ItemStack>, i32) {
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
}
