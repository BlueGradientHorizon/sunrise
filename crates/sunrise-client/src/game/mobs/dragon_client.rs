#[allow(unused_imports)]
pub(in crate::game::client_side) use super::api_values::TryIntoApi;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::commands::Target;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game,
    creatures::{Finalized, SpawnSite},
    potions::{Victim, mob_can_be_affected},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::ADJACENCY;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::AMBIENT_INTERVAL;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::BODY;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::BREATH_WAIT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::Breath;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::Crystal;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::CrystalDamage;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::DirectHit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::Dragon;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::DragonHitOrigin;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::FIGHT_FILE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::Fight;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::FightPlayer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::FightWho;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::Hurt;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::MAX_HEALTH;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::PART_SIZES;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::Path;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::PathBox;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::Phase;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::Push;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::Respawn;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::SPECIAL;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::breath_expired;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::breath_puffs;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::dragon_part_id;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::dragon_part_index;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::dragon_part_wire_id;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::wrap;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dragon::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::DragonPhase;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    health::{DamageContainer, DamageLevel, HurtSource, Hurting, Knock, Reduction},
    hud::HudBatch,
    hud_layout, hud_overlays,
    mesher::Vertex,
    mobs::{Damage, DamageKind, Who},
    particles::{ParticleOptions, ParticleType},
    projectiles::{Projectile, ProjectileKind},
    random_source,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client_core::nbt_value;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client_gameplay::dragon_render::{
    dissolve_color, dissolve_mask, dragon_rays,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::position::BlockPos;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3, trig},
    jmath,
    random::{LegacyRandom, RandomSource},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::entity_data::Value;

#[path = "dragon/walls_client.rs"]
pub(in crate::game) mod walls;

impl Game<'_> {
    /// The boss bars' draws on a `w` x `h` GUI.
    pub(in crate::game) fn boss_bar_draws(&self, w: i32, h: i32) -> Vec<hud_layout::GuiDraw> {
        let now = sunrise_client::boss_overlay::now_millis();
        let bars = self.client.boss_overlay.bars(now);
        // NeoForge's `CustomizeGuiOverlayEvent.BossEventProgress`, asked per
        // bar only while a mod listens.
        let customized = mod_api::__private::listened(mod_api::HookId::CustomizeBossEventProgress);
        hud_overlays::boss_overlay(w, h, &bars, &self.gui.atlas.font, |index, at| {
            let Some(bar) = customized
                .then(|| self.client.boss_overlay.mod_bar(index, now))
                .flatten()
            else {
                return true;
            };
            let Ok(bar) = bar.try_into_api() else {
                return true;
            };
            let mut cancel = false;
            self.native_customize_boss_event_progress(
                &bar,
                &mut at.x,
                &mut at.y,
                &mut at.increment,
                &mut cancel,
            );
            !cancel
        })
    }
    /// `BossHealthOverlay.shouldPlayMusic`.
    pub(in crate::game::client_side) fn boss_music(&self) -> bool {
        self.client.boss_overlay.should_play_music()
    }
    /// `EnderDragonRenderer.submit` and `EnderDragonModel.setupAnim`, from the real dragon's slot.
    pub(in crate::game::client_side) fn dragon_geometry(
        &self,
        d: &Dragon,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
        additive: &mut Vec<Vertex>,
    ) {
        let Some(model) = self.client.dragon_model.as_ref() else {
            return;
        };
        let a = alpha as f32;
        let sprite = |n: &str| {
            self.gui
                .atlas
                .sprites
                .get(n)
                .map(|r| [r.u0, r.v0, r.u1, r.v1])
        };
        let (Some(skin), Some(eyes)) = (
            sprite("entity/enderdragon/dragon"),
            sprite("entity/enderdragon/dragon_eyes"),
        ) else {
            return;
        };
        let p = d.previous.lerp(d.position, alpha);
        let pos = glam::DVec3::new(p.x, p.y, p.z);
        let h = |delay: usize| d.sample_lerp(delay, a);
        let yr = h(7).1;
        let rot2 = (h(5).0 - h(10).0) as f32;
        let mut base = glam::Mat4::from_translation((pos - camera).as_vec3())
            * glam::Mat4::from_rotation_y((-yr).to_radians())
            * glam::Mat4::from_rotation_x((rot2 * 10.0).to_radians())
            * glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0, 1.0))
            * glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0))
            * glam::Mat4::from_translation(glam::Vec3::new(0.0, -1.501, 0.0));
        let rays_base = base * glam::Mat4::from_translation(glam::Vec3::new(0.0, -1.0, -2.0));
        // setupAnim.
        let flap = (d.old_flap + (d.flap - d.old_flap) * a) * std::f32::consts::TAU;
        let mut bounce = (flap - 1.0).sin() + 1.0;
        bounce = (bounce * bounce + bounce * 2.0) * 0.05;
        base *=
            glam::Mat4::from_translation(glam::Vec3::new(0.0, (bounce - 2.0) * 16.0, -48.0) / 16.0)
                * glam::Mat4::from_rotation_x((bounce * 2.0).to_radians());
        let mut poses = sunrise_client::mobs::anim::Poses::new(model);
        poses.get("jaw")[3] = (flap.sin() + 1.0) * 0.2;
        let sitting = d.phase.sitting();
        let landing = matches!(d.phase, Phase::Landing { .. } | Phase::Takeoff { .. });
        let egg_dist = {
            let p = self.podium_top();
            ((p.x - d.position.x).powi(2)
                + (p.y - d.position.y).powi(2)
                + (p.z - d.position.z).powi(2))
            .sqrt()
        };
        let head_offset = |part: usize, start: (f64, f32), point: (f64, f32)| -> f32 {
            if landing {
                part as f32 / (egg_dist / 4.0).max(1.0) as f32
            } else if sitting {
                part as f32
            } else if part == 6 {
                0.0
            } else {
                (point.0 - start.0) as f32
            }
        };
        let start = h(6);
        let rot2y = wrap(h(5).1 - h(10).1);
        let rot = wrap(h(5).1 + rot2y / 2.0);
        let deg = std::f32::consts::PI / 180.0;
        let (mut xx, mut yy, mut zz) = (0.0f32, 20.0f32, -12.0f32);
        for i in 0..5 {
            let point = h(5 - i);
            let pose = poses.get(&format!("neck{i}"));
            let neck_x = (i as f32 * 0.45 + flap).cos() * 0.15;
            pose[4] = wrap(point.1 - start.1) * deg * 1.5;
            pose[3] = neck_x + head_offset(i, start, point) * deg * 1.5 * 5.0;
            pose[5] = -wrap(point.1 - rot) * deg * 1.5;
            pose[0] = xx;
            pose[1] = yy;
            pose[2] = zz;
            let (xr, yrr) = (pose[3], pose[4]);
            xx -= yrr.sin() * xr.cos() * 10.0;
            yy += xr.sin() * 10.0;
            zz -= yrr.cos() * xr.cos() * 10.0;
        }
        let current = h(0);
        let head = poses.get("head");
        head[0] = xx;
        head[1] = yy;
        head[2] = zz;
        head[4] = wrap(current.1 - start.1) * deg;
        head[3] = wrap(head_offset(6, start, current)) * deg * 1.5 * 5.0;
        head[5] = -wrap(current.1 - rot) * deg;
        poses.get("body")[5] = -rot2y * 1.5 * deg;
        let lw = [0.125 - flap.cos() * 0.2, -0.25, -(flap.sin() + 0.125) * 0.8];
        let lt = ((flap + 2.0).sin() + 0.5) * 0.75;
        {
            let p = poses.get("left_wing");
            (p[3], p[4], p[5]) = (lw[0], lw[1], lw[2]);
        }
        poses.get("left_wing_tip")[5] = lt;
        {
            let p = poses.get("right_wing");
            (p[3], p[4], p[5]) = (lw[0], -lw[1], -lw[2]);
        }
        poses.get("right_wing_tip")[5] = -lt;
        for side in ["left", "right"] {
            poses.get(&format!("{side}_hind_leg"))[3] = 1.0 + bounce * 0.1;
            poses.get(&format!("{side}_hind_leg_tip"))[3] = 0.5 + bounce * 0.1;
            poses.get(&format!("{side}_hind_foot"))[3] = 0.75 + bounce * 0.1;
            poses.get(&format!("{side}_front_leg"))[3] = 1.3 + bounce * 0.1;
            poses.get(&format!("{side}_front_leg_tip"))[3] = -0.5 - bounce * 0.1;
            poses.get(&format!("{side}_front_foot"))[3] = 0.75 + bounce * 0.1;
        }
        let mut tail_x = 0.0f32;
        let (mut xx, mut yy, mut zz) = (0.0f32, 10.0f32, 60.0f32);
        let start = h(11);
        for i in 0..12 {
            let point = h(12 + i);
            tail_x += (i as f32 * 0.45 + flap).sin() * 0.05;
            let p = poses.get(&format!("tail{i}"));
            p[4] = (wrap(point.1 - start.1) * 1.5 + 180.0) * deg;
            p[3] = tail_x + (point.0 - start.0) as f32 * deg * 1.5 * 5.0;
            p[5] = wrap(point.1 - rot) * deg * 1.5;
            p[0] = xx;
            p[1] = yy;
            p[2] = zz;
            let (xr, yrr) = (p[3], p[4]);
            yy += xr.sin() * 10.0;
            zz -= yrr.cos() * xr.cos() * 10.0;
            xx -= yrr.sin() * xr.cos() * 10.0;
        }
        let l = self.world.light(
            d.position.x.floor() as i32,
            d.position.y.floor() as i32,
            d.position.z.floor() as i32,
        );
        let light = (u32::from(l >> 4) * 16) | ((u32::from(l & 15) * 16) << 8);
        let death = if d.death_time > 0 {
            d.death_time as f32 + a
        } else {
            0.0
        };
        // Dying: DYING_RENDER_TYPE, `entityCutoutDissolve` of the skin with dragon_exploding as
        // the mask, white at `ARGB.white(1 - deathTime / 200)` and no overlay.
        let mask = sprite("entity/enderdragon/dragon_exploding").filter(|_| death > 0.0);
        let (color, overlay) = if mask.is_some() {
            (dissolve_color(death), sunrise_client::gfx::DISSOLVE_OVERLAY)
        } else {
            (0xFFFF_FFFF, u32::from(d.hurt_time > 0))
        };
        let first = out.len();
        self.mark_entity(first, "minecraft:ender_dragon");
        self.mark_motion(first, 0);
        sunrise_client::mobs::render::draw(model, base, &poses, skin, color, light, overlay, out);
        if let Some(mask) = mask {
            dissolve_mask(&mut out[first..], skin, mask);
        }
        // The skin's OptiFine emissive overlay, full bright.
        let glow = self
            .res
            .mob_data()
            .emissive
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .overlay("entity/enderdragon/dragon")
            .and_then(sprite);
        if let Some(glow) = glow.filter(|_| mask.is_none()) {
            let full = sunrise_client::mobs::render::FULL_BRIGHT;
            sunrise_client::mobs::render::draw(model, base, &poses, glow, color, full, 0, out);
        }
        #[cfg(test)]
        super::native_render_pair_tests::witness_body(d.id, first, out.len());
        // RenderTypes.eyes: the glowing eyes, added on top.
        let first_eye = additive.len();
        sunrise_client::mobs::render::draw(
            model,
            base,
            &poses,
            eyes,
            0xFFFF_FFFF,
            240 | 240 << 8,
            0,
            additive,
        );
        // `RenderTypes.eyes` has an `OitPipelineSet`: improved transparency
        // moves the tagged quads into its stages (the HDR tier is the eyes'
        // shine, `mobs::render::EYES_TIER`).
        for v in &mut additive[first_eye..] {
            v.layer = 2
                | sunrise_client::gfx::OIT_ENTITY
                | u32::from(sunrise_client::mesher::BRIGHT) << 16;
        }
        if death > 0.0 {
            let rays = additive.len();
            self.dragon_rays(rays_base, death / 200.0, additive);
            for v in &mut additive[rays..] {
                v.layer |= u32::from(sunrise_client::mesher::VERY_BRIGHT) << 16;
            }
        }
        // The healing beam to the nearest crystal.
        if let Some(c) = d.nearest_crystal.and_then(|i| self.crystals.get(i)) {
            let bob = f64::from(sunrise_client::end_crystal::y(c.time as f32 + a));
            let to = glam::DVec3::new(c.position.x, c.position.y + bob, c.position.z);
            self.crystal_beam(pos, to - pos, self.visual_ticks() as f32 + a, camera, out);
        }
    }
    /// `Mob.playAmbientSound` when EntityType.spawn places a dragon.
    pub(in crate::game) fn dragon_spawn_ambient(&mut self, id: u32) -> bool {
        let Some(d) = self.dragon.as_mut().filter(|d| d.id == id) else {
            return false;
        };
        let at = [d.position.x, d.position.y, d.position.z];
        let pitch = (d.random.next_float() - d.random.next_float()) * 0.2 + 1.0;
        self.audio.0.play(
            "minecraft:entity.ender_dragon.ambient",
            Some(at),
            5.0,
            pitch,
        );
        true
    }
    /// `DragonLandingPhase.doClientTick`, `DragonSittingFlamingPhase.doClientTick`,
    /// `DragonDeathPhase.doClientTick` and `DragonSittingAttackingPhase.doClientTick`,
    /// which `EnderDragon.aiStep` runs on a client. The first two breathe
    /// dragon breath from the head (their `look.yRot(...)` results are thrown
    /// away in vanilla, so every puff leaves along the same line), the third
    /// puffs an explosion every 10 ticks and the last growls on every tick.
    pub(in crate::game::client_side) fn remote_dragon_phase_tick(&mut self) {
        let look = self.dragon_look_normalised();
        let Some(d) = self.dragon.as_mut() else {
            return;
        };
        let head = d.parts[0].0;
        let head_y = head.y + PART_SIZES[0].1 * 0.5;
        let motion = d.velocity;
        let at = [d.position.x, d.position.y, d.position.z];
        // `flameTicks`: the client copy's own counter, advanced before it is read.
        if let Phase::Flaming { ticks } = &mut d.phase {
            *ticks += 1;
        }
        let puffs = breath_puffs(
            &d.phase,
            look,
            [head.x, head_y, head.z],
            [motion.x, motion.y, motion.z],
            &mut d.random,
        );
        let mut emitter = None;
        let mut growl = None;
        match &mut d.phase {
            Phase::Dying { time, .. } => {
                if *time % 10 == 0 {
                    let r = &mut d.random;
                    let o = [
                        f64::from((r.next_float() - 0.5) * 8.0),
                        f64::from((r.next_float() - 0.5) * 4.0),
                        f64::from((r.next_float() - 0.5) * 8.0),
                    ];
                    emitter = Some([at[0] + o[0], at[1] + 2.0 + o[1], at[2] + o[2]]);
                }
                *time += 1;
            }
            Phase::Attacking { .. } => {
                growl = Some(0.8 + d.random.next_float() * 0.3);
            }
            _ => {}
        }
        let breath = ParticleOptions::simple(ParticleType::DragonBreath);
        for (p, v) in puffs {
            self.particles.0.add(&breath, p, v);
        }
        if let Some(p) = emitter {
            self.particles.0.add(
                &ParticleOptions::simple(ParticleType::ExplosionEmitter),
                p,
                [0.0; 3],
            );
        }
        if let Some(pitch) = growl {
            self.audio
                .0
                .play("minecraft:entity.ender_dragon.growl", Some(at), 2.5, pitch);
        }
    }
    /// `LevelEvent 3001`: the dragon's growl over the island.
    pub(in crate::game) fn respawn_growl(&mut self) {
        let pitch = 0.8 + self.world.random.next_float() * 0.3;
        self.audio.0.play(
            "minecraft:entity.ender_dragon.growl",
            Some([0.5, 128.5, 0.5]),
            64.0,
            pitch,
        );
    }
    /// `DragonStrafePlayerPhase`'s shot: `new DragonFireball(level, dragon,
    /// direction.normalize())`, whose `AbstractHurtingProjectile` constructor
    /// takes the dragon as its owner and `assignDirectionalMovement`s the
    /// direction normalized again times `accelerationPower` (0.1), then
    /// `snapTo` the start. It joins the level's other projectiles.
    pub(in crate::game) fn spawn_dragon_fireball(&mut self, at: Vec3, dir: Vec3) {
        // Vec3.normalize: a vector shorter than 1e-5 is zero.
        let normalize = |v: Vec3| {
            let l = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
            if l < 1.0e-5 {
                Vec3::new(0.0, 0.0, 0.0)
            } else {
                Vec3::new(v.x / l, v.y / l, v.z / l)
            }
        };
        let n = normalize(normalize(dir));
        self.audio.0.play(
            "minecraft:entity.ender_dragon.shoot",
            Some([at.x, at.y, at.z]),
            10.0,
            1.0,
        );
        let owner = self.dragon.as_ref().map(|d| Who::Mob(d.id));
        let mut p = sunrise_client::projectiles::Projectile::new(
            sunrise_client::projectiles::ProjectileKind::DragonFireball,
            at,
            owner,
        );
        p.acceleration = 0.1;
        p.velocity = Vec3::new(n.x * 0.1, n.y * 0.1, n.z * 0.1);
        self.add_projectile(p);
    }
    /// One client tick of the dragon. `lerp`: the step toward its last
    /// sent position and yaw (LivingEntity's interpolation), and the
    /// steps left including this one.
    pub(in crate::game::client_side) fn tick_remote_dragon(
        &mut self,
        lerp: Option<([f64; 3], Option<f32>, u32)>,
    ) {
        let Some(d) = self.dragon.as_mut() else {
            return;
        };
        d.previous = d.position;
        d.previous_yaw = d.yaw;
        d.old_flap = d.flap;
        d.ticks += 1;
        d.hurt_time = (d.hurt_time - 1).max(0);
        for part in &mut d.parts {
            part.1 = part.0;
        }
        if let Some((target, yaw, steps)) = lerp {
            let n = f64::from(steps.max(1));
            d.position = Vec3::new(
                d.position.x + (target[0] - d.position.x) / n,
                d.position.y + (target[1] - d.position.y) / n,
                d.position.z + (target[2] - d.position.z) / n,
            );
            if let Some(yaw) = yaw {
                d.yaw += wrap(yaw - d.yaw) / steps.max(1) as f32;
            }
        }
        if !d.alive() {
            // EnderDragon.tickDeath on a client: it rises, and bursts
            // from tick 180 to 200; the server removes it.
            d.death_time += 1;
            let (t, p) = (d.death_time, d.position);
            // `EnderDragon.aiStep` still growls when the phase is not sitting.
            let sitting = d.phase.sitting();
            if (180..=200).contains(&t) {
                let r = &mut d.random;
                let o = [
                    f64::from((r.next_float() - 0.5) * 8.0),
                    f64::from((r.next_float() - 0.5) * 4.0),
                    f64::from((r.next_float() - 0.5) * 8.0),
                ];
                self.particles.0.add(
                    &ParticleOptions::simple(ParticleType::ExplosionEmitter),
                    [p.x + o[0], p.y + 2.0 + o[1], p.z + o[2]],
                    [0.0; 3],
                );
            }
            if let Some(d) = self.dragon.as_mut() {
                // deathMove is new Vec3(0.0, 0.1F, 0.0): the float widened.
                let rise = f64::from(0.1_f32);
                d.position.y += rise;
                for part in &mut d.parts {
                    part.0.y += rise;
                }
            }
            // `EnderDragon.aiStep` after `tickDeath`: the client growl is
            // before the `isDeadOrDying` branch, then a dead dragon adds
            // `ParticleTypes.EXPLOSION` every tick. Both use the risen spot.
            let mut growl = None;
            if let Some(d) = self.dragon.as_mut()
                && !sitting
            {
                d.growl -= 1;
                if d.growl < 0 {
                    // `this.random`: pitch, then `nextInt(200)`, before the puff.
                    let pitch = 0.8 + d.random.next_float() * 0.3;
                    d.growl = 200 + d.random.next_int_bound(200).unwrap_or(0);
                    growl = Some(([d.position.x, d.position.y, d.position.z], pitch));
                }
            }
            if let Some((at, pitch)) = growl {
                self.audio
                    .0
                    .play("minecraft:entity.ender_dragon.growl", Some(at), 2.5, pitch);
            }
            let Some(d) = self.dragon.as_mut() else {
                return;
            };
            let p = d.position;
            let r = &mut d.random;
            let o = [
                f64::from((r.next_float() - 0.5) * 8.0),
                f64::from((r.next_float() - 0.5) * 4.0),
                f64::from((r.next_float() - 0.5) * 8.0),
            ];
            self.particles.0.add(
                &ParticleOptions::simple(ParticleType::Explosion),
                [p.x + o[0], p.y + 2.0 + o[1], p.z + o[2]],
                [0.0; 3],
            );
            return;
        }
        // processFlappingMovement and onFlap: its wing beats, and their sound.
        // Mth.cos(flapTime * 6.2831855F); flapSpeed's `(float) Math.pow(2.0, movement.y)`.
        let flap_was = trig::cos(f64::from(d.old_flap * std::f32::consts::TAU));
        let v = d.velocity;
        let mut flap_speed = 0.2 / (((v.x * v.x + v.z * v.z).sqrt() as f32) * 10.0 + 1.0);
        flap_speed *= 2f64.powf(v.y) as f32;
        d.flap += if d.phase.sitting() { 0.1 } else { flap_speed };
        let flap_now = trig::cos(f64::from(d.flap * std::f32::consts::TAU));
        let at = [d.position.x, d.position.y, d.position.z];
        if flap_was <= -0.3 && flap_now >= -0.3 {
            let pitch = 0.8 + self.world.random.next_float() * 0.3;
            self.audio
                .0
                .play("minecraft:entity.ender_dragon.flap", Some(at), 5.0, pitch);
        }
        let d = self.dragon.as_mut().expect("dragon");
        if !d.phase.sitting() {
            d.growl -= 1;
            if d.growl < 0 {
                d.growl = 200 + d.random.next_int_bound(200).unwrap_or(0);
                let pitch = 0.8 + self.world.random.next_float() * 0.3;
                self.audio
                    .0
                    .play("minecraft:entity.ender_dragon.growl", Some(at), 2.5, pitch);
            }
        }
        // checkCrystals' search runs on a client too (the healing beam).
        let d = self.dragon.as_mut().expect("dragon");
        if d.random.next_int_bound(10).unwrap_or(1) == 0 {
            let p = d.position;
            let nearest = self.nearest_crystal_to(p);
            self.dragon.as_mut().expect("dragon").nearest_crystal = nearest;
        }
        let d = self.dragon.as_mut().expect("dragon");
        d.yaw = wrap(d.yaw);
        // EnderDragon.aiStep on a client with isNoAi, as on the server.
        if d.no_ai {
            d.flap = 0.5;
            return;
        }
        d.record();
        self.remote_dragon_phase_tick();
        self.place_dragon_parts();
    }
}

#[cfg(test)]
#[path = "dragon/thorns_tests_client.rs"]
pub(in crate::game) mod thorns_tests;
