//! Client-side allay interaction prediction.
use super::{Game, level_sounds::LevelSound};
use mod_api::SoundSource;
use sunrise_client::{
    first_person::Hand, items::ItemStack, lan::playerdata::uuid_ints, mobs::allay,
};

impl Game<'_, crate::game::client_side::ClientState> {
    fn client_acting_uuid(&self) -> [i32; 4] {
        self.remote
            .as_ref()
            .and_then(|r| r.player_uuid(r.entity_id as u32))
            .map(uuid_ints)
            .or(self.level.player_uuid)
            .unwrap_or_default()
    }

    /// The client's `Allay.mobInteract` runs before the server's response.
    /// Whether `predict_allay_interaction` would act, on a copy of the allay.
    pub(in crate::game::client_side) fn allay_would_take(
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
            self.client_acting_uuid(),
        )
        .is_some()
    }

    pub(in crate::game::client_side) fn predict_allay_interaction(
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
        let actor = self.client_acting_uuid();
        let Some(result) = predict(
            &mut self.mobs[index],
            offered.as_ref(),
            hand == Hand::Main,
            duplicates,
            actor,
        ) else {
            return false;
        };
        let mob = &self.mobs[index];
        if !mob.silent {
            let mut sound = LevelSound::new(
                result.sound(),
                [mob.position.x, mob.position.y, mob.position.z],
                2.0,
                1.0,
            )
            .in_source(SoundSource::ALL[6]);
            sound.emitter = Some(mob.id);
            let seed = self.side.audio.0.next_sound_seed();
            self.play_client_level_sound(sound, seed);
        }
        match result {
            Interaction::Duplicate | Interaction::Give => {
                self.with_hand(hand, Self::consume_held);
            }
            Interaction::Take(stack) => {
                let _ = self.add_stack_to_inventory(stack);
            }
        }
        true
    }
}
