#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, spill};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::decorated_pot::DECORATED_POT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::decorated_pot::WOBBLE_NEGATIVE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::decorated_pot::WOBBLE_POSITIVE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::decorated_pot::pot_data_in;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::decorated_pot::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    items::ItemStack,
    mobs::Who,
    particles::{ParticleOptions, ParticleType},
    random_sequences::RandomSequences,
    save,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::LegacyRandom;
#[allow(unused_imports)]
pub(in crate::game::client_side) use {
    sunrise_core::nbt::Tag,
    sunrise_world::loot::{ContainerLootContext, LootData},
};

impl Game<'_> {
    /// The pot's item or empty-hand method, on the captured block state.
    pub(in crate::game) fn use_decorated_pot(
        &mut self,
        pos: [i32; 3],
        state: sunrise_world::registry::BlockStateId,
        held: Option<&ItemStack>,
        branch: sunrise_world::level::UseBranch,
    ) -> Option<mod_api::InteractionResult> {
        use mod_api::InteractionResult as Answer;
        if &*self.res.facts().get(state).name != "minecraft:decorated_pot" {
            return None;
        }
        if matches!(branch, sunrise_world::level::UseBranch::WithoutItem) {
            let current = self.world.block(pos[0], pos[1], pos[2]);
            if &*self.res.facts().get(current).name != "minecraft:decorated_pot" {
                return Some(Answer::Pass);
            }
            self.audio.0.play(
                "minecraft:block.decorated_pot.insert_fail",
                Some(pos.map(|v| f64::from(v) + 0.5)),
                1.0,
                1.0,
            );
            self.block_entity_event(pos, 1, WOBBLE_NEGATIVE);
            self.lan_block_event(pos, 1, WOBBLE_NEGATIVE);
            self.post_block_game_event("minecraft:block_change", pos, Some(Who::Player), None);
            return Some(Answer::Success);
        }
        let Some(mut tag) = self.pot_tag(pos) else {
            return Some(Answer::Pass);
        };
        let mut pot_item = tag.get("item").and_then(save::item_from_nbt);
        let insertable = held.filter(|stack| match &pot_item {
            None => true,
            Some(p) => p.same_item_same_components(stack) && p.count < self.max_stack(p),
        });
        let center = pos.map(|v| f64::from(v) + 0.5);
        let Some(stack) = insertable else {
            spill::put_saved_entity(&mut self.world, pos, tag);
            return Some(Answer::TryWithEmptyHand);
        };
        let stack = stack.clone();
        self.block_entity_event(pos, 1, WOBBLE_POSITIVE);
        self.lan_block_event(pos, 1, WOBBLE_POSITIVE);
        self.award_stat("used", &stack.item, 1);
        // consumeAndReturn(1): one item goes in (a creative player keeps theirs).
        let inserted = match &mut pot_item {
            Some(p) => {
                p.count += 1;
                p
            }
            None => pot_item.insert(ItemStack {
                count: 1,
                ..stack.clone()
            }),
        };
        let pitch_bend = inserted.count as f32 / self.max_stack(inserted) as f32;
        if let (Tag::Compound(map), Some(item)) = (&mut tag, &pot_item) {
            map.insert("item".into(), save::item_to_nbt(item, self.res.items()));
        }
        spill::put_saved_entity(&mut self.world, pos, tag);
        // The sherds and item go to the guests (BlockEntityData).
        self.mark_block_entity_updated((pos[0], pos[1], pos[2]));
        self.consume_held();
        self.audio.0.play(
            "minecraft:block.decorated_pot.insert",
            Some(center),
            1.0,
            0.7 + 0.5 * pitch_bend,
        );
        // ServerLevel.sendParticles(DUST_PLUME, x + 0.5, y + 1.2, z + 0.5, 7, 0, 0, 0, 0):
        // the server draws none of it; the players within 32 blocks do, the window included.
        let plume = ParticleOptions::simple(ParticleType::DustPlume);
        let plume_at = [center[0], f64::from(pos[1]) + 1.2, center[2]];
        self.particles
            .0
            .send_particles(&plume, plume_at, [0.0; 3], 0.0, 7);
        self.lan_send_particles(super::lan_particles::QueuedParticle {
            dimension: self.dim.id(),
            options: plume,
            state: None,
            at: plume_at,
            spread: [0.0; 3],
            speed: [0.0; 3],
            count: 7,
            randomization: 0,
        });
        self.post_block_game_event("minecraft:block_change", pos, Some(Who::Player), None);
        Some(Answer::Success)
    }
}
