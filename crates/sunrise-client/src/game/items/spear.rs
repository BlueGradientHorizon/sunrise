//! Spears (`KineticWeapon`): holding the use button charges the spear, and
//! from `delay_ticks` on every living entity along the view that the
//! player's speed allows is stabbed: the damage grows with the speed
//! relative to the target, the knockback and the dismount need the
//! attacker's own speed, and each condition runs out after its time
//! (`Item.use`, `ItemStack.onUseTick`, `KineticWeapon.damageEntities`,
//! `Player.stabAttack`). The hit's feedback (`onKineticHit`) reaches the
//! players around as entity event 2. A left click jabs instead
//! (`PiercingWeapon.attack`), which Lunge turns into a dash.
#[allow(unused_imports)]
use super::{Game, view_vector};
#[allow(unused_imports)]
pub(in crate::game) use sunrise_client::spear::hits_in_reach;
#[allow(unused_imports)]
use sunrise_client::{
    attack_strength::delay,
    collisions::SceneCollisions,
    first_person::Hand,
    items::ItemStack,
    mobs::Damage,
    spear::{HIT_FEEDBACK_TICKS, Kinetic, REACH},
};
#[allow(unused_imports)]
use sunrise_core::{
    geometry::{Aabb, Vec3, traverse_blocks},
    position::BlockPos,
};

/// The key of this window's own player in `Game::kinetic_feedback`.
pub(in crate::game) const OWN: i32 = i32::MIN;

/// What a stab reaches along the view: a mob (its index in `mobs`) or
/// another player (its index in `stab_players`).
#[derive(Clone, Copy)]
pub(in crate::game) enum Target {
    Mob(usize),
    Player(usize),
}

impl Game<'_> {
    /// The boxes of the mobs a spear can hit: alive, and not the vehicle
    /// ridden (`PiercingWeapon.canHitEntity`), with their index.
    pub(in crate::game) fn hittable_mob_boxes(&self) -> Vec<(usize, Aabb)> {
        (self.mobs.iter().enumerate())
            .filter(|(_, m)| m.alive() && !m.removed && self.riding != Some(m.id))
            .map(|(i, m)| (i, m.bounds()))
            .collect()
    }
}
