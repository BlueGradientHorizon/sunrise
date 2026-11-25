#[allow(unused_imports)]
pub(in crate::game::client_side) use super::api_values::{TryIntoApi, TryIntoEngine};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::level_sounds::LevelSound;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::piston_push::Lid;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Dim, Game, InteractTarget,
    entity_culling::EntityCullDecision,
    native_mob_draw::PlayerRender,
    native_projectiles::{Impact, PendingImpact, ProjectileHit, projectile_impact_event},
    view_vector,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::Creatures;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::DyingDrops;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::Finalized;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::MobDrawFrame;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::MobOutputBatch;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::NativeSound;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::NormalActionFrame;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::NormalMobActions;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::OrbOrder;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::PICKABLE_PROJECTILE_IDS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::PendingMobBatch;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::ProjectileHandBack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::ProjectileHitter;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::RemainingActions;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::Skinned;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::SpawnReason;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::SpawnSite;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::arrow_miss;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::bounced_place;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::breeding_credit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::bucketable;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::death_source::death_hurt_source;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::dye_color_mix_in_recipes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::dye_mix;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::flame_geometry;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::from_loot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::is_item_billboard;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::killed_by_mob;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::living_draw::LivingFrame;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::local_difficulty_in;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::mob_effect_instance;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::named_mob_pickable;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::pickable_projectile_hit;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::pickable_projectile_hitboxes;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::pickable_projectile_hitboxes_into;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_targets::{
    NonMobHit, ProjectileHitInfo, TargetQuery, is_pickable_projectile,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::seen;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::thrown_item_axes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::thrown_item_quad;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::to_loot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{EquipmentSlot, LootDrops, SoundSource};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_assets::item_model::DisplayContext;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::health::{
    damage_type, melee, source_damage_type,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    arrows::EntityHit,
    blocks::Fluid,
    crafting::Recipes,
    fast_hash::FxHashMap,
    graphics::EntityCullingMode,
    health::{Attack, HealthEvent, HurtSource, Hurting, Knock},
    items::{ItemStack, Items},
    join_seq::JoinSeq,
    mesher::Vertex,
    mobs::{
        Damage, DamageKind, Mob, MobEvent, MobWorld, PlayerView, Seen, SpawnSetup, Species, Who,
        ai::{eating_sound, eating_sound_is_bound, feed_animal, takes_food},
        entity_sounds::full_id,
        finalize::Group,
        spawning::{self, BiomeFacts},
    },
    particles::{ParticleOptions, ParticleType},
    projectiles::{Hit, Projectile, ProjectileKind},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client_render::render_geometry::EntitySpan;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Direction, Vec3, trig::Mth},
    position::BlockPos,
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::projectile_world::ProjWorld;

#[path = "living_draw_client.rs"]
pub(in crate::game) mod living_draw;

impl Game<'_> {
    /// What an arrow's tick asked for: its particles and sounds.
    pub(in crate::game) fn arrow_event(
        &mut self,
        at: Vec3,
        event: sunrise_client::arrows::ArrowEvent,
    ) {
        use sunrise_client::arrows::{ArrowEvent, ArrowParticle, ArrowSound, BlockReaction};
        use sunrise_client::particles::{ParticleOptions, ParticleType};
        let sound_name = |sound| match sound {
            ArrowSound::Splash => "minecraft:entity.generic.splash",
            ArrowSound::HitGround => "minecraft:entity.arrow.hit",
            ArrowSound::CrossbowHit => "minecraft:item.crossbow.hit",
            ArrowSound::Extinguish => "minecraft:entity.generic.extinguish_fire",
            ArrowSound::TridentHitGround => "minecraft:item.trident.hit_ground",
            ArrowSound::TridentHit => "minecraft:item.trident.hit",
        };
        if !self.is_logical_server() {
            let kind = |k| match k {
                ArrowParticle::Bubble => ParticleType::Bubble,
                ArrowParticle::Crit => ParticleType::Crit,
                ArrowParticle::Splash => ParticleType::Splash,
            };
            match event {
                ArrowEvent::Particle {
                    kind: k,
                    position,
                    velocity,
                } => self.particles.0.add(
                    &ParticleOptions::simple(kind(k)),
                    [position.x, position.y, position.z],
                    [velocity.x, velocity.y, velocity.z],
                ),
                ArrowEvent::Sound {
                    sound,
                    volume,
                    pitch,
                } => self.audio.0.play_on(
                    6,
                    sound_name(sound),
                    Some([at.x, at.y, at.z]),
                    volume,
                    pitch,
                ),
                _ => {}
            }
            return;
        }
        match event {
            ArrowEvent::Particle { .. } => {}
            // Arrow.tick: `broadcastEntityEvent(this, 0)`. The guests burst the particles.
            ArrowEvent::PotionPuff { id, .. } => {
                self.lan_entity_event(id as i32, 0, [at.x, at.y, at.z]);
            }
            ArrowEvent::Sound {
                sound,
                volume,
                pitch,
            } => {
                // NEUTRAL, which neither AbstractArrow nor ThrownTrident overrides.
                self.server_play_sound_on(6, sound_name(sound), [at.x, at.y, at.z], volume, pitch);
            }
            ArrowEvent::BubbleColumnParticles { pos } => self.bubble_column_particles(pos),
            ArrowEvent::Block {
                pos,
                reaction,
                owner,
            } => match reaction {
                BlockReaction::PrimeTnt => {
                    // `TntBlock.onProjectileHit`: the arrow's owner lights it.
                    if self.tnt_prime_allowed(pos) {
                        self.spawn_primed_tnt(pos, false, owner, None);
                        self.with_level(|b, level, ticks| b.remove_block(level, ticks, pos, false));
                        let _ = self.process_level_events();
                    }
                }
                BlockReaction::Light(lit) => {
                    let _ = self.level_set([pos.x, pos.y, pos.z], lit);
                }
            },
            ArrowEvent::SplashEvent => {
                self.projectile_event("minecraft:splash", None, [at.x, at.y, at.z]);
            }
            // finish_arrow_hit drops the pickup, which it holds.
            ArrowEvent::ShootEvent | ArrowEvent::Drop => {}
        }
    }
    pub(in crate::game) fn begin_mob_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
    ) -> MobDrawFrame<'_> {
        // The coarse mob bounds pay off only in dense async scenes; otherwise,
        // keep the existing exact mesh-culling path.
        let precheck = self.settings.sunrise.entity_culling == EntityCullingMode::Asynchronous
            && self
                .occlusion
                .0
                .async_culler
                .as_ref()
                .is_some_and(|culler| culler.camera_is_stationary())
            && self.settings.sunrise.entity_culling_precheck != 0
            && self.mobs.len() >= self.settings.sunrise.entity_culling_precheck as usize
            && !self.occlusion.0.shadow_casters.0
            && mod_api::__private::mods().is_empty();
        self.client.rendered_mobs.set(0);
        // What custom entity model animations read of the frame.
        sunrise_client::mobs::cem_anim::set_frame(sunrise_client::mobs::cem_anim::Frame {
            time: self.world.game_time as f64 + alpha,
            day_time: self.world.day_time as f64 + alpha,
            dimension: match self.dim {
                Dim::Overworld | Dim::Extra(_) => 0.0,
                Dim::Nether => -1.0,
                Dim::End => 1.0,
            },
            player: self.player.as_ref().map_or(camera.to_array(), |p| {
                [p.position.x, p.position.y, p.position.z]
            }),
            player_rot: [self.pitch, self.yaw],
            paused: self.pause,
            first_person: self.client.camera_type.is_first_person(),
            third_person_reversed: self.client.camera_type
                == sunrise_client::camera::CameraType::ThirdPersonFront,
            ..Default::default()
        });
        MobDrawFrame {
            precheck,
            living: LivingFrame {
                camera,
                alpha,
                scale: self.view_scale(),
                viewer: self.player_team(),
                skinning: self.skinning(),
                listened: self.mob_render_listened(),
                now: std::time::Instant::now(),
            },
        }
    }
    /// Mob models and projectiles for the entity pass.
    /// The renderer's models for the GPU path (`mobs::skin`) this frame, if it
    /// poses entities: a new frame of draws, the store restarted when the
    /// renderer's no longer matches. Once per frame, before any entity draws.
    pub(in crate::game::client_side) fn begin_skinning(&self) {
        let renderer = self.renderer.0.as_ref().filter(|r| r.skins_entities());
        match renderer.and_then(|r| r.skinned_models()) {
            Some(stored) => {
                let mut models = self.skin_models.0.borrow_mut();
                // Reuse helps CPU-bound crowds, but regressed TAA frame tails in
                // windowed measurements. Keep the original arithmetic with TAA on.
                models.set_rotation_cache_enabled(!self.settings.sunrise.taa);
                models.begin(&mut self.client.skin_frame.borrow_mut(), stored);
                models.tagged = renderer.is_some_and(|r| r.skins_tagged());
            }
            None => self.client.skin_frame.borrow_mut().clear_draws(),
        }
    }
    /// `Entity.sendBubbleColumnParticles` at the top of the column block `pos`: two
    /// splashes and two bubbles over it, which the level random places, shown here
    /// and to the players within range.
    pub(in crate::game) fn bubble_column_particles(&mut self, pos: BlockPos) {
        for _ in 0..2 {
            let r = &mut self.world.random;
            let splash = [
                f64::from(pos.x) + r.next_double(),
                f64::from(pos.y + 1),
                f64::from(pos.z) + r.next_double(),
            ];
            let bubble = [
                f64::from(pos.x) + r.next_double(),
                f64::from(pos.y + 1),
                f64::from(pos.z) + r.next_double(),
            ];
            self.server_level_particles(
                &crate::lan::server_effects::ServerParticle::simple(ParticleType::Splash),
                splash,
                [0.0; 3],
                1.0,
                1,
            );
            self.server_level_particles(
                &crate::lan::server_effects::ServerParticle::simple(ParticleType::Bubble),
                bubble,
                [0.0, 0.01, 0.0],
                0.2,
                1,
            );
        }
    }
    /// Culls one entity's vertices from `entity_start`, counting what it drew
    /// on the GPU path by the corners of `skinned`'s bounds; a culled entity's
    /// posed draws since `skinned`'s mark are taken back.
    pub(in crate::game::client_side) fn cull_skinned(
        &self,
        out: &mut Vec<Vertex>,
        entity_start: usize,
        skinned: Skinned,
    ) {
        let corners = skinned.and_then(|(_, bounds)| bounds);
        if out.len() == entity_start {
            if let Some((lo, hi)) = corners
                && self.cull_entity_camera_bounds(lo, hi)
                && let Some((mark, _)) = skinned
            {
                if self.occlusion.0.shadow_casters.0 {
                    let mut frame = self.client.skin_frame.borrow_mut();
                    let end = frame.vertices as usize;
                    sunrise_client::gfx::pack_draw::EntitySpan::hide(
                        &mut frame.spans,
                        mark.2 as usize,
                        end,
                    );
                } else {
                    self.client.skin_frame.borrow_mut().truncate(mark);
                }
            }
            return;
        }
        if let Some((lo, hi)) = corners {
            out.extend([lo, hi].map(|pos| Vertex {
                pos: pos.to_array(),
                ..bytemuck::Zeroable::zeroed()
            }));
        }
        let len = out.len();
        let hidden = self.cull_entity_vertices(out, entity_start);
        if out.len() < len {
            if let Some((mark, _)) = skinned {
                self.client.skin_frame.borrow_mut().truncate(mark);
            }
        } else if corners.is_some() {
            if let Some((mark, _)) = skinned {
                // Previous GPU-only entities can leave zero-length CPU spans at
                // this same offset. Use this query's result, not their metadata.
                if hidden {
                    let mut skin = self.client.skin_frame.borrow_mut();
                    let end = skin.vertices as usize;
                    EntitySpan::hide(&mut skin.spans, mark.2 as usize, end);
                }
            }
            out.truncate(len - 2);
            // The temporary bounds are not real geometry. Keep the culling end marker
            // at the true CPU end before the next entity appends a span there.
            for span in self.client.entity_spans.borrow_mut().iter_mut().rev() {
                if span.start as usize <= out.len() {
                    break;
                }
                span.start = out.len() as u32;
            }
        }
    }
    /// `LivingEntity.makePoofParticles` (entity event 60) about a mob's box.
    pub(in crate::game) fn death_poof(&mut self, at: [f64; 3], width: f64, height: f64) {
        let options = sunrise_client::particles::ParticleOptions::simple(
            sunrise_client::particles::ParticleType::Poof,
        );
        for _ in 0..20 {
            let r = self.particles.0.level_random();
            let xa = r.next_gaussian() * 0.02;
            let ya = r.next_gaussian() * 0.02;
            let za = r.next_gaussian() * 0.02;
            let x = at[0] + width * (2.0 * r.next_double() - 1.0) - xa * 10.0;
            let y = at[1] + height * r.next_double() - ya * 10.0;
            let z = at[2] + width * (2.0 * r.next_double() - 1.0) - za * 10.0;
            self.particles.0.add(&options, [x, y, z], [xa, ya, za]);
        }
    }
    /// `Guardian.aiStep` on a client: two bubbles a tick trailing a guardian
    /// that swims along, and bubbles up the beam to its target. The server
    /// never draws them, so the mob's own random stays untouched.
    pub(in crate::game) fn guardian_bubbles(&mut self) {
        use sunrise_client::mobs::guardian;
        use sunrise_client::particles::{ParticleOptions, ParticleType};
        let mut swimming = Vec::new();
        let mut beams = Vec::new();
        for m in &self.mobs {
            if !m.alive() || !matches!(m.species(), Species::Guardian | Species::ElderGuardian) {
                continue;
            }
            let at = [m.position.x, m.position.y, m.position.z];
            if m.extra.flag2 && m.fluids.in_water {
                let (width, height, _) = m.dims();
                swimming.push((at, width, height, m.view_vector()));
            }
            if let Some((_, scale)) = guardian::beam(m, 0.0)
                && let Some(t) = self.beam_target(m.ai.target, 1.0)
            {
                beams.push(([at[0], m.eye_y(), at[2]], [t.x, t.y, t.z], f64::from(scale)));
            }
        }
        let bubble = ParticleOptions::simple(ParticleType::Bubble);
        for (at, width, height, view) in swimming {
            for _ in 0..2 {
                let (x, y, z) = {
                    let r = self.particles.0.random();
                    (r.next_double(), r.next_double(), r.next_double())
                };
                let p = [
                    at[0] + width * (2.0 * x - 1.0) * 0.5 - view[0] * 1.5,
                    at[1] + height * y - view[1] * 1.5,
                    at[2] + width * (2.0 * z - 1.0) * 0.5 - view[2] * 1.5,
                ];
                self.particles.0.add(&bubble, p, [0.0; 3]);
            }
        }
        for (eye, target, scale) in beams {
            let d = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
            let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if length <= 0.0 {
                continue;
            }
            let mut dist = self.particles.0.random().next_double();
            while dist < length {
                let r = self.particles.0.random();
                dist += 1.8 - scale + r.next_double() * (1.7 - scale);
                let f = dist / length;
                let p = [eye[0] + d[0] * f, eye[1] + d[1] * f, eye[2] + d[2] * f];
                self.particles.0.add(&bubble, p, [0.0; 3]);
            }
        }
    }
    /// `Armadillo.mobInteract`: a brush brushes a scute off an adult
    /// (`brushOffScute`) and loses 16 durability.
    pub(in crate::game) fn interact_armadillo(&mut self, index: usize, item: &str) -> bool {
        let m = &self.mobs[index];
        if item != "minecraft:brush"
            || m.species() != Species::Armadillo
            || !m.alive()
            || m.is_baby()
        {
            return false;
        }
        let (id, at) = (m.id, m.position);
        let feet = [at.x, at.y, at.z];
        let tool = self.held().cloned();
        let table = "minecraft:brush/armadillo";
        let mut stacks = self.sequence_loot(table, table, tool.as_ref());
        let kind = super::native_hooks::mob_kind(&self.mobs[index]);
        let player = Some(self.native_acting_id());
        self.native_use_drops(
            table,
            feet,
            None,
            Some((id, &kind)),
            player,
            tool.as_ref(),
            &mut stacks,
        );
        for d in stacks {
            self.spawn_at_location(at, 0.0, d);
        }
        self.server_play_sound("minecraft:entity.armadillo.brush", feet, 1.0, 1.0);
        let ctx = self.who_context(Some(Who::Mob(id)), None);
        self.post_game_event("minecraft:entity_interact", feet, ctx);
        // ItemStack.hurtAndBreak(16).
        if !self.creative
            && let Some(stack) = tool
        {
            let max = self
                .res
                .items()
                .get(&stack.item)
                .map_or(0, |i| i.max_damage);
            if max > 0 {
                self.damage_held(16);
            }
        }
        true
    }
    /// `Snowball`/`ThrownEgg.handleEntityEvent(3)`: eight crumbs of the
    /// thrown item where it broke, at rest for a snowball, scattered by the
    /// client entity's random (the particle engine's here) for an egg.
    pub(in crate::game) fn item_burst(&mut self, at: [f64; 3], item: &str) {
        let options = self.item_particle(&sunrise_client::items::ItemStack::new(item, 1));
        let egg = item == "minecraft:egg";
        for _ in 0..8 {
            let v = if egg {
                let r = self.particles.0.random();
                [
                    (f64::from(r.next_float()) - 0.5) * 0.08,
                    (f64::from(r.next_float()) - 0.5) * 0.08,
                    (f64::from(r.next_float()) - 0.5) * 0.08,
                ]
            } else {
                [0.0; 3]
            };
            self.particles.0.add(&options, at, v);
        }
    }
    /// `ThrownEnderpearl.onHit` for a living non-player owner: the same hook,
    /// then the normal entity teleport, without a player's endermite roll or damage.
    pub(in crate::game) fn mob_pearl_teleport(&mut self, pearl: &Projectile, id: u32, hit: Hit) {
        let Some(owner) = self.entity(id) else { return };
        let Some(from) = self
            .mobs
            .iter()
            .find(|m| {
                m.id == id
                    && m.alive()
                    && !m.removed
                    && !sunrise_client::mobs::villager::sleeping(m)
            })
            .map(|m| m.position)
        else {
            return;
        };
        let dimension = self.dim;
        let failures = mod_api::runtime::isolated_failures();
        let _: std::result::Result<(), ()> = self.storage_transaction(|game, _| {
            let (mut target, _, cancel) =
                game.native_pearl_teleport(owner, pearl.id, from, pearl.previous, 0.0, hit);
            if mod_api::runtime::isolated_failures() != failures {
                return Err(());
            }
            // Listener writes stay in the transaction; never overwrite a changed,
            // dead, sleeping or removed owner using the old decision.
            if cancel
                || game.dim != dimension
                || game.entity(id) != Some(owner)
                || !game.mobs.iter().any(|m| {
                    m.id == id
                        && m.alive()
                        && !m.removed
                        && m.position == from
                        && !sunrise_client::mobs::villager::sleeping(m)
                })
            {
                return Ok(());
            }
            if !target.iter().all(|v| v.is_finite()) {
                target = [pearl.previous.x, pearl.previous.y, pearl.previous.z];
            }
            if !super::native_entity_moves::in_bounds(target) {
                return Ok(());
            }
            let _ = game.place_teleport_filed(
                super::commands::Target::Mob(id),
                target,
                None,
                [false; 3],
                true,
            );
            if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
                m.fall_distance = 0.0;
                // Entity.teleport's setOldRot: keep the owner's angles without
                // interpolating from the pre-teleport rotation on the client.
                m.previous_yaw = m.yaw;
                m.previous_head_pitch = m.head_pitch;
            }
            game.audio
                .0
                .play("minecraft:entity.player.teleport", Some(target), 1.0, 1.0);
            Ok(())
        });
    }
    /// `MushroomCow.shear`'s `sendParticles(EXPLOSION, x, getY(0.5), z, 1, 0, 0, 0, 0)`:
    /// one puff over a sheared mooshroom, half its `height` up, shown here and to
    /// the players within range.
    pub(in crate::game) fn mooshroom_shear_puff(&mut self, at: [f64; 3], height: f64) {
        let at = [at[0], at[1] + 0.5 * height, at[2]];
        self.server_level_particles(
            &crate::lan::server_effects::ServerParticle::simple(ParticleType::Explosion),
            at,
            [0.0; 3],
            0.0,
            1,
        );
    }
    pub(in crate::game::client_side) fn pickup_arrow_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
    ) {
        // ItemPickupParticle: a picked-up arrow flying to its collector.
        let taken = self.pickups.0.live.iter().filter_map(|p| match &p.taken {
            super::pickups::Taken::Arrow(a) => Some((&**a, super::pickups::position(p, alpha))),
            _ => None,
        });
        self.projectile_geometry(camera, alpha, taken, out);
    }
    pub(in crate::game) fn projectile_geometry<'a>(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        projectiles: impl Iterator<Item = (&'a Projectile, [f64; 3])>,
        out: &mut Vec<Vertex>,
    ) {
        let atlas = &self.gui.atlas;
        let (right, up) = thrown_item_axes(self.client.camera_type, self.yaw, self.pitch);
        // A glowing projectile's outline: what it drew up to the next one.
        let mut glowing = None;
        for (a, at) in projectiles {
            if let Some((start, rgb)) = glowing.take() {
                self.mark_outline(&out[start..], rgb);
            }
            self.mark_entity(out.len(), a.entity_type());
            if a.id != 0
                && let Some(rgb) = self.thing_outline(a.id, a.kind.id())
            {
                glowing = Some((out.len(), rgb));
            }
            let p = glam::DVec3::from_array(at) - camera;
            let c = p.as_vec3();
            // EntityRenderer.getPackedLightCoords: the block its light probe
            // (the eye) is in, at the position this frame draws rather than
            // this tick's, and not the one under its feet.
            let light = self.world.light(
                at[0].floor() as i32,
                a.light_probe_y(alpha).floor() as i32,
                at[2].floor() as i32,
            );
            let mut packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);
            // EntityRenderer.getBlockLightLevel: a shulker bullet and anything
            // on fire are drawn at 15 block light (the fireball renderers'
            // own override); the sky light stays the level's.
            if let Some(block) = a.block_light_override() {
                packed = (packed & 0xFF) | (u32::from(block) * 16) << 8;
            }
            if a.is_arrow() {
                let Some(model) = self.client.arrow_model.as_ref() else {
                    continue;
                };
                // SpectralArrowRenderer, and TippableArrowRenderer's `getColor() > 0`.
                let texture = if a.is_spectral_arrow() {
                    "entity/projectiles/arrow_spectral"
                } else if a.arrow.as_ref().is_some_and(|s| s.effect_color > 0) {
                    "entity/projectiles/arrow_tipped"
                } else {
                    "entity/projectiles/arrow"
                };
                let Some(r) = atlas.sprites.get(texture).copied() else {
                    continue;
                };
                // ArrowRenderer: `Mth.lerp(partialTick, state.yRotO, state.yRot)`
                // and the same for the pitch, so a turning arrow is drawn
                // between the two ticks it turned over.
                let alpha32 = alpha as f32;
                let yaw = a.previous_yaw + alpha32 * (a.yaw - a.previous_yaw);
                let pitch = a.previous_pitch + alpha32 * (a.pitch - a.previous_pitch);
                let m = glam::Mat4::from_translation(c)
                    * glam::Mat4::from_rotation_y((yaw - 90.0).to_radians())
                    * glam::Mat4::from_rotation_z(pitch.to_radians());
                // ArrowRenderer: `state.shake = shakeTime - partialTicks`; ArrowModel.setupAnim
                // rolls the root while the shake timer runs.
                let shake = a
                    .arrow
                    .as_ref()
                    .map_or(0.0, |s| s.shake as f32 - alpha as f32);
                let roll = if shake > 0.0 {
                    -(shake * 3.0).msin() * shake * (std::f32::consts::PI / 180.0)
                } else {
                    0.0
                };
                let pose_for = |part: &sunrise_client::mobs::Part| {
                    let mut pose = part.pose;
                    if std::ptr::eq(part, model) {
                        pose[5] += roll;
                    }
                    pose
                };
                sunrise_client::mobs::render_model(
                    model,
                    m,
                    &pose_for,
                    [r.u0, r.v0, r.u1, r.v1],
                    packed,
                    0,
                    out,
                );
                if a.arrow.as_ref().is_some_and(|s| s.on_fire()) {
                    flame_geometry(atlas, c, 0.5, 0.5, self.yaw, light, out);
                }
                continue;
            }
            // ThrownTridentRenderer.
            if matches!(a.kind, ProjectileKind::Trident) {
                let Some(model) = self.client.trident_model.as_ref() else {
                    continue;
                };
                let Some(r) = atlas.sprites.get("entity/trident/trident").copied() else {
                    continue;
                };
                let m = glam::Mat4::from_translation(c)
                    * glam::Mat4::from_rotation_y((a.yaw - 90.0).to_radians())
                    * glam::Mat4::from_rotation_z((a.pitch + 90.0).to_radians());
                let poses = sunrise_client::mobs::anim::Poses::new(model);
                sunrise_client::mobs::render::draw(
                    model,
                    m,
                    &poses,
                    [r.u0, r.v0, r.u1, r.v1],
                    0xFFFF_FFFF,
                    packed,
                    0,
                    out,
                );
                continue;
            }
            // WindChargeRenderer.
            if matches!(
                a.kind,
                ProjectileKind::WindCharge | ProjectileKind::BreezeWindCharge
            ) {
                self.wind_charge_geometry(a, c, packed, alpha as f32, out);
                continue;
            }
            // ShulkerBulletRenderer: the tumbling bullet model, then a larger
            // translucent copy of it (colour 0x26FFFFFF); always 15 block light.
            if matches!(a.kind, ProjectileKind::ShulkerBullet { .. }) {
                let model = &self.client.entity_models.shulker_bullet;
                if let (Some(model), Some(r)) =
                    (model, atlas.sprites.get("entity/shulker/spark").copied())
                {
                    use sunrise_core::geometry::trig;
                    let tc = a.age as f32 + alpha as f32;
                    let mut poses = sunrise_client::mobs::anim::Poses::new(model);
                    poses.yrot("main", a.yaw.to_radians());
                    poses.xrot("main", a.pitch.to_radians());
                    let light = (packed & 0xFF) | ((15 * 16) << 8);
                    let base = glam::Mat4::from_translation(c)
                        * glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.15, 0.0))
                        * glam::Mat4::from_rotation_y(
                            (trig::sin(f64::from(tc * 0.1)) * 180.0).to_radians(),
                        )
                        * glam::Mat4::from_rotation_x(
                            (trig::cos(f64::from(tc * 0.1)) * 180.0).to_radians(),
                        )
                        * glam::Mat4::from_rotation_z(
                            (trig::sin(f64::from(tc * 0.15)) * 360.0).to_radians(),
                        )
                        * glam::Mat4::from_scale(glam::Vec3::new(-0.5, -0.5, 0.5));
                    let uv = [r.u0, r.v0, r.u1, r.v1];
                    sunrise_client::mobs::render::draw(
                        model,
                        base,
                        &poses,
                        uv,
                        0xFFFF_FFFF,
                        light,
                        0,
                        out,
                    );
                    // The translucent shell blends over the opaque core
                    // (`RenderTypes.entityTranslucent`).
                    let shell = out.len();
                    sunrise_client::mobs::render::draw(
                        model,
                        base * glam::Mat4::from_scale(glam::Vec3::splat(1.5)),
                        &poses,
                        uv,
                        0x26FF_FFFF,
                        light,
                        0,
                        out,
                    );
                    sunrise_client::gfx::mark_oit(&mut out[shell..]);
                }
                continue;
            }
            // WitherSkullRenderer: the skull model facing its flight.
            if let Some(dangerous) = super::wither::skull_kind(a) {
                let model = &self.client.entity_models.wither_skull;
                let tex = if dangerous {
                    "entity/wither/wither_invulnerable"
                } else {
                    "entity/wither/wither"
                };
                if let (Some(model), Some(r)) = (model, atlas.sprites.get(tex).copied()) {
                    let mut poses = sunrise_client::mobs::anim::Poses::new(model);
                    poses.yrot("head", a.yaw.to_radians());
                    poses.xrot("head", a.pitch.to_radians());
                    let base = glam::Mat4::from_translation(c)
                        * glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0));
                    sunrise_client::mobs::render::draw(
                        model,
                        base,
                        &poses,
                        [r.u0, r.v0, r.u1, r.v1],
                        0xFFFF_FFFF,
                        packed,
                        0,
                        out,
                    );
                }
                continue;
            }
            if self.thrown_item_geometry(a, alpha, at, c, packed, out) {
                continue;
            }
            // `Entity.displayFireAnimation`: a thrown item that flew through a
            // fire block burns (`ThrowableProjectile.tick`'s
            // `applyEffectsFromBlocks`), and ThrownItemRenderer draws the flame
            // over its box as any other entity renderer does.
            if a.on_fire() {
                let size = a.kind.size() as f32;
                flame_geometry(atlas, c, size, size, self.yaw, light, out);
            }
            // `ThrownItemRenderer` for a fireball: the item `Fireball.getItem`
            // returns (a fire charge until `setItem` gave it another), its model
            // at the renderer's scale for the kind. `packed` is already 15, the
            // renderer's full bright.
            if let Some(stack) = a.item.as_ref() {
                let owner = super::item_draw::owner_at(at, 0.0, a.kind.id());
                let state = self.resolve_item(stack, DisplayContext::Ground, Some(&owner), 0);
                let pose = glam::Mat4::from_translation(c)
                    * glam::Mat4::from_quat(self.camera_orientation())
                    * glam::Mat4::from_scale(glam::Vec3::splat(a.sprite_scale()));
                self.submit_item_world(&state, pose, packed, out);
                continue;
            }
            let Some(name) = a.item_sprite() else {
                continue;
            };
            let Some(r) = atlas.sprites.get(name).copied() else {
                continue;
            };
            let lift = is_item_billboard(&a.kind);
            let quad = thrown_item_quad(c, right, up, a.sprite_scale(), lift);
            for (v, uv) in quad
                .iter()
                .zip([[r.u0, r.v1], [r.u1, r.v1], [r.u1, r.v0], [r.u0, r.v0]])
            {
                out.push(Vertex {
                    normal: [0; 4],
                    pos: v.to_array(),
                    uv,
                    layer: 0,
                    color: 0xFFFF_FFFF,
                    light: packed,

                    glint_uv: [0.0, 0.0],
                });
            }
        }
        if let Some((start, rgb)) = glowing {
            self.mark_outline(&out[start..], rgb);
        }
    }
    /// Whether this frame's models can be posed on the GPU (`begin_skinning`).
    pub(in crate::game::client_side) fn skinning(&self) -> bool {
        self.renderer
            .0
            .as_ref()
            .is_some_and(|r| r.skins_entities() && r.skinned_models().is_some())
    }
}

#[cfg(test)]
#[path = "../../creatures/spawn_egg_mob_tests_client.rs"]
pub(in crate::game) mod spawn_egg_mob_tests;

#[cfg(test)]
#[path = "local_sound_event_tests_client.rs"]
pub(in crate::game) mod local_sound_event_tests;

#[cfg(test)]
#[path = "projectile_batch_tests_client.rs"]
pub(in crate::game) mod projectile_batch_tests;

#[cfg(test)]
#[path = "loaded_arrow_shot_tests_client.rs"]
pub(in crate::game) mod loaded_arrow_shot_tests;

#[cfg(test)]
#[path = "projectiles_120_124_tests_client.rs"]
pub(in crate::game) mod projectiles_120_124_tests;
