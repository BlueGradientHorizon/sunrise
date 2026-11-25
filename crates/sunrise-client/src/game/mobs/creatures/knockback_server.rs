#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::knockback::apply_player_knockback;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::knockback::player_knockback_velocity;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::knockback::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::KnockbackEntry;

impl Game<'_> {
    /// `LivingEntity.knockback` on the player: pushed `power` away from the
    /// direction (`xd`, `zd`), less its knockback resistance, and up only from
    /// the ground.
    pub(in crate::game) fn knock_back_player(
        &mut self,
        power: f64,
        xd: f64,
        zd: f64,
        source: HurtSource<'_>,
        attack: &Attack,
        _damage: f32,
    ) {
        self.knock_back_player_entry(power, xd, zd, source, attack, KnockbackEntry::Short);
    }

    /// Full living knockback entry. Effect pushes enter here directly.
    pub(in crate::game) fn knock_back_player_full(
        &mut self,
        power: f64,
        xd: f64,
        zd: f64,
        source: HurtSource<'_>,
        attack: &Attack,
        _damage: f32,
        _comes_from_effect: bool,
    ) {
        self.knock_back_player_entry(power, xd, zd, source, attack, KnockbackEntry::Full);
    }

    /// The player's push, entered through `entry`: what `living_knockback` tells its listener.
    pub(in crate::game) fn knock_back_player_entry(
        &mut self,
        power: f64,
        xd: f64,
        zd: f64,
        source: HurtSource<'_>,
        attack: &Attack,
        entry: KnockbackEntry,
    ) {
        // No attacker named: the mob the player was last hurt by.
        let credited;
        let attack = if attack.attacker == 0
            && let Some((Who::Mob(id), _)) = self.player_hurt_by()
        {
            credited = self.attack_of(id as i32);
            &credited
        } else {
            attack
        };
        let plan = self.prepare_knockback_credit(
            self.player.as_ref().map_or(0, |p| p.entity_id),
            None,
            attack,
        );
        let mut credit = plan.lend(&mut self.server.last_hurt_by_player);
        if let Some(p) = self.player.as_mut() {
            let level = crate::game::entity::damage_level_of!(self, attack, &mut credit, None);
            let mut push = |level: &mut sunrise_client::health::DamageLevel<'_>| {
                apply_player_knockback(p, level, power, (xd, zd), source);
            };
            match entry {
                KnockbackEntry::Short => level.through_short_knockback(push),
                KnockbackEntry::Full => push(level),
            }
        }
    }
}
