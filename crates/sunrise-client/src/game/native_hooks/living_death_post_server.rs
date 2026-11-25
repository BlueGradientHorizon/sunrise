#[cfg(test)]
use super::*;

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;
    use sunrise_client::mobs::Mob;
    use sunrise_core::geometry::Vec3;

    /// A dead sheep: its table always drops at least one mutton, so the
    /// ordering against the drop list does not wait for a lucky roll.
    pub(in crate::game) fn sheep(game: &mut crate::game::ServerGame<'_>, damage: Damage) -> Mob {
        game.side.next_entity += 1;
        let kind = game.res.mob_types().get("minecraft:sheep").unwrap().clone();
        let mut mob = Mob::new(game.side.next_entity, kind, Vec3::new(0.5, 64.0, 0.5), 0.0);
        mob.health = 0.0;
        mob.last_damage = Some((damage, 0));
        game.mobs.push(mob.clone());
        mob
    }
}
