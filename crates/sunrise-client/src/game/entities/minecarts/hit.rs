//! `VehicleEntity.hurtServer`'s `DamageSource` as far as a minecart reads it,
//! and the tail of an accepted hit: the `ENTITY_DAMAGE` game event, the drop's
//! `CustomName` (`destroy`) and `chestVehicleDestroyed`'s angry piglins.
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

#[cfg(test)]
#[path = "minecart_hit_tests.rs"]
pub(in crate::game) mod hit_tests;
