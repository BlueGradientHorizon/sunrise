#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::native_entity_refs::tests::{bind_local_player, run_fresh_cases, with_game};
#[allow(unused_imports)]
use mod_api::{EntityRef, LivingRef};
#[allow(unused_imports)]
use sunrise_client::{chunk::Chunk, motion::PlayerMotion};

pub(in crate::game) const PLAYER: u32 = 20;
pub(in crate::game) const PIG: u32 = 71;
pub(in crate::game) const MOB: u32 = 72;
pub(in crate::game) const WOLF: u32 = 73;
pub(in crate::game) const PLAYER_AGE: i32 = 39;
pub(in crate::game) const MOB_AGE: u32 = 117;

pub(in crate::game) fn living(game: &crate::game::ServerGame<'_>, id: u32) -> LivingRef {
    game.living(game.entity(id).expect("admitted entity"))
        .expect("living entity")
}

pub(in crate::game) fn add_mob(game: &mut crate::game::ServerGame<'_>, id: u32, kind: &str, x: f64) {
    let kind = game.res.mob_types().get(kind).unwrap().clone();
    let mut mob = Mob::new(id, kind, Vec3::new(x, 80.0, 0.5), 0.0);
    mob.no_ai = true;
    mob.no_gravity = true;
    mob.ground.on_ground = true;
    game.mobs.push(mob);
}

pub(in crate::game) fn shoot(game: &mut crate::game::ServerGame<'_>, owner: Who, kind: ProjectileKind) {
    let at = game.mobs[0].position;
    let mut shot = Projectile::new(kind, at, Some(owner));
    shot.velocity = Vec3::new(0.0, 0.0, 2.0);
    shot.id = 77;
    game.apply_projectile_hit(
        &shot,
        Hit::Entity {
            who: Who::Mob(PIG),
            at,
        },
    )
    .unwrap();
}

pub(in crate::game) fn arrow() -> ProjectileKind {
    ProjectileKind::Arrow {
        base_damage: 2.0,
        critical: false,
        effect: None,
    }
}
