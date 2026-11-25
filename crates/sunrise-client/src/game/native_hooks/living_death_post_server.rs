#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::living_death_post::*;

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::super::super::native_entity_refs::tests::with_game;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::living_death_post::tests::Observer;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::living_death_post::tests::Post;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::living_death_post::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use mod_api::{
        Flow, Hooks, LivingDeathEvent, LivingDeathPostEvent, LivingDropsEvent,
        LivingExperienceDropEvent, MobKilledEvent, ModEntry, ServerHost,
    };
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_client::mobs::{DamageKind, Mob};
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;

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
