//! Real skull, rocket, command and zero-damage projectile entries.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::native_entity_refs::tests::{bind_local_player, with_game};
#[allow(unused_imports)]
use mod_api::{
    EnderPearlTeleportEvent, EntityInvulnerabilityCheckEvent, Flow, Hooks, LevelSide, ServerHost,
};
#[allow(unused_imports)]
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[allow(unused_imports)]
use sunrise_client::{
    command::args::GameMode,
    motion::PlayerMotion,
    projectiles::{Hit, Projectile, ProjectileKind, ThrownType},
};

pub(in crate::game) struct Probe(pub(in crate::game) Arc<AtomicUsize>);
#[mod_api::__subscriptions]
impl Hooks for Probe {
    fn entity_invulnerability_check(
        &self,
        _: LevelSide<'_>,
        _: &mut EntityInvulnerabilityCheckEvent<'_>,
    ) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
    fn ender_pearl_teleport(
        &self,
        _: &mut dyn ServerHost,
        event: &mut EnderPearlTeleportEvent<'_>,
    ) -> Flow {
        event.damage = 0.0;
        Flow::Continue
    }
}
