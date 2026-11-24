#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game, Hit, dragon::FightWho, item_draw::owner_at, view_vector,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fireworks::FIREWORK_EXPLODE_EVENT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fireworks::Rocket;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fireworks::blast_damage;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fireworks::fireworks_of;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fireworks::rocket_draws;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fireworks::use_on_passes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fireworks::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_assets::item_model::DisplayContext;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    block_entity_render::vertex_light,
    collisions::SceneCollisions,
    entity_nbt::RocketNbt,
    items::ItemStack,
    mesher::Vertex,
    mobs::{Damage, Species, Who},
    particles::{FireworkExplosion, ParticleOptions, ParticleType},
    projectiles::{Hit as ProjectileHit, ProjectileWorld},
    random_source,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::RandomSource,
};

#[path = "fireworks/impact_server.rs"]
pub(in crate::game) mod impact;

impl Game<'_> {
    /// `FireworkRocketItem.use` while gliding: drop the player's leads, then a
    /// rocket attached to the player.
    pub(in crate::game) fn boost_firework(&mut self, stack: &ItemStack) -> bool {
        if &*stack.item != "minecraft:firework_rocket"
            || !self.player.as_ref().is_some_and(|p| p.fall_flying)
        {
            return false;
        }
        // Server path of `use`: `dropAllLeashConnections(null)`, and when that
        // drops anything `playSound(null, player, LEAD_BREAK, NEUTRAL, 1, 1)`.
        // The entity overload follows the player and excludes nobody, including
        // the guest whose action is running while its hub is swapped out.
        if self.drop_all_leash_connections() {
            self.play_player_bound_sound("minecraft:item.lead.break", 6, 1.0, 1.0, false);
        }
        let at = self.player.as_ref().map_or(Vec3::ZERO, |p| p.position);
        let mut rocket = self.new_rocket(at, stack, true, &mut random_source::entity());
        rocket.owner = Some(Who::Player);
        rocket.guest = self.server.lan_acting;
        self.rockets.push(rocket);
        if !self.creative {
            self.consume_held();
        }
        self.award_stat("used", "minecraft:firework_rocket", 1);
        self.swing_arm();
        if let (Some(player), Some(name)) = (
            self.server.lan_acting_uuid,
            self.server.lan_acting_name.clone(),
        ) {
            self.native_player_elytra_boost(player, &name);
        }
        true
    }
    /// A rocket shot at an angle (`DATA_SHOT_AT_ANGLE`): out of a dispenser
    /// (`FireworkRocketItem.asProjectile`) or a crossbow.
    ///
    /// `shoot` gives the shot's velocity from the rocket's own random, after the
    /// constructor's draws (`asProjectile`, then `spawnProjectileUsingShoot`).
    pub(in crate::game) fn launch_angled_rocket(
        &mut self,
        at: Vec3,
        stack: &ItemStack,
        owner: Option<Who>,
        shoot: impl FnOnce(&mut sunrise_core::random::LegacyRandom) -> Vec3,
    ) {
        let mut own = random_source::entity();
        let mut rocket = self.new_rocket(at, stack, false, &mut own);
        rocket.velocity = shoot(&mut own);
        rocket.turn_to_shot();
        rocket.angled = true;
        rocket.owner = owner;
        rocket.guest = self
            .server
            .lan_acting
            .filter(|_| owner == Some(Who::Player));
        self.rockets.push(rocket);
    }
    /// `FireworkRocketItem.useOn`: launch from the clicked point.
    pub(in crate::game) fn launch_firework(&mut self, hit: &Hit, stack: &ItemStack) -> bool {
        if &*stack.item != "minecraft:firework_rocket"
            || use_on_passes(self.player.as_ref().is_some_and(|p| p.fall_flying))
        {
            return false;
        }
        let at = Vec3::new(
            hit.point[0] + f64::from(hit.face[0]) * 0.15,
            hit.point[1] + f64::from(hit.face[1]) * 0.15,
            hit.point[2] + f64::from(hit.face[2]) * 0.15,
        );
        let mut rocket = self.new_rocket(at, stack, false, &mut random_source::entity());
        rocket.owner = Some(Who::Player);
        rocket.guest = self.server.lan_acting;
        self.rockets.push(rocket);
        if !self.creative {
            self.consume_held();
        }
        // `ItemStack.useOn` awards ITEM_USED for `FireworkRocketItem.useOn`'s SUCCESS.
        self.award_stat("used", "minecraft:firework_rocket", 1);
        self.swing_arm();
        true
    }
    /// `FireworkRocketEntity.explode`'s game event: EXPLODE at the rocket, from
    /// its owner (`Projectile.getOwner`), a guest's rocket as theirs.
    pub(in crate::game) fn rocket_explode_event(
        &mut self,
        mut owner: Option<Who>,
        guest: Option<u32>,
        at: [f64; 3],
    ) {
        if let Some(conn) = guest.filter(|conn| self.server.lan_acting != Some(*conn)) {
            let posted = self
                .as_guest(conn, |g| {
                    let ctx = g.who_context(owner, None);
                    g.post_game_event("minecraft:explode", at, ctx);
                })
                .is_some();
            if posted {
                return;
            }
            // An owner who left cannot resolve to the host: no source entity.
            owner = None;
        }
        let ctx = self.who_context(owner, None);
        self.post_game_event("minecraft:explode", at, ctx);
    }
    pub(in crate::game) fn rocket_hurt_mobs(
        &mut self,
        hits: &[(u32, f32)],
        rocket_id: u32,
        attacker: Option<Who>,
        at: [f64; 3],
    ) {
        let mut attack = self.attack_of_who(attacker);
        attack.projectile = Some(rocket_id as i32);
        attack.projectile_kind = sunrise_client::health::DirectEntity::Projectile;
        attack.projectile_type = Some("minecraft:firework_rocket");
        for &(id, amount) in hits {
            let plan = self.prepare_knockback_credit(id as i32, attacker, &attack);
            // The hit's causing entity, lent to its listeners beside the victim.
            let causing = self.causer_id(attacker);
            let decision = crate::game::entity::causer_decision!(self, causing);
            let mut credit = plan.lend(&mut self.server.last_hurt_by_player);
            if let Some(index) = self.mob_index.find(&self.mobs, id) {
                let before = self.mobs[index].health;
                let join = self.mobs[index].join;
                let (victim, lend) =
                    crate::game::entity::victim_and_causer!(self, index, causing, decision);
                let hurt = victim.damage(
                    crate::game::entity::damage_level_of!(self, &attack, &mut credit, lend),
                    amount,
                    Damage::fireworks(attacker, at),
                );
                let Some((index, hurt)) = self.complete_mob_hurt(index, hurt) else {
                    continue;
                };
                // LivingEntity.actuallyHurt posts after health loss, before death settlement.
                if self.mobs[index].health < before {
                    let pos = self.mobs[index].position.components();
                    let ctx = self.who_context(Some(Who::Mob(id)), None);
                    self.post_game_event("minecraft:entity_damage", pos, ctx);
                }
                // LivingEntity.hurtServer: a player's rocket makes them the
                // last player to hurt it (`setLastHurtByPlayer`), so its death
                // message names them and their item.
                if let Some(index) = self.mob_index.find(&self.mobs, id)
                    && self.mobs[index].join == join
                {
                    if hurt && attacker == Some(Who::Player) {
                        self.remember_mob_player_credit(id);
                    }
                    self.finish_mob_damage_at(index);
                }
            }
            self.settle_mob_hurt(id);
        }
    }
    /// `hurtServer` with `damageSources().fireworks` from the rocket at
    /// `from`: the rocket is the direct entity, so the player is knocked away
    /// from it and the death message names it; creative and spectator
    /// players are immune.
    pub(in crate::game) fn rocket_hurt_player(&mut self, from: Vec3, amount: f32) {
        // Ability, spectator and death refusals belong to hurtServer, after
        // its invulnerability observation (also for an attached target).
        // No mob ever owns a rocket.
        self.combat.0.projectile_owner = None;
        self.combat.0.rocket_hit = true;
        self.combat.0.projectile_direct =
            Some(Text::translate("entity.minecraft.firework_rocket", &[]));
        let at = [from.x, from.y, from.z];
        self.hurt_player_from(amount, Some(at), "minecraft:fireworks", Some(None), false);
    }
    /// `Entity.move` after a free rocket's move (`from`, the move it asked
    /// for, the one it made): a move of a block or more casts for a reset
    /// block or water, `checkFallDamage` adds the drop outside water and on
    /// the ground posts HIT_GROUND after a fall and resets it; then
    /// `applyEffectsFromBlocks`: a cobweb or powder snow makes it stuck and
    /// resets the fall, as does a bubble column (whose push the rocket's
    /// `setDeltaMovement(movement2)` undoes).
    pub(in crate::game) fn rocket_moved(
        &mut self,
        r: &mut Rocket,
        (from, wanted, moved, restored): (Vec3, Vec3, Vec3, Vec3),
        water: bool,
    ) {
        let movement = r.flight_contact(self.body_motion(&game_side!(self)), wanted, moved);
        r.fall_distance = self.vehicle_fall_reset(r.fall_distance, from, moved);
        if !water && moved.y < 0.0 {
            r.fall_distance -= f64::from(moved.y as f32);
        }
        if r.ground.on_ground {
            if r.fall_distance > 0.0 {
                // Block.fallOn (farmland's random draw, honey's slide) is not
                // run for a rocket yet, nor for the other entities that are
                // not living.
                let p = r.position;
                let state = self.landing_state(p, r.bounds());
                let at = [p.x, p.y, p.z];
                self.projectile_own_event(
                    "minecraft:hit_ground",
                    r.owner,
                    r.guest,
                    at,
                    Some(state),
                );
            }
            r.fall_distance = 0.0;
        }
        let entity = (r.id, "minecraft:firework_rocket");
        let response = self.body_motion(&game_side!(self)).restitute(
            entity,
            movement,
            0.0,
            0.98_f32,
            r.velocity,
            r.fall_distance,
        );
        r.velocity = self.finish_body_restitution(entity, r.position, response);
        let side = game_side!(self);
        let motion = self.body_motion(&side);
        motion.apply_block_speed(r.position, &mut r.velocity);
        let writes =
            self.body_step_on(entity, (r.position, r.bounds()), &r.ground, &mut r.velocity);
        super::entity::apply_contact_writes(
            writes,
            &mut r.velocity,
            Some(&mut r.fall_distance),
            Some(&mut r.stuck),
            None,
        );
        let length = moved.length_squared();
        let recorded = length > 1.0e-7 || wanted.length_squared() - length < 1.0e-7;
        let (world, facts) = (&self.world, self.res.facts());
        let (registry, collision) = (self.pack.assets.registry, self.collision);
        let mut tops = Vec::new();
        let rocket = Rocket::bounds_at;
        let effects = sunrise_client::falling_block::Effects {
            bounds: rocket,
            from,
            to: r.position,
            original: recorded.then_some(wanted),
        };
        let stuck = sunrise_client::falling_block::apply_effects_from_blocks_of(
            effects,
            &mut r.velocity,
            &mut r.fall_distance,
            &mut tops,
            |p| {
                use sunrise_client::falling_block::Inside;
                match &*facts.get(world.block(p.x, p.y, p.z)).name {
                    "minecraft:cobweb" => Inside::Cobweb,
                    "minecraft:powder_snow" => Inside::PowderSnow,
                    _ => sunrise_client::motion::bubble_column_at(
                        world, registry, facts, collision, p,
                    )
                    .map_or(Inside::Nothing, Inside::BubbleColumn),
                }
            },
        );
        if let Some(multiplier) = stuck {
            r.stuck = multiplier;
        }
        for pos in tops {
            self.bubble_column_particles(pos);
        }
        // FireworkRocketEntity.tick restores the pre-move delta after block effects.
        r.velocity = restored;
    }
}

#[cfg(test)]
#[path = "fireworks/hit_attempt_tests_server.rs"]
pub(in crate::game) mod hit_attempt_tests;
