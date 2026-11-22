//! Single-source-of-truth pickable target query and non-mob projectile hit dispatch.
use super::super::{Game, commands::Target, entity::Entity as _, minecarts::CartHit};
use sunrise_client::join_seq::ListStamp;
use sunrise_client::mobs::{Species, Who};
use sunrise_client::projectiles::{Projectile, ProjectileKind};
use sunrise_core::geometry::{Aabb, Vec3};

/// Per-projectile target filters (`canHitEntity` overrides).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::game) struct TargetFilter {
    pub skip_no_physics: bool,
    pub skip_crystals: bool,
}

impl TargetFilter {
    /// `AbstractHurtingProjectile` / `ShulkerBullet` skip `noPhysics`; wind charges skip crystals.
    pub(in crate::game) fn for_projectile(kind: &ProjectileKind) -> Self {
        Self {
            skip_no_physics: projectile_skips_no_physics(kind),
            skip_crystals: matches!(
                kind,
                ProjectileKind::WindCharge | ProjectileKind::BreezeWindCharge
            ),
        }
    }
}

/// `AbstractHurtingProjectile.canHitEntity` / `ShulkerBullet.canHitEntity`: skip `noPhysics`.
fn projectile_skips_no_physics(kind: &ProjectileKind) -> bool {
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

/// `LeashFenceKnotEntity.recalculateBoundingBox` around `position` (`(x+0.5, y+0.375, z+0.5)`).
pub(in crate::game) fn knot_bounds(position: Vec3) -> Aabb {
    let half = 0.375 / 2.0;
    Aabb::new(
        Vec3::new(position.x - half, position.y, position.z - half),
        Vec3::new(position.x + half, position.y + 0.5, position.z + half),
    )
}

fn extents((lo, hi): ([f64; 3], [f64; 3])) -> Aabb {
    Aabb::new(
        Vec3::new(lo[0], lo[1], lo[2]),
        Vec3::new(hi[0], hi[1], hi[2]),
    )
}

/// Write stamps and positions read by projectile hit tests, to detect mid-loop hook writes.
#[derive(PartialEq)]
pub(in crate::game) struct TargetsRead {
    lists: [ListStamp; 12],
    dragon: Option<[Aabb; 8]>,
    player: Option<Aabb>,
}

/// Entity target snapshots built once for the projectile loop and rebuilt on hook writes.
pub(in crate::game) struct ProjectileTargets {
    /// All pickable targets (`Entity.canBeHitByProjectile`).
    pub targets: Vec<(Who, Aabb)>,
    /// Pickable targets excluding `noPhysics` (`AbstractHurtingProjectile` / `ShulkerBullet`).
    pub physics_targets: Vec<(Who, Aabb)>,
    /// Pickable targets excluding `noPhysics` and End crystals (`AbstractWindCharge.canHitEntity`).
    pub wind_targets: Vec<(Who, Aabb)>,
    /// LAN guests a projectile can hit.
    pub guests: Vec<(Who, Aabb)>,
    /// `targets` plus `guests`, when there are guests.
    pub targets_guests: Vec<(Who, Aabb)>,
    /// `physics_targets` plus `guests`, when there are guests.
    pub physics_targets_guests: Vec<(Who, Aabb)>,
    /// `wind_targets` plus `guests`, when there are guests.
    pub wind_targets_guests: Vec<(Who, Aabb)>,
    pub has_guests: bool,
    /// Breezes that deflect projectiles.
    pub breezes: Vec<u32>,
    /// Every guest by `(conn, entity_id)`.
    pub guest_entities: Vec<(u32, u32)>,
    pub host_dead: bool,
    pub dead_guests: Vec<u32>,
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

impl Game<'_> {
    /// Exact vanilla damage type id for `shot`, honoring `unattributed_fireball` when unowned.
    pub(in crate::game) fn projectile_damage_type(&self, shot: &Projectile) -> &'static str {
        match shot.kind {
            ProjectileKind::Arrow { .. } => "minecraft:arrow",
            ProjectileKind::Trident => "minecraft:trident",
            ProjectileKind::SmallFireball | ProjectileKind::LargeFireball { .. } => {
                if shot.owner.is_some() {
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
                if shot.owner.is_some() {
                    "minecraft:wither_skull"
                } else {
                    "minecraft:magic"
                }
            }
            _ => "minecraft:thrown",
        }
    }

    /// Snapshot all entity target lists for the projectile tick loop.
    pub(in crate::game) fn projectile_tick_targets(&self) -> ProjectileTargets {
        let targets = self.projectile_targets_for(TargetFilter::default());
        let physics_targets = self.projectile_targets_for(TargetFilter {
            skip_no_physics: true,
            skip_crystals: false,
        });
        let wind_targets = self.projectile_targets_for(TargetFilter {
            skip_no_physics: true,
            skip_crystals: true,
        });
        let breezes = self
            .mobs
            .iter()
            .filter(|m| m.alive() && m.species() == Species::Breeze)
            .map(|m| m.id)
            .collect();
        // Mobs' projectiles also hit the guests (game/network/lan_mobs.rs).
        let guests = self.lan_projectile_targets();
        let has_guests = !guests.is_empty();
        let (targets_guests, physics_targets_guests, wind_targets_guests) = if has_guests {
            let mut tg = targets.clone();
            tg.extend_from_slice(&guests);
            let mut ptg = physics_targets.clone();
            ptg.extend_from_slice(&guests);
            let mut wtg = wind_targets.clone();
            wtg.extend_from_slice(&guests);
            (tg, ptg, wtg)
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };
        let dead_guests = self.lan.as_ref().map_or_else(Vec::new, |h| {
            h.hub
                .guests
                .iter()
                .filter(|g| g.dead)
                .map(|g| g.conn)
                .collect()
        });
        ProjectileTargets {
            targets,
            physics_targets,
            wind_targets,
            guests,
            targets_guests,
            physics_targets_guests,
            wind_targets_guests,
            has_guests,
            breezes,
            guest_entities: self.lan_guest_entities(),
            host_dead: self.player_view().is_some_and(|p| !p.alive),
            dead_guests,
        }
    }

    /// Write stamps of every entity list read by `projectile_targets_for`.
    pub(in crate::game) fn projectile_targets_read(&self) -> TargetsRead {
        TargetsRead {
            lists: [
                self.mobs.stamp(),
                self.boats.stamp(),
                self.carts.stamp(),
                self.armor_stands.stamp(),
                self.tnts.stamp(),
                self.falling.stamp(),
                self.item_frames.stamp(),
                self.paintings.stamp(),
                self.cushions.stamp(),
                self.crystals.stamp(),
                self.resident_entities.knots.stamp(),
                self.others.stamp(),
            ],
            dragon: self.dragon_part_boxes(),
            player: self
                .player_view()
                .filter(|p| p.alive && !self.spectator())
                .map(|p| p.bounds),
        }
    }

    /// Whether a hook changed any list or target box read since `read` and `seen`.
    pub(in crate::game) fn projectile_targets_changed(
        &self,
        read: &TargetsRead,
        seen: &ProjectileTargets,
    ) -> bool {
        self.projectile_targets_read() != *read || !self.lan_projectile_targets_are(&seen.guests)
    }

    /// All non-mob pickable entities (`Entity::can_be_hit_by_projectile` plus `BlockAttachedEntity`).
    pub(in crate::game) fn pickable_object_targets(&self) -> Vec<(Who, Aabb)> {
        self.pickable_object_targets_for(false)
    }

    /// Non-mob pickable entities, optionally excluding `noPhysics` entities.
    pub(in crate::game) fn pickable_object_targets_for(
        &self,
        skip_no_physics: bool,
    ) -> Vec<(Who, Aabb)> {
        fn allows<E: crate::game::entity::Entity>(
            game: &Game<'_>,
            entity: &E,
            skip_no_physics: bool,
        ) -> bool {
            entity.can_be_hit_by_projectile()
                && (!skip_no_physics
                    || !entity.no_physics(game.persist.no_physics.get(&entity.id()).copied()))
        }
        let mut targets = Vec::new();
        for b in self.boats.iter() {
            if allows(self, b, skip_no_physics) {
                targets.push((Who::Mob(b.id), b.bounds()));
            }
        }
        for c in self.carts.iter() {
            if allows(self, c, skip_no_physics) {
                targets.push((Who::Mob(c.id), c.bounds()));
            }
        }
        for a in self.armor_stands.iter() {
            if allows(self, a, skip_no_physics) {
                targets.push((Who::Mob(a.id), a.bounds()));
            }
        }
        for t in self.tnts.iter() {
            if allows(self, t, skip_no_physics) {
                targets.push((Who::Mob(t.id), t.bounds()));
            }
        }
        for f in self.falling.iter() {
            if allows(self, f, skip_no_physics) {
                targets.push((Who::Mob(f.id), f.bounds()));
            }
        }
        // Off-walk `BlockAttachedEntity` subclasses (`isPickable() == true`).
        let attached_allows = |id: u32| {
            !skip_no_physics || !self.persist.no_physics.get(&id).copied().unwrap_or(false)
        };
        for f in self.item_frames.iter() {
            if attached_allows(f.id) {
                targets.push((Who::Mob(f.id), extents(f.bounds())));
            }
        }
        for p in self.paintings.iter() {
            if attached_allows(p.id) {
                targets.push((Who::Mob(p.id), extents(p.bounds())));
            }
        }
        for c in self.cushions.iter() {
            if attached_allows(c.id) {
                targets.push((Who::Mob(c.id), extents(c.bounds())));
            }
        }
        for k in self.resident_entities.knots.iter() {
            if attached_allows(k.id) {
                let pos = sunrise_client::mobs::leash::knot_state(k.fence).position;
                targets.push((Who::Mob(k.id), knot_bounds(pos)));
            }
        }
        for o in self.others.iter() {
            if o.kind == "minecraft:leash_knot" && attached_allows(o.id) {
                targets.push((Who::Mob(o.id), knot_bounds(o.position)));
            }
        }
        targets
    }

    /// All entities a projectile can strike (`Entity.canBeHitByProjectile`).
    pub(in crate::game) fn projectile_targets(&self) -> Vec<(Who, Aabb)> {
        self.projectile_targets_for(TargetFilter::default())
    }

    /// All entities a projectile can strike, applying per-projectile `canHitEntity` filters.
    pub(in crate::game) fn projectile_targets_for(&self, filter: TargetFilter) -> Vec<(Who, Aabb)> {
        let mut targets: Vec<(Who, Aabb)> = self
            .mobs
            .iter()
            .filter(|m| {
                m.can_be_hit_by_projectile() && (!filter.skip_no_physics || !m.no_physics(None))
            })
            .map(|m| (Who::Mob(m.id), m.bounds()))
            .collect();
        if let Some(p) = self.player_view()
            && p.alive
            && !self.spectator()
        {
            targets.push((Who::Player, p.bounds));
        }
        targets.extend(
            self.special_targets()
                .into_iter()
                .filter(|(id, _)| !filter.skip_crystals || *id < crate::game::dragon::CRYSTAL_IDS)
                .map(|(id, b)| (Who::Mob(id), b)),
        );
        targets.extend(self.pickable_object_targets_for(filter.skip_no_physics));
        targets
    }

    /// `FishingHook.canHitEntity`: `Projectile.canHitEntity` plus alive `ItemEntity`.
    pub(in crate::game) fn fishing_hook_targets(
        &self,
        owner_conn: Option<u32>,
        own_vehicle: Option<u32>,
    ) -> Vec<(u32, Aabb)> {
        let mut targets: Vec<(u32, Aabb)> = self
            .projectile_targets()
            .into_iter()
            .filter_map(|(who, bounds)| match who {
                Who::Player => None,
                Who::Mob(id) if id == 0 || Some(id) == own_vehicle => None,
                Who::Mob(id) => Some((id, bounds)),
            })
            .collect();
        targets.extend(
            self.creatures
                .hook_targets
                .iter()
                .filter(|t| t.conn != owner_conn)
                .map(|t| (t.id, t.bounds)),
        );
        targets.extend(
            self.dropped
                .iter()
                .filter(|i| i.health > 0)
                .map(|i| (i.id, i.bounds())),
        );
        targets
    }

    /// Resolve a projectile's causing target, creative flag, and entity id.
    fn projectile_source_info(
        &self,
        info: &ProjectileHitInfo,
    ) -> (Option<Target>, bool, bool, i32) {
        let causing_target = match info.owner {
            Some(Who::Player) => Some(info.guest.map_or(Target::Player, Target::Guest)),
            Some(Who::Mob(id)) => Some(Target::Mob(id)),
            None => None,
        };
        let by_player = info.owner == Some(Who::Player);
        let creative = match info.owner {
            Some(Who::Player) => match info.guest {
                Some(conn) if self.lan_acting == Some(conn) => self.creative,
                Some(conn) => self.command_guest(conn).is_some_and(|g| g.creative()),
                None => self.creative,
            },
            _ => false,
        };
        let causing_id = match info.owner {
            Some(Who::Player) => match info.guest {
                Some(conn) => self.command_guest(conn).map_or(0, |g| g.entity_id),
                None => self.own_entity_id(),
            },
            Some(Who::Mob(id)) => id as i32,
            None => 0,
        };
        (causing_target, by_player, creative, causing_id)
    }

    /// Whether `Entity.isInvulnerableToBase` / native hook refuses a projectile hit on `id`.
    fn object_refuses_projectile(
        &mut self,
        info: &ProjectileHitInfo,
        id: u32,
        kind: &str,
        causing: i32,
        creative: bool,
    ) -> bool {
        let source = sunrise_client::health::HurtSource {
            damage_type: info.damage_type,
            causing,
            direct: info.direct_id as i32,
            position: Some([info.position.x, info.position.y, info.position.z]),
        };
        let vanilla = self.saved_refuses_damage(id, info.damage_type, creative);
        crate::game::entity::damage_level_of!(self, self.combat.attack(), no_push, None)
            .invulnerability_check(
                sunrise_client::health::Hurting::Entity(id as i32, kind, source),
                vanilla,
            )
    }

    /// Unified `hurtServer` dispatch for every non-mob pickable target.
    pub(in crate::game) fn projectile_hurts_non_mob(
        &mut self,
        info: &ProjectileHitInfo,
        id: u32,
    ) -> NonMobHit {
        let (causing_target, by_player, creative, causing_id) = self.projectile_source_info(info);
        let mob_blocked = !self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::MOB_GRIEFING)
            && matches!(causing_target, Some(Target::Mob(_)));

        // ArmorStand.hurtServer (with pre-hurt ignition & restore on refusal for fire projectiles).
        if let Some(i) = self.armor_stands.iter().position(|a| a.id == id) {
            let prev_fire = self.armor_stands[i].fire_ticks;
            if let Some(secs) = info.ignite_seconds
                && !self.armor_stands[i].mod_fire_immune
            {
                let ticks = (secs * 20.0) as i32;
                let a = &mut self.armor_stands[i];
                a.fire_ticks = a.fire_ticks.max(ticks);
            }
            let hurt = self.hurt_armor_stand(
                id,
                info.damage_type,
                causing_target,
                info.direct_id,
                Some([info.position.x, info.position.y, info.position.z]),
            );
            if !hurt
                && info.ignite_seconds.is_some()
                && let Some(a) = self.armor_stands.iter_mut().find(|a| a.id == id)
            {
                a.fire_ticks = prev_fire;
            }
            return if hurt {
                NonMobHit::Hurt
            } else {
                NonMobHit::Refused
            };
        }

        // ItemFrame / GlowItemFrame (`ItemFrame.hurtServer`).
        if let Some(i) = self.item_frames.iter().position(|f| f.id == id) {
            let kind = self.item_frames[i].kind();
            if self.object_refuses_projectile(info, id, kind, causing_id, creative) {
                return NonMobHit::Refused;
            }
            let hurt = self.projectile_hurts_frame_by(
                id,
                by_player,
                creative,
                causing_target,
                info.damage_type,
            );
            return if hurt {
                NonMobHit::Hurt
            } else {
                NonMobHit::Refused
            };
        }

        // Painting (`BlockAttachedEntity.hurtServer`).
        if self.paintings.iter().any(|p| p.id == id) {
            if mob_blocked
                || self.object_refuses_projectile(
                    info,
                    id,
                    "minecraft:painting",
                    causing_id,
                    creative,
                )
            {
                return NonMobHit::Refused;
            }
            let Some(i) = self.paintings.iter().position(|p| p.id == id) else {
                return NonMobHit::Refused;
            };
            let broken = self.break_painting_by(i, by_player, causing_target);
            return if broken {
                NonMobHit::Hurt
            } else {
                NonMobHit::Refused
            };
        }

        // Cushion (`Cushion.hurtServer`).
        if self.cushions.iter().any(|c| c.id == id) {
            let may_build = match causing_target {
                Some(Target::Player) => self.may_build(),
                Some(Target::Guest(conn)) if self.lan_acting == Some(conn) => self.may_build(),
                Some(Target::Guest(conn)) => self
                    .command_guest(conn)
                    .is_none_or(|g| g.abilities.may_build),
                _ => true,
            };
            if !may_build
                || mob_blocked
                || self.object_refuses_projectile(
                    info,
                    id,
                    "minecraft:cushion",
                    causing_id,
                    creative,
                )
            {
                return NonMobHit::Refused;
            }
            let Some(i) = self.cushions.iter().position(|c| c.id == id) else {
                return NonMobHit::Refused;
            };
            let broken = self.break_cushion_by(i, causing_target);
            return if broken {
                NonMobHit::Hurt
            } else {
                NonMobHit::Refused
            };
        }

        // LeashFenceKnotEntity (`BlockAttachedEntity.hurtServer`), natural or explicit.
        let is_knot = self.natural_knot(id).is_some()
            || self
                .others
                .iter()
                .any(|o| o.id == id && o.kind == "minecraft:leash_knot");
        if is_knot {
            if mob_blocked
                || self.object_refuses_projectile(
                    info,
                    id,
                    "minecraft:leash_knot",
                    causing_id,
                    creative,
                )
            {
                return NonMobHit::Refused;
            }
            let broken = self.break_leash_knot(id, causing_target);
            return if broken {
                NonMobHit::Hurt
            } else {
                NonMobHit::Refused
            };
        }

        // FallingBlockEntity (`Entity.hurtServer`: always returns false).
        if self.falling.iter().any(|f| f.id == id) {
            return NonMobHit::Refused;
        }

        // PrimedTnt (`Entity.hurtServer`: always returns false).
        if self.tnts.iter().any(|t| t.id == id) {
            return NonMobHit::Refused;
        }

        // Boat (`VehicleEntity.hurtServer`).
        if let Some(i) = self.boats.iter().position(|b| b.id == id) {
            let kind = self.boats[i].entity_type();
            if self.object_refuses_projectile(info, id, &kind, causing_id, creative) {
                return NonMobHit::Refused;
            }
            let Some(i) = self.boats.iter().position(|b| b.id == id) else {
                return NonMobHit::Refused;
            };
            self.hurt_boat(i, info.amount, creative);
            return NonMobHit::Hurt;
        }

        // Minecart (`VehicleEntity.hurtServer`).
        if let Some(i) = self.carts.iter().position(|c| c.id == id) {
            let kind = self.carts[i].kind.id();
            if self.object_refuses_projectile(info, id, kind, causing_id, creative) {
                return NonMobHit::Refused;
            }
            let Some(i) = self.carts.iter().position(|c| c.id == id) else {
                return NonMobHit::Refused;
            };
            if info.is_arrow && info.damage_type == "minecraft:arrow" {
                self.arrow_hits_cart(
                    i,
                    info.amount,
                    info.velocity,
                    info.burning_arrow,
                    creative,
                    info.owner,
                    info.guest,
                );
            } else {
                let hit = if info.damage_type == "minecraft:trident" {
                    let by = match info.owner {
                        Some(Who::Player) => Some(info.guest.map_or(Target::Player, Target::Guest)),
                        Some(Who::Mob(m)) => Some(Target::Mob(m)),
                        None => None,
                    };
                    CartHit::attributed(by)
                } else {
                    CartHit::projectile(info.owner, info.guest)
                };
                self.hurt_cart(i, info.amount, creative, false, hit);
            }
            return NonMobHit::Hurt;
        }

        NonMobHit::NotObject
    }
}
