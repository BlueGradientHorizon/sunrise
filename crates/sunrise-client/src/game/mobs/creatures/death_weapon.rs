//! Recorded projectile weapons and the death-loot producer.
#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// `DamageSources.arrow` / `trident`: type and tags follow the actual projectile.
    pub(in crate::game) fn projectile_damage_kind(&self, shot: &Projectile) -> DamageKind {
        match shot.kind {
            ProjectileKind::Arrow { .. } => DamageKind::Arrow,
            ProjectileKind::Trident => DamageKind::Named(
                sunrise_client::mobs::NamedDamage::registered(
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
    pub(in crate::game) fn projectile_attack(&self, shot: &Projectile) -> Attack {
        let mut attack = match shot.owner {
            Some(Who::Player) => self
                .player_entity_id_by_connection(shot.guest)
                .map_or_else(Attack::default, |entity| self.attack_of(entity)),
            Some(Who::Mob(entity)) => self.attack_of(entity as i32),
            None => Attack::default(),
        };
        Self::set_projectile_attack(&mut attack, shot);
        attack
    }
    /// The actual direct projectile's family and saved weapon, also for explicit damage sources.
    pub(in crate::game) fn set_projectile_attack(attack: &mut Attack, shot: &Projectile) {
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
    ) -> sunrise_client::health::DirectEntity {
        match kind {
            ProjectileKind::Arrow { .. } | ProjectileKind::Trident => {
                sunrise_client::health::DirectEntity::Arrow
            }
            ProjectileKind::LargeFireball { .. } => {
                sunrise_client::health::DirectEntity::LargeFireball
            }
            _ => sunrise_client::health::DirectEntity::Projectile,
        }
    }
    /// Only listeners need a saved weapon copy; physical family classification never does.
    pub(in crate::game) fn set_projectile_weapon(attack: &mut Attack, weapon: Option<&ItemStack>) {
        if mod_api::__private::listened(mod_api::HookId::LivingIncomingDamage)
            || mod_api::__private::listened(mod_api::HookId::LivingDeath)
            || mod_api::__private::listened(mod_api::HookId::LootDrops)
        {
            attack.projectile_weapon = weapon.cloned();
        }
    }
    /// What an arrow or a thrown trident hits an entity of type `victim`
    /// for (`AbstractArrow.onHitEntity`, `ThrownTrident.onHitEntity`): a
    /// trident's 8 through `EnchantmentHelper.modifyDamage` of the trident
    /// (Impaling's `minecraft:damage` on `#sensitive_to_impaling`).
    pub(in crate::game) fn shot_damage(
        &mut self,
        shot: &mut Projectile,
        victim: Option<&str>,
        who: Who,
    ) -> f32 {
        let base = self.shot_damage_base(shot, victim, who);
        if matches!(shot.kind, ProjectileKind::Trident) {
            base as f32
        } else {
            shot.arrow_damage_with_base(base) + shot.bonus.damage
        }
    }
    pub(in crate::game) fn drop_death_loot(
        &mut self,
        m: &mut Mob,
        player_uuid: Option<u128>,
        player_killed: bool,
        killer: sunrise_world::loot::LootEntity,
        consumed: bool,
        pos: [f64; 3],
        source: Option<(&'static str, u32)>,
    ) {
        let orbs = self.drop_death_loot_items(
            m,
            player_uuid,
            player_killed,
            killer,
            consumed,
            pos,
            source,
        );
        if let Some(amount) = orbs {
            self.spawn_orbs(pos, amount as u32);
        }
    }

    /// `LivingEntity.dropExperience` for a dying mob: the reward (drawn from
    /// the mob's random after its loot rolls), heard by the mods
    /// (`living_experience_drop`). The orbs to spawn; none when the mob drops
    /// no experience or a sculk catalyst took it.
    pub(in crate::game) fn death_experience(
        &mut self,
        m: &mut Mob,
        player_uuid: Option<u128>,
        player_killed: bool,
        consumed: bool,
    ) -> Option<i32> {
        let mob_drops = self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::MOB_DROPS);
        let experience =
            sunrise_client::mobs::drops::experience_reward(m, player_killed, mob_drops);
        let reward = experience.filter(|_| !consumed)?;
        let kind = crate::game::native_hooks::mob_kind(m);
        Some(self.living_experience_drop(m.id, &kind, player_uuid, reward))
    }
}

#[cfg(test)]
#[path = "death_weapon_tests.rs"]
pub(in crate::game) mod tests;

#[cfg(test)]
#[path = "death_weapon_review_tests.rs"]
pub(in crate::game) mod review_tests;
