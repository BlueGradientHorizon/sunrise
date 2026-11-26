//! Recorded projectile weapons and the death-loot producer.
use super::*;

impl crate::game::CommonGame<'_> {
    /// `DamageSources.arrow` / `trident`: type and tags follow the actual projectile.
    pub fn projectile_damage_kind(&self, shot: &Projectile) -> DamageKind {
        match shot.kind {
            ProjectileKind::Arrow { .. } => DamageKind::Arrow,
            ProjectileKind::Trident => DamageKind::Named(
                sunrise_sim::mobs::NamedDamage::registered(
                    &self.health_resources.damage,
                    "minecraft:trident",
                )
                .expect("registered vanilla trident damage"),
            ),
            ProjectileKind::SmallFireball | ProjectileKind::LargeFireball { .. } => {
                DamageKind::Fireball
            }
            _ => DamageKind::Thrown,
        }
    }
    /// The actual direct projectile's family and saved weapon, also for explicit damage sources.
    pub fn set_projectile_attack(attack: &mut Attack, shot: &Projectile) {
        attack.projectile = Some(shot.id as i32);
        attack.projectile_kind = Self::projectile_direct_entity(&shot.kind);
        attack.projectile_type = Some(shot.entity_type());
        let weapon = shot.arrow.as_ref().and_then(|arrow| {
            if matches!(shot.kind, ProjectileKind::Trident) {
                Some(&arrow.pickup_item)
            } else {
                arrow.fired_from_weapon.as_ref()
            }
        });
        Self::set_projectile_weapon(attack, weapon);
    }
    /// Classify a projectile's vanilla `DamageSource.getDirectEntity()` family.
    pub(in crate::game) fn projectile_direct_entity(
        kind: &ProjectileKind,
    ) -> sunrise_sim::health::DirectEntity {
        match kind {
            ProjectileKind::Arrow { .. } | ProjectileKind::Trident => {
                sunrise_sim::health::DirectEntity::Arrow
            }
            ProjectileKind::LargeFireball { .. } => {
                sunrise_sim::health::DirectEntity::LargeFireball
            }
            _ => sunrise_sim::health::DirectEntity::Projectile,
        }
    }
    /// Only listeners need a saved weapon copy; physical family classification never does.
    pub fn set_projectile_weapon(attack: &mut Attack, weapon: Option<&ItemStack>) {
        if mod_api::__private::listened(mod_api::HookId::LivingIncomingDamage)
            || mod_api::__private::listened(mod_api::HookId::LivingDeath)
            || mod_api::__private::listened(mod_api::HookId::LootDrops)
        {
            attack.projectile_weapon = weapon.cloned();
        }
    }
}
