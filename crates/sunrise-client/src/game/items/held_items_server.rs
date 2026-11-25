#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::held_items::OFFHAND_SLOT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::held_items::attack_range_contains;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::held_items::attack_reach;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::held_items::held_attack_item;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::held_items::in_attack_range;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::held_items::*;

impl Game<'_> {
    /// `Player.attack`: the attack damage attribute with held and effect modifiers.
    pub(in crate::game) fn attack_damage(&mut self) -> f32 {
        self.player
            .as_mut()
            .and_then(|p| p.attribute("minecraft:attack_damage").ok())
            .unwrap_or(1.0) as f32
    }
    /// `Minecraft.startAttack` on an entity: main-hand `attack_range` gate.
    pub(in crate::game) fn held_range_allows(&self, distance: f64) -> bool {
        let range = self
            .held()
            .and_then(|stack| self.attack_range_component(stack));
        range.is_none_or(|r| attack_range_contains(&r, distance, self.own_player_is_creative()))
    }
    /// `LivingEntity.updateFallFlying`: the glide's wear. `Util.getRandom`
    /// picks one of the gliders that will not break (from the entity's own
    /// random), and
    /// `ItemStack.hurtAndBreak` takes `points` of its durability (Unbreaking
    /// may save it). Callers skip creative.
    pub(in crate::game::server_side) fn wear_glider(&mut self, points: u32) {
        self.side_wear_glider(points);
    }
}
