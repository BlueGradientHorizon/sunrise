#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{FallingBlock, Game, fireworks::fireworks_of};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_ticks::{
    TickRound, places_of, slot_place,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote_projectiles::has_smoke_trail;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote_projectiles::told_visit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote_projectiles::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::items::ItemStack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::join_seq::{JoinList, RestoreOrder};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::particles::{ParticleOptions, ParticleType};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::projectiles::{Projectile, ProjectileKind};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client_gameplay::remote::projectiles::{
    RemoteCloud, TRIDENT, cloud_client_tick, default_cloud_particle, projectile_kind,
    remote_projectile_item, remote_projectile_power, remote_projectile_step,
    shulker_bullet_trail_at,
};
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::client_side) use sunrise_client_gameplay::remote::projectiles::{
    remote_bullet_tick, remote_projectile_tick,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::Vec3, position::BlockPos, random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::entity_data::Value;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::registry::BlockStateId;

impl Game<'_> {
    /// `Arrow.tick` and `SpectralArrow.tick` on a client: a tipped arrow
    /// sheds two entity-effect swirls in its colour while flying, one every
    /// fifth tick in a block; a spectral arrow an effect sparkle at its
    /// position while flying.
    pub(in crate::game) fn arrow_effect_particles(&mut self) {
        let mut puffs: Vec<(Vec3, i32, u32)> = Vec::new();
        for p in &self.creatures.projectiles {
            let Some(a) = p.arrow.as_ref().filter(|_| p.is_arrow()) else {
                continue;
            };
            let at = p.position;
            if p.is_spectral_arrow() {
                if !p.stuck {
                    let options = ParticleOptions::simple(ParticleType::Effect);
                    self.particles.0.add(&options, [at.x, at.y, at.z], [0.0; 3]);
                }
                continue;
            }
            let count = match (p.stuck, a.in_ground_time % 5) {
                (true, 0) => 1,
                (true, _) => 0,
                (false, _) => 2,
            };
            puffs.push((at, a.effect_color, count));
        }
        for (at, color, count) in puffs {
            self.arrow_makeparticle(at, color, count);
        }
    }
    /// `Arrow.makeParticle`: `count` entity-effect swirls in `color` (an ARGB
    /// int, -1 for none) inside the arrow's 0.5 x 0.5 box.
    pub(in crate::game) fn arrow_makeparticle(&mut self, at: Vec3, color: i32, count: u32) {
        // `EntityType.ARROW` is 0.5 x 0.5.
        const SIZE: f64 = 0.5;
        if color == -1 {
            return;
        }
        // ColorParticleOption.create(ENTITY_EFFECT, colour): ARGB as floats.
        let channel = |shift: u32| (color as u32 >> shift & 255) as f32 / 255.0;
        let options = ParticleOptions::colored(
            ParticleType::EntityEffect,
            [channel(16), channel(8), channel(0), channel(24)],
        );
        for _ in 0..count {
            // getRandomX(0.5), getRandomY, getRandomZ(0.5).
            let r = self.particles.0.random();
            let x = at.x + SIZE * (2.0 * r.next_double() - 1.0) * 0.5;
            let y = at.y + SIZE * r.next_double();
            let z = at.z + SIZE * (2.0 * r.next_double() - 1.0) * 0.5;
            self.particles.0.add(&options, [x, y, z], [0.0; 3]);
        }
    }
    /// `DragonFireball.getTrailParticle`: dragon breath, where
    /// `createParticleTrail` puts every hurting projectile's trail.
    pub(in crate::game) fn dragon_breath_trails(&mut self) {
        let trail = ParticleOptions::simple(ParticleType::DragonBreath);
        let at: Vec<Vec3> = self
            .creatures
            .projectiles
            .iter()
            .filter(|p| p.kind == ProjectileKind::DragonFireball)
            .map(|p| p.position)
            .collect();
        for p in at {
            self.particles
                .0
                .add(&trail, [p.x, p.y + 0.5, p.z], [0.0; 3]);
        }
    }
    /// `ShulkerBullet.tick` on a client: two `END_ROD` particles in its
    /// 0.3125 box (`getRandomX(0.5)`, `getRandomY`, `getRandomZ(0.5)`).
    pub(in crate::game) fn shulker_bullet_trails(&mut self) {
        let bullets: Vec<Vec3> = self
            .creatures
            .projectiles
            .iter()
            .filter(|p| matches!(p.kind, ProjectileKind::ShulkerBullet { .. }))
            .map(|p| p.position)
            .collect();
        let options = ParticleOptions::simple(ParticleType::EndRod);
        for at in bullets {
            for _ in 0..2 {
                let p = shulker_bullet_trail_at(at, self.particles.0.random());
                self.particles.0.add(&options, p, [0.0; 3]);
            }
        }
    }
    /// `AbstractHurtingProjectile.createParticleTrail`: one smoke puff half a
    /// block above each position.
    pub(in crate::game) fn smoke_trails(&mut self, at: impl IntoIterator<Item = Vec3>) {
        let smoke = ParticleOptions::simple(ParticleType::Smoke);
        for p in at {
            self.particles
                .0
                .add(&smoke, [p.x, p.y + 0.5, p.z], [0.0; 3]);
        }
    }
    /// `spawn_remote_projectile` without first removing an entity under the same id: for
    /// `AddEntity` of an id the client does not track yet, which no list holds.
    pub(in crate::game) fn spawn_remote_projectile_inner(
        &mut self,
        kind: &str,
        packet: &sunrise_protocol::packets::play::AddEntity,
        rotation: (f32, f32),
    ) -> bool {
        let id = packet.id as u32;
        let position = packet.position;
        let velocity = packet.velocity;
        let data = packet.data;
        let at = Vec3::new(position[0], position[1], position[2]);
        let velocity = Vec3::new(velocity[0], velocity[1], velocity[2]);
        let (yaw, pitch) = rotation;
        match kind {
            "minecraft:evoker_fangs" => self.insert_fangs(id, position, yaw, 0, 0),
            "minecraft:tnt" => self.tnts.push(super::tnt::PrimedTnt {
                size_answer: None,
                attachment_lifetime: Default::default(),
                rotation: [0.0, 0.0],
                join: Default::default(),
                no_gravity: false,
                id,
                position: at,
                previous: at,
                velocity,
                fuse: 80,
                ticks: 0,
                ground: Default::default(),
                fall_distance: 0.0,
                stuck: Vec3::ZERO,
                in_water: false,
                owner: None,
                guest: None,
                power: 4.0,
                used_portal: false,
                block_state: None,
            }),
            // FallingBlockEntity.recreateFromPacket: the block state is the data.
            "minecraft:falling_block" => self.falling.push(FallingBlock {
                size_answer: None,
                attachment_lifetime: Default::default(),
                rotation: [0.0, 0.0],
                join: Default::default(),
                id,
                state: BlockStateId(data.max(0) as u32),
                blocks_building: true,
                start_pos: BlockPos::default(),
                position: at,
                previous: at,
                velocity,
                age: 0,
                no_gravity: false,
                portal: Default::default(),
                drop_item: false,
                fall_distance: 0.0,
                ground: Default::default(),
                fire_ticks: 0,
                sync_velocity: false,
                stuck: Vec3::ZERO,
                damage: Default::default(),
                saved: None,
            }),
            "minecraft:trident" => {
                let mut t = super::trident::ThrownTrident::remote(id, at, yaw, pitch);
                t.velocity = velocity;
                // Projectile.getAddEntityPacket: the owner's id is the data.
                t.owner = data as u32;
                self.tridents.push(t);
            }
            "minecraft:firework_rocket" => {
                self.rockets
                    .push(super::fireworks::Rocket::remote(id, at, velocity));
            }
            "minecraft:splash_potion"
            | "minecraft:lingering_potion"
            | "minecraft:experience_bottle" => {
                self.potions.add_remote_thrown(id, kind, at, velocity);
            }
            "minecraft:area_effect_cloud" => self.potions.clouds.push(RemoteCloud {
                id,
                at: position,
                radius: 3.0,
                waiting: false,
                particle: default_cloud_particle(),
                random: sunrise_client::random_source::entity(),
            }),
            "minecraft:eye_of_ender" => {
                self.end
                    .add_remote_eye(id, position, [velocity.x, velocity.y, velocity.z])
            }
            // FishingHook.recreateFromPacket: the owner's id is the data.
            "minecraft:fishing_bobber" => self.spawn_remote_hook(
                id,
                sunrise_client::lan::playerdata::uuid_ints(packet.uuid),
                at,
                velocity,
                data,
            ),
            "minecraft:end_crystal" => {
                // EndCrystal's constructor starts `time` from the crystal's own
                // random, so each placed crystal spins from its own phase. The id is the
                // server's, given in the order its chunks load, so under `SUNRISE_STILL` the
                // position alone seeds it.
                let drawn = sunrise_client::mobs::entity_random(id, at)
                    .next_int_bound(100_000)
                    .unwrap_or(0);
                let time = super::dragon::Crystal::start_time(at, drawn, self.clock.still());
                self.crystals
                    .push(super::dragon::Crystal::new(id, at, true, time));
            }
            _ => {
                let Some(k) = projectile_kind(kind) else {
                    return false;
                };
                if k == ProjectileKind::LlamaSpit {
                    self.particles
                        .0
                        .llama_spit_burst(position, [velocity.x, velocity.y, velocity.z]);
                }
                let mut p = Projectile::new(k, at, None);
                // SpectralArrow: its own type; its pickup item says so.
                if kind == "minecraft:spectral_arrow"
                    && let Some(a) = p.arrow.as_mut()
                {
                    a.pickup_item = ItemStack::new("minecraft:spectral_arrow", 1);
                }
                p.id = id;
                p.velocity = velocity;
                p.yaw = yaw;
                p.pitch = pitch;
                self.add_projectile(p);
            }
        }
        true
    }
    pub(in crate::game) fn throwable_bubbles(&mut self, at: Vec<([f64; 3], [f64; 3])>) {
        let bubble = ParticleOptions::simple(ParticleType::Bubble);
        for (position, speed) in at {
            for _ in 0..4 {
                self.particles.0.add(&bubble, position, speed);
            }
        }
    }
    /// `AreaEffectCloud.clientTick` for each cloud a server tracks.
    pub(in crate::game) fn tick_remote_clouds(&mut self) {
        for c in &mut self.potions.clouds {
            cloud_client_tick(
                &mut self.particles.0,
                &self.world,
                c.at,
                c.radius,
                c.waiting,
                &c.particle,
                &mut c.random,
            );
        }
    }
}
