use super::*;
use sunrise_gameplay_common::game::creatures::knockback::apply_player_knockback;
use mod_api::KnockbackEntry;

impl crate::game::ServerGame<'_> {
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
            self.common.player.as_ref().map_or(0, |p| p.entity_id),
            None,
            attack,
        );
        let mut credit = plan.lend(&mut self.server.last_hurt_by_player);
        if let Some(p) = self.common.player.as_mut() {
            let level = crate::game::server_side::entity_server::damage_level_of!(
                self,
                attack,
                &mut credit,
                None
            );
            let mut push = |level: &mut sunrise_sim::health::DamageLevel<'_>| {
                apply_player_knockback(p, level, power, (xd, zd), source);
            };
            match entry {
                KnockbackEntry::Short => level.through_short_knockback(push),
                KnockbackEntry::Full => push(level),
            }
        }
    }
}
