use super::super::Game;
use sunrise_items::items::ItemStack;
use sunrise_sim::first_person_sim::Hand;

#[cfg(feature = "client")]
impl crate::game::ClientGame<'_> {
    /// `FlintAndSteelItem.useOn` / `FireChargeItem.useOn` on this client's level:
    /// lighting an unlit campfire, candle or candle cake, or placing fire at the hit face.
    /// True when it succeeded (SUCCESS, swings and ends the click); false when
    /// placement is rejected (FAIL, no swing, ends the click).
    pub(in crate::game) fn predict_fire_starter(
        &mut self,
        stack: &ItemStack,
        hit: &super::super::Hit,
        hand: Hand,
        seq: i32,
    ) -> bool {
        let is_fire_charge = &*stack.item == "minecraft:fire_charge";
        let clicked = self.common.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
        let name = self.block_name(hit.pos);
        // TntBlock.useItemOn primes TNT unless sneaking with hands full.
        // Server priming sends TNT_PRIMED and can refuse priming; leave ignition
        // effects and item consumption to the server.
        let sneaking = self.remote_shift_down();
        let hands_full = self.common.stack_in(self.common.selected).is_some() || self.common.offhand.is_some();
        if !(sneaking && hands_full) && &*name == "minecraft:tnt" {
            return true;
        }
        if self.common.can_light(clicked, &name) {
            let registry = self.common.pack.assets.registry;
            if let Some(mut state) = registry.state(clicked).cloned() {
                state.properties.insert("lit".into(), "true".into());
                if let Ok(lit_state) = registry.resolve(&state) {
                    let pos =
                        sunrise_core::position::BlockPos::new(hit.pos[0], hit.pos[1], hit.pos[2]);
                    self.predict_blocks(&[(pos, lit_state)], seq);
                }
            }
            let at = [
                f64::from(hit.pos[0]) + 0.5,
                f64::from(hit.pos[1]) + 0.5,
                f64::from(hit.pos[2]) + 0.5,
            ];
            let sound = if is_fire_charge {
                "minecraft:item.firecharge.use"
            } else {
                "minecraft:item.flintandsteel.use"
            };
            let pitch =
                sunrise_items::items::fire_starter_pitch(is_fire_charge, &mut self.common.world.random);
            self.client.audio.0.play(sound, Some(at), 1.0, pitch);
            if is_fire_charge && !self.common.creative {
                self.common.with_hand(hand, Game::consume_held);
            }
            return true;
        }
        let target = [
            hit.pos[0] + hit.face[0],
            hit.pos[1] + hit.face[1],
            hit.pos[2] + hit.face[2],
        ];
        if self.common.can_place_used_fire(target) {
            let pos = sunrise_core::position::BlockPos::new(target[0], target[1], target[2]);
            let state = self.common.with_level(|b, level, ticks| b.fire_state_for(level, ticks, pos));
            self.predict_blocks(&[(pos, state)], seq);
            let sound = sunrise_items::items::fire_starter_sound(is_fire_charge);
            let pitch =
                sunrise_items::items::fire_starter_pitch(is_fire_charge, &mut self.common.world.random);
            let target_at = [
                f64::from(target[0]) + 0.5,
                f64::from(target[1]) + 0.5,
                f64::from(target[2]) + 0.5,
            ];
            self.client.audio.0.play(sound, Some(target_at), 1.0, pitch);
            if is_fire_charge && !self.common.creative {
                self.common.with_hand(hand, Game::consume_held);
            }
            return true;
        }
        false
    }
}

use sunrise_gameplay_common::game::tables::WorldTablesExt as _;
