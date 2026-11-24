//! Nautili on the game side (`AbstractNautilus.mobInteract`): taming with
//! pufferfish, feeding, the saddle and armour, riding, and the zombie
//! nautilus a drowned with a trident sometimes rides.
#[allow(unused_imports)]
use super::Game;
#[allow(unused_imports)]
use sunrise_client::{items::ItemStack, mobs::nautilus};
#[allow(unused_imports)]
use sunrise_core::random::RandomSource;

impl Game<'_> {
    pub(in crate::game) fn item_tagged(&self, tag: &str, item: &str) -> bool {
        self.res
            .items()
            .item_tags
            .get(tag)
            .is_some_and(|t| t.contains(item))
    }
    /// `AbstractNautilus.usePlayerItem`: bucketed fish leave a water bucket.
    pub(in crate::game) fn use_nautilus_food(&mut self, item: &str) {
        let bucket = self.item_tagged("minecraft:nautilus_bucket_food", item);
        self.consume_held();
        if bucket && !self.creative {
            let water = ItemStack::new("minecraft:water_bucket", 1);
            if self.inventory[self.selected].is_none() {
                self.inventory[self.selected] = Some(water);
            } else if let Some(rest) = self.add_stack_to_inventory(water) {
                let at = self.eye_position().unwrap_or([0.0; 3]);
                self.spawn_item(rest, at, [0.0; 3], 40);
            }
        }
    }
    pub(in crate::game) fn nautilus_sound(&mut self, index: usize, sound: &str) {
        let at = self.mobs[index].center();
        self.audio
            .0
            .play(&format!("minecraft:{sound}"), Some(at), 1.0, 1.0);
    }
    /// `AbstractNautilus.interact` (`super.interact` is `Mob.interact`): any
    /// click on a nautilus, used or not, makes it persistent before its own
    /// handling runs.
    pub(in crate::game) fn nautilus_clicked(&mut self, id: Option<i32>) {
        let Some(index) = id.and_then(|id| Self::named_entity(&self.mobs, id, |m| m.id)) else {
            return;
        };
        if nautilus::is_nautilus(self.mobs[index].species()) {
            self.mobs[index].persistent = true;
        }
    }
}

#[cfg(test)]
#[path = "tests/nautili_tests.rs"]
pub(in crate::game) mod nautili_tests;
