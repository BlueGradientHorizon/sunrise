//! Client-side allay interaction prediction.
use super::Game;
use sunrise_client::{first_person::Hand, items::ItemStack, mobs::allay};

impl Game<'_> {
    /// The client's `Allay.mobInteract` runs before the server's response.
    /// Whether `predict_allay_interaction` would act, on a copy of the allay.
    pub(in crate::game) fn allay_would_take(
        &self,
        index: usize,
        hand: Hand,
        offered: Option<ItemStack>,
    ) -> bool {
        let duplicates = offered.as_ref().is_some_and(|s| {
            self.items()
                .item_tags
                .get("minecraft:duplicates_allays")
                .is_some_and(|tag| tag.contains(&*s.item))
        });
        allay::interaction::predict(
            &mut self.mobs[index].clone(),
            offered.as_ref(),
            hand == Hand::Main,
            duplicates,
            self.acting_uuid(),
        )
        .is_some()
    }

    pub(in crate::game) fn predict_allay_interaction(
        &mut self,
        index: usize,
        hand: Hand,
        offered: Option<ItemStack>,
    ) -> bool {
        use allay::interaction::{Interaction, predict};
        let duplicates = offered.as_ref().is_some_and(|s| {
            self.items()
                .item_tags
                .get("minecraft:duplicates_allays")
                .is_some_and(|tag| tag.contains(&*s.item))
        });
        let actor = self.acting_uuid();
        let Some(result) = predict(
            &mut self.mobs[index],
            offered.as_ref(),
            hand == Hand::Main,
            duplicates,
            actor,
        ) else {
            return false;
        };
        self.play_mob_bound_sound(index, result.sound(), 6, 2.0, 1.0, false);
        match result {
            Interaction::Duplicate | Interaction::Give => {
                self.with_hand(hand, Self::consume_held);
            }
            Interaction::Take(stack) => {
                // Player.addItem returns leftovers on the client without
                // spawning an item entity; the server owns any dropped stack.
                let _ = self.add_stack_to_inventory(stack);
            }
        }
        true
    }
}
