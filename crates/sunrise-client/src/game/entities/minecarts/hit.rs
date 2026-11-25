//! `VehicleEntity.hurtServer`'s `DamageSource` as far as a minecart reads it,
//! and the tail of an accepted hit: the `ENTITY_DAMAGE` game event, the drop's
//! `CustomName` (`destroy`) and `chestVehicleDestroyed`'s angry piglins.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::commands::Target;

/// The `DamageSource` a cart took. `VehicleEntity.hurtServer` attributes its
/// `ENTITY_DAMAGE` to `source.getEntity()`, and `chestVehicleDestroyed` angers
/// piglins only when `source.getDirectEntity()` is the player.
#[derive(Clone, Copy, Default)]
pub(in crate::game) struct CartHit {
    pub(in crate::game) by: Option<Target>,
    pub(in crate::game) by_player: bool,
}

impl CartHit {
    /// `/damage`'s `DamageSource`: `by` is its direct entity, `causing` the
    /// causing entity `getEntity()` names.
    pub(in crate::game) fn command(by: Option<Target>, causing: Option<Target>) -> Self {
        Self {
            by: causing,
            by_player: matches!(by, Some(Target::Player)),
        }
    }
    /// A hit whose causing entity is `by` and whose direct entity is no player:
    /// a thrown trident, or a blast's indirect source.
    pub(in crate::game) fn attributed(by: Option<Target>) -> Self {
        Self {
            by,
            by_player: false,
        }
    }
}

/// The tail of a cart's `hurtServer`.
impl Game<'_> {
    /// `VehicleEntity.hurtServer`: shake, add the damage, break past 40 (at
    /// once for creative players); TNT minecarts prime instead when moving
    /// or hurt by fire or an explosion (`damageSourceIgnitesTnt`).
    pub(in crate::game) fn hurt_cart(
        &mut self,
        i: usize,
        damage: f32,
        creative: bool,
        fire: bool,
        hit: CartHit,
    ) {
        self.hurt_cart_result(i, damage, creative, fire, hit);
    }
    /// `VehicleEntity.destroy(level, dropItem)`: the cart's item, carrying the
    /// vehicle's `CustomName` as `DataComponents.CUSTOM_NAME`.
    pub(in crate::game) fn cart_drop(&self, kind: CartKind, id: u32) -> ItemStack {
        let mut stack = ItemStack::new(kind.item(), 1);
        if let Some(name) = self.persist_raw(id).and_then(|raw| raw.get("CustomName")) {
            let text = sunrise_client::entity_nbt::name_text(name);
            stack.data_mut().custom_name =
                Some(sunrise_client::scoreboard::component_tag(&text.component()));
        }
        stack
    }
    /// `chestVehicleDestroyed`'s `angerNearbyPiglins`, for a container cart
    /// (`AbstractMinecartContainer`) that the hit's direct entity, a player,
    /// broke.
    pub(in crate::game) fn cart_destroy_angers(&mut self, hit: CartHit, kind: CartKind) {
        if hit.by_player && kind.slots() > 0 {
            self.anger_piglins(true);
        }
    }
}

#[cfg(test)]
#[path = "minecart_hit_tests.rs"]
pub(in crate::game) mod hit_tests;
