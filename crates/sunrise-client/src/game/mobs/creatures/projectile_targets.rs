//! Single-source-of-truth live pickable target query and non-mob projectile hit dispatch.
#[allow(unused_imports)]
use super::super::{
    Game, commands::Target, entity::Entity as _, lan_mobs::guest_bounds, minecarts::CartHit,
};
#[allow(unused_imports)]
use super::ProjWorld;
#[allow(unused_imports)]
use sunrise_client::arrows::ArrowTargets;
#[allow(unused_imports)]
use sunrise_client::mobs::{MobWorld, Who};
#[allow(unused_imports)]
use sunrise_client::projectiles::{
    Projectile, ProjectileKind, ProjectileWorld, TargetCandidate, TargetOrder,
};
#[allow(unused_imports)]
use sunrise_core::{
    geometry::{Aabb, Vec3},
    position::BlockPos,
};

/// Per-projectile target filters (`canHitEntity` overrides).
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::game) struct TargetQuery {
    pub skip_no_physics: bool,
    pub skip_crystals: bool,
    pub skip_wind_charges: bool,
    pub skip_self_id: Option<u32>,
    pub include_guests: bool,
    pub include_items: bool,
    /// `AbstractArrow.canHitEntity` player-vs-player team / `pvp` rule check.
    pub arrow_friendly_fire: Option<(Who, Option<u32>)>,
}

impl TargetQuery {
    /// Build a live target query for a shared-list projectile.
    pub(in crate::game) fn for_projectile(p: &Projectile, include_guests: bool) -> Self {
        let is_wind = matches!(
            p.kind,
            ProjectileKind::WindCharge | ProjectileKind::BreezeWindCharge
        );
        Self {
            skip_no_physics: projectile_skips_no_physics(&p.kind),
            skip_crystals: is_wind,
            skip_wind_charges: is_wind,
            skip_self_id: (p.id != 0).then_some(p.id),
            include_guests,
            include_items: false,
            arrow_friendly_fire: p
                .arrow
                .is_some()
                .then_some(p.owner)
                .flatten()
                .map(|o| (o, p.guest)),
        }
    }
}

/// `AbstractHurtingProjectile.canHitEntity` / `ShulkerBullet.canHitEntity`: skip `noPhysics`.
pub(in crate::game) fn projectile_skips_no_physics(kind: &ProjectileKind) -> bool {
    matches!(
        kind,
        ProjectileKind::SmallFireball
            | ProjectileKind::LargeFireball { .. }
            | ProjectileKind::DragonFireball
            | ProjectileKind::WitherSkull { .. }
            | ProjectileKind::WindCharge
            | ProjectileKind::BreezeWindCharge
            | ProjectileKind::ShulkerBullet { .. }
    )
}

/// Whether `p` is pickable by other projectiles (`#redirectable_projectile` or `ShulkerBullet`).
pub(in crate::game) fn is_pickable_projectile(p: &Projectile) -> bool {
    !p.stuck
        && (super::super::deflect::redirectable(p)
            || matches!(p.kind, ProjectileKind::ShulkerBullet { .. }))
}

/// `LeashFenceKnotEntity.recalculateBoundingBox` around `position` (`(x+0.5, y+0.375, z+0.5)`).
pub(in crate::game) fn knot_bounds(position: Vec3) -> Aabb {
    let half = 0.375 / 2.0;
    Aabb::new(
        Vec3::new(position.x - half, position.y, position.z - half),
        Vec3::new(position.x + half, position.y + 0.5, position.z + half),
    )
}

pub(in crate::game) fn extents((lo, hi): ([f64; 3], [f64; 3])) -> Aabb {
    Aabb::new(
        Vec3::new(lo[0], lo[1], lo[2]),
        Vec3::new(hi[0], hi[1], hi[2]),
    )
}

/// Zero-allocation live world view for `ProjectileWorld` and `ArrowTargets`.
pub(in crate::game) struct LiveProjWorld<'a, 'r> {
    pub game: &'a Game<'r>,
    pub w: MobWorld<'a>,
    pub border: Option<[f64; 4]>,
    pub query: TargetQuery,
    /// Pickable projectiles from a lifted pass when `creatures.projectiles` is temporarily empty.
    pub extra_projectiles: &'a [TargetCandidate],
}

impl<'a, 'r> LiveProjWorld<'a, 'r> {
    pub(in crate::game) fn base(&self) -> ProjWorld<'_, 'a> {
        ProjWorld {
            w: &self.w,
            targets: &[],
            border: self.border,
        }
    }
}

impl ProjectileWorld for LiveProjWorld<'_, '_> {
    fn clip(&self, from: Vec3, to: Vec3) -> Option<(Vec3, [i32; 3], [i32; 3])> {
        self.base().clip(from, to)
    }
    fn in_water(&self, bounds: Aabb) -> bool {
        self.base().in_water(bounds)
    }
    fn in_lava_box(&self, bounds: Aabb) -> bool {
        self.base().in_lava_box(bounds)
    }
    fn pushed_by_currents(&self, bounds: Aabb, velocity: Vec3) -> Vec3 {
        self.base().pushed_by_currents(bounds, velocity)
    }
    fn targets(&self) -> &[(Who, Aabb)] {
        &[]
    }
    fn visit_targets(&self, visit: impl FnMut(TargetCandidate)) {
        self.game
            .visit_projectile_targets(self.query, self.extra_projectiles, visit);
    }
    fn owner_tree_contains(&self, owner: Who, candidate: Who) -> bool {
        self.game.in_same_vehicle_tree(owner, candidate)
    }
    fn outside_owner_collision_range(&self, owner: Who, swept: Aabb) -> bool {
        self.game.outside_owner_collision_range(owner, swept)
    }
    fn target_bounds(&self, target: Who) -> Option<Aabb> {
        self.game.live_target_bounds(target)
    }
    fn below_world_y(&self) -> f64 {
        self.base().below_world_y()
    }
    fn empty(&self, p: [i32; 3]) -> bool {
        self.base().empty(p)
    }
    fn can_stand_on(&self, p: [i32; 3], entity_y: f64) -> bool {
        self.base().can_stand_on(p, entity_y)
    }
    fn ignites(&self, p: [i32; 3]) -> bool {
        self.base().ignites(p)
    }
    fn lava_height(&self, p: BlockPos) -> Option<f64> {
        self.base().lava_height(p)
    }
    fn bubble_column(&self, p: [i32; 3]) -> Option<(bool, bool)> {
        self.base().bubble_column(p)
    }
    fn stuck_speed(&self, p: [i32; 3]) -> Option<Vec3> {
        self.base().stuck_speed(p)
    }
}

impl ArrowTargets for LiveProjWorld<'_, '_> {
    fn visit(&self, visit: impl FnMut(TargetCandidate)) {
        self.game
            .visit_projectile_targets(self.query, self.extra_projectiles, visit);
    }
    fn owner_tree_contains(&self, owner: Who, candidate: Who) -> bool {
        self.game.in_same_vehicle_tree(owner, candidate)
    }
    fn outside_owner_collision_range(&self, owner: Who, swept: Aabb) -> bool {
        self.game.outside_owner_collision_range(owner, swept)
    }
}

/// Context for hurting a non-mob pickable entity from any projectile.
pub(in crate::game) struct ProjectileHitInfo {
    pub direct_id: u32,
    pub owner: Option<Who>,
    pub guest: Option<u32>,
    pub damage_type: &'static str,
    pub amount: f32,
    pub velocity: Vec3,
    pub position: Vec3,
    pub is_arrow: bool,
    pub burning_arrow: bool,
    pub ignite_seconds: Option<f32>,
}

impl ProjectileHitInfo {
    pub(in crate::game) fn from_projectile(
        p: &Projectile,
        damage_type: &'static str,
        amount: f32,
        ignite_seconds: Option<f32>,
    ) -> Self {
        let burning_arrow = p.arrow.as_ref().is_some_and(|a| a.on_fire());
        Self {
            direct_id: p.id,
            owner: p.owner,
            guest: p.guest,
            damage_type,
            amount,
            velocity: p.velocity,
            position: p.position,
            is_arrow: p.arrow.is_some(),
            burning_arrow,
            ignite_seconds,
        }
    }
}

/// Outcome of `Game::projectile_hurts_non_mob`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::game) enum NonMobHit {
    /// Target id does not name a non-mob pickable object (caller handles mob/player/special).
    NotObject,
    /// Non-mob object's `hurtServer` returned true.
    Hurt,
    /// Non-mob object's `hurtServer` returned false (refused, e.g. falling block, fixed frame, TNT).
    Refused,
}

impl<'r> Game<'r> {
    /// Construct a live zero-allocation world view for projectile sweeps.
    pub(in crate::game) fn live_proj_world<'a>(
        &'a self,
        query: TargetQuery,
        extra_projectiles: &'a [TargetCandidate],
    ) -> LiveProjWorld<'a, 'r> {
        LiveProjWorld {
            game: self,
            w: self.mob_world_base(&[]),
            border: Some(self.border.bounds()),
            query,
            extra_projectiles,
        }
    }

    /// Resolve a LAN guest connection id from their entity id.
    pub(in crate::game) fn lan_guest_conn_by_id(&self, id: u32) -> Option<u32> {
        self.lan.as_ref().and_then(|host| {
            host.hub
                .guests
                .iter()
                .find(|g| g.entity_id as u32 == id)
                .map(|g| g.conn)
        })
    }

    /// Exact vanilla damage type id for `shot`, using the live resolved owner.
    pub(in crate::game) fn projectile_damage_type(&self, shot: &Projectile) -> &'static str {
        let has_owner = self
            .resolved_projectile_owner(shot.owner, shot.guest)
            .is_some();
        match shot.kind {
            ProjectileKind::Arrow { .. } => "minecraft:arrow",
            ProjectileKind::Trident => "minecraft:trident",
            ProjectileKind::SmallFireball | ProjectileKind::LargeFireball { .. } => {
                if has_owner {
                    "minecraft:fireball"
                } else {
                    "minecraft:unattributed_fireball"
                }
            }
            ProjectileKind::WindCharge | ProjectileKind::BreezeWindCharge => {
                "minecraft:wind_charge"
            }
            ProjectileKind::LlamaSpit => "minecraft:spit",
            ProjectileKind::ShulkerBullet { .. } => "minecraft:mob_projectile",
            ProjectileKind::WitherSkull { .. } => {
                if has_owner {
                    "minecraft:wither_skull"
                } else {
                    "minecraft:magic"
                }
            }
            _ => "minecraft:thrown",
        }
    }

    /// Immediate vehicle (`Entity.getVehicle`) of `who`, if any.
    pub(in crate::game) fn mount_of(&self, who: Who) -> Option<Who> {
        match who {
            Who::Player => self.riding.map(Who::Mob),
            Who::Mob(id) => self
                .lan
                .as_ref()
                .and_then(|h| h.hub.guests.iter().find(|g| g.entity_id as u32 == id))
                .and_then(|g| g.vehicle)
                .or_else(|| {
                    self.mobs
                        .iter()
                        .find(|m| m.id == id)
                        .and_then(|m| m.extra.mount)
                })
                .or_else(|| {
                    self.armor_stands
                        .iter()
                        .find(|a| a.id == id)
                        .and_then(|a| a.vehicle)
                })
                .or_else(|| self.resident_entities.general.row(id).and_then(|r| r.mount))
                .map(Who::Mob),
        }
    }

    /// `Entity.getRootVehicle`: climb vehicle pointers to the root entity.
    pub(in crate::game) fn root_vehicle_of(&self, mut who: Who) -> Who {
        for _ in 0..16 {
            let Some(next) = self.mount_of(who) else {
                break;
            };
            if next == who {
                break;
            }
            who = next;
        }
        who
    }

    /// `Entity.isPassengerOfSameVehicle`: whether `a` and `b` share the same root vehicle tree.
    pub(in crate::game) fn in_same_vehicle_tree(&self, a: Who, b: Who) -> bool {
        a == b || self.root_vehicle_of(a) == self.root_vehicle_of(b)
    }

    /// `Projectile.isOutsideOwnerCollisionRange`: true when `swept` touches no pickable entity
    /// in `owner.getRootVehicle().getSelfAndPassengers()`.
    pub(in crate::game) fn outside_owner_collision_range(&self, owner: Who, swept: Aabb) -> bool {
        let root = self.root_vehicle_of(owner);
        let mut outside = true;
        self.visit_projectile_targets(
            TargetQuery {
                include_guests: true,
                ..TargetQuery::default()
            },
            &[],
            |c| {
                if outside && c.bounds.intersects(swept) && self.root_vehicle_of(c.who) == root {
                    outside = false;
                }
            },
        );
        outside
    }

    /// Current bounding box of a live target (for `ShulkerBullet` homing).
    pub(in crate::game) fn live_target_bounds(&self, target: Who) -> Option<Aabb> {
        match target {
            Who::Player => self
                .player_view()
                .filter(|p| p.alive && !self.spectator())
                .map(|p| p.bounds),
            Who::Mob(id) => {
                if let Some(m) = self
                    .mobs
                    .iter()
                    .find(|m| m.id == id && m.alive() && !m.removed)
                {
                    return Some(m.bounds());
                }
                if let Some(host) = self.lan.as_ref()
                    && let Some(g) = host.hub.guests.iter().find(|g| {
                        g.entity_id as u32 == id
                            && host.hub.here(g)
                            && g.loaded
                            && !g.dead
                            && !g.spectator()
                    })
                {
                    return Some(guest_bounds(g));
                }
                if let Some(c) = self.crystals.iter().find(|c| c.id == id) {
                    return Some(c.bounds());
                }
                if id >= super::super::dragon::SPECIAL {
                    let part = (id - super::super::dragon::SPECIAL) as usize;
                    return self
                        .dragon_part_boxes()
                        .and_then(|boxes| boxes.get(part).copied());
                }
                if let Some(a) = self.armor_stands.iter().find(|a| a.id == id) {
                    return Some(a.bounds());
                }
                if let Some(b) = self.boats.iter().find(|b| b.id == id) {
                    return Some(b.bounds());
                }
                if let Some(c) = self.carts.iter().find(|c| c.id == id) {
                    return Some(c.bounds());
                }
                None
            }
        }
    }

    /// `Projectile.onRedirectProjectile` (`AIM_DEFLECT`, or `MOMENTUM_DEFLECT` for `ShulkerBullet`).
    pub(in crate::game) fn redirect_projectile(
        &mut self,
        target_id: u32,
        hitter_owner: Option<Who>,
        hitter_owner_uuid: Option<[i32; 4]>,
        hitter_guest: Option<u32>,
        momentum: Option<Vec3>,
    ) {
        let Some(j) = self
            .creatures
            .projectiles
            .iter()
            .position(|q| q.id == target_id && super::super::deflect::redirectable(q))
        else {
            return;
        };
        let dir = match momentum {
            Some(v) => (v.length_squared() > 0.0).then(|| v.normalize()),
            None => self.owner_look(hitter_owner, hitter_guest),
        };
        self.creatures.projectiles.update(j, |target| {
            target.deflect(
                sunrise_client::projectiles::Turn::Direction(dir),
                [1.0; 3],
                (hitter_owner, hitter_owner_uuid, hitter_guest),
                true,
            );
        });
    }
}
