#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::durability::Wearer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::enchant_effects::MOB_SLOTS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::enchant_effects::PLAYER_SLOTS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::enchant_effects::PlayerSlot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::enchant_effects::look_impulse;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::enchant_effects::stack_enchanted_damage;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::enchant_effects::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use serde_json::Value;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    enchant::{EffectContext, Enchantments, level_value},
    items::{ItemStack, Items},
    mobs::{Damage, DamageKind, Who},
    random_source,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::Vec3,
    position::BlockPos,
    random::{LegacyRandom, RandomSource},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::{
    fluid::FluidKind, level::LevelAccess, registry::BlockStateId,
};

impl Game<'_> {
    /// One `post_piercing_attack` effect on the player: `AllOf`,
    /// `ChangeItemDamage`, `ApplyExhaustion`, `ApplyEntityImpulse` and
    /// `PlaySoundEffect`.
    pub(in crate::game) fn apply_piercing_effect(
        &mut self,
        effect: &serde_json::Value,
        level: u32,
    ) {
        let amount = || level_value(&effect["amount"], level);
        match effect["type"].as_str() {
            Some("minecraft:all_of") => {
                for e in effect["effects"].as_array().into_iter().flatten() {
                    self.apply_piercing_effect(e, level);
                }
            }
            Some("minecraft:change_item_damage") => self.damage_held(amount() as u32),
            Some("minecraft:apply_exhaustion") => {
                if let Some(p) = self.player.as_mut() {
                    p.cause_food_exhaustion(amount());
                }
            }
            // The direction turned by the look (`Entity.getLookQuaternion`:
            // pitch about x, then yaw about y), scaled by coordinate_scale
            // and the magnitude, added to the velocity (the guest's client
            // is told of it by `guest_hit_by`).
            Some("minecraft:apply_impulse") => {
                let impulse = look_impulse(effect, level, self.yaw, self.pitch);
                if let Some(p) = self.player.as_mut() {
                    p.velocity = p.velocity.add_vector(impulse);
                }
            }
            // The sound for the level (a list picks by level), heard by all.
            Some("minecraft:play_sound") => {
                let sounds: Vec<&str> = match &effect["sound"] {
                    serde_json::Value::Array(list) => {
                        list.iter().filter_map(|s| s.as_str()).collect()
                    }
                    one => one.as_str().into_iter().collect(),
                };
                let index = (level as usize).saturating_sub(1);
                if let Some(sound) = sounds.get(index.min(sounds.len().saturating_sub(1))) {
                    let float = |key: &str| effect[key].as_f64().unwrap_or(1.0) as f32;
                    let at = self
                        .player
                        .as_ref()
                        .map(|p| [p.position.x, p.position.y, p.position.z]);
                    self.audio
                        .0
                        .play(sound, at, float("volume"), float("pitch"));
                }
            }
            _ => {}
        }
    }
}
