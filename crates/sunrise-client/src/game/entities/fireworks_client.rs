#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game, Hit, dragon::FightWho, item_draw::owner_at, view_vector,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::FIREWORK_EXPLODE_EVENT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::Rocket;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::blast_damage;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::fireworks_of;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::rocket_draws;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::use_on_passes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_assets::item_model::DisplayContext;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
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
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::RandomSource,
};

impl Game<'_> {
    /// `FireworkRocketEntity.explode` and `dealExplosionDamage`.
    pub(in crate::game) fn explode_rocket(&mut self, r: &Rocket) {
        let at = [r.position.x, r.position.y, r.position.z];
        // FireworkRocketEntity.explode broadcasts before discard. Send now:
        // tick_lan removes missing tracked entities before flushing effects.
        if r.id != 0
            && let Some(host) = &self.lan
        {
            host.hub.entity_event(r.id as i32, FIREWORK_EXPLODE_EVENT);
        }
        // `this.gameEvent(GameEvent.EXPLODE, this.getOwner())`, before the blast.
        self.rocket_explode_event(r.owner, r.guest, at);
        if !r.explosions.is_empty() {
            let damage = 5.0 + r.explosions.len() as f32 * 2.0;
            let from = r.position;
            // `attachedToEntity` takes the full damage first, before the AABB LivingEntity sweep.
            let boosted = r.guest.filter(|_| r.attached);
            if r.attached && boosted.is_none() {
                self.rocket_hurt_player(from, damage);
            }
            if let Some(conn) = boosted {
                self.as_guest(conn, |g| g.rocket_hurt_player(from, damage));
            }
            let reach = r.bounds().inflate(Vec3::new(5.0, 5.0, 5.0));
            enum SplashTarget {
                Host,
                Guest(u32),
                Mob(u32),
                Dragon,
                Stand(u32),
            }
            let mut candidates: Vec<(sunrise_client::projectiles::TargetOrder, SplashTarget)> =
                Vec::new();
            if !(r.attached && boosted.is_none())
                && !self.spectator()
                && let Some(p) = self.player_view()
                && p.bounds.intersects(reach)
            {
                let pos = self.player.as_ref().map_or(Vec3::ZERO, |pl| pl.position);
                candidates.push((
                    sunrise_client::projectiles::TargetOrder::entity(pos, 0),
                    SplashTarget::Host,
                ));
            }
            if let Some(host) = self.lan.as_ref() {
                for g in &host.hub.guests {
                    if host.hub.here(g)
                        && !g.spectator()
                        && Some(g.conn) != boosted
                        && super::lan_mobs::guest_bounds(g).intersects(reach)
                    {
                        let pos = Vec3::new(g.position[0], g.position[1], g.position[2]);
                        candidates.push((
                            sunrise_client::projectiles::TargetOrder::entity(
                                pos,
                                g.entity_id as u64,
                            ),
                            SplashTarget::Guest(g.conn),
                        ));
                    }
                }
            }
            for m in self.mobs.iter() {
                if !m.removed && m.bounds().intersects(reach) {
                    candidates.push((
                        sunrise_client::projectiles::TargetOrder::entity(m.position, m.join.get()),
                        SplashTarget::Mob(m.id),
                    ));
                }
            }
            if let Some(d) = self.dragon.as_ref().filter(|d| d.alive())
                && d.body_bounds().intersects(reach)
            {
                candidates.push((
                    sunrise_client::projectiles::TargetOrder::entity(d.position, u64::from(d.id)),
                    SplashTarget::Dragon,
                ));
            }
            for a in self.armor_stands.iter() {
                if a.bounds().intersects(reach) {
                    candidates.push((
                        sunrise_client::projectiles::TargetOrder::entity(a.position, a.join.get()),
                        SplashTarget::Stand(a.id),
                    ));
                }
            }
            candidates.sort_by_key(|(order, _)| *order);
            let owner = r.owner;
            let by = (owner == Some(Who::Player)).then(|| {
                r.guest
                    .map_or_else(|| self.acting_player(), FightWho::Guest)
            });
            let causing = match owner {
                Some(Who::Player) => Some(r.guest.map_or(
                    super::commands::Target::Player,
                    super::commands::Target::Guest,
                )),
                Some(Who::Mob(id)) => Some(super::commands::Target::Mob(id)),
                None => None,
            };
            for (_, target) in candidates {
                match target {
                    SplashTarget::Host => self.rocket_blast_player(from, damage),
                    SplashTarget::Guest(conn) => {
                        self.as_guest(conn, |g| g.rocket_blast_player(from, damage));
                    }
                    SplashTarget::Mob(id) => {
                        let hit = {
                            let world = self.mob_world_base(&[]);
                            self.mobs
                                .iter()
                                .find(|m| m.id == id && !m.removed)
                                .and_then(|m| {
                                    let d2 = (m.position.x - from.x).powi(2)
                                        + (m.position.y - from.y).powi(2)
                                        + (m.position.z - from.z).powi(2);
                                    let sees = d2 <= 25.0
                                        && (0..2).any(|step| {
                                            let to = Vec3::new(
                                                m.position.x,
                                                m.position.y
                                                    + f64::from(m.height()) * 0.5 * f64::from(step),
                                                m.position.z,
                                            );
                                            world.clear_between(from, to, false)
                                        });
                                    sees.then(|| (m.id, blast_damage(damage, from, m.position)))
                                })
                        };
                        if let Some(hit) = hit {
                            let hits = [hit];
                            if let Some(conn) = r.guest.filter(|_| owner.is_some()) {
                                let ran = self
                                    .as_guest(conn, |g| g.rocket_hurt_mobs(&hits, r.id, owner, at))
                                    .is_some();
                                if ran
                                    && let Some(host) = self.lan.as_mut()
                                    && host
                                        .hub
                                        .guest(conn)
                                        .is_some_and(|guest| guest.participates_online())
                                    && self.mobs.iter().any(|m| m.id == hit.0)
                                {
                                    host.mob_players.insert(hit.0, conn);
                                }
                                if !ran {
                                    self.rocket_hurt_mobs(&hits, r.id, owner, at);
                                }
                            } else {
                                self.rocket_hurt_mobs(&hits, r.id, owner, at);
                            }
                        }
                    }
                    SplashTarget::Dragon => {
                        self.rocket_blast_dragon(from, damage, reach, by);
                    }
                    SplashTarget::Stand(id) => {
                        let sees = {
                            let world = self.mob_world_base(&[]);
                            self.armor_stands
                                .iter()
                                .find(|a| a.id == id)
                                .is_some_and(|a| {
                                    let d2 = (a.position.x - from.x).powi(2)
                                        + (a.position.y - from.y).powi(2)
                                        + (a.position.z - from.z).powi(2);
                                    if d2 > 25.0 {
                                        return false;
                                    }
                                    let bounds = a.bounds();
                                    let height = bounds.max_position().y - bounds.min_position().y;
                                    (0..2).any(|step| {
                                        let to = Vec3::new(
                                            a.position.x,
                                            a.position.y + height * 0.5 * f64::from(step),
                                            a.position.z,
                                        );
                                        world.clear_between(from, to, false)
                                    })
                                })
                        };
                        if sees {
                            self.hurt_armor_stand(
                                id,
                                "minecraft:fireworks",
                                causing,
                                r.id,
                                Some(at),
                            );
                        }
                    }
                }
            }
        }
        let camera = self.eye_position().unwrap_or(at);
        let v = [r.velocity.x, r.velocity.y, r.velocity.z];
        self.native_firework_explode(r.id, at, r.explosions.len());
    }
    /// FireworkRocketEntity.handleEntityEvent: use the synchronized item data.
    pub(in crate::game::client_side) fn remote_firework_explosion(&mut self, id: i32) {
        let camera = self.eye_position();
        if let Some(r) = self.rockets.iter().find(|r| r.id == id as u32) {
            let at = [r.position.x, r.position.y, r.position.z];
            let v = [r.velocity.x, r.velocity.y, r.velocity.z];
            self.particles.0.fireworks(
                at,
                v,
                r.explosions.as_ref().clone(),
                camera.unwrap_or(at),
                !r.silent,
            );
        }
    }
    /// The trail (`FireworkRocketEntity.tick` on a client): a spark each tick
    /// at the rocket, drifting against its motion.
    pub(in crate::game::client_side) fn rocket_spark(&mut self, r: &Rocket) {
        let g = |g: &mut sunrise_core::random::LegacyRandom| g.next_gaussian() * 0.05;
        let v = [
            g(&mut self.world.random),
            -r.velocity.y * 0.5,
            g(&mut self.world.random),
        ];
        let at = [r.position.x, r.position.y, r.position.z];
        self.particles
            .0
            .add(&ParticleOptions::simple(ParticleType::Firework), at, v);
    }
    /// One tick of `r`, a clone of the resident rocket at `index`. Returns whether the
    /// rocket stays in the list; one that explodes or was removed has left it.
    pub(in crate::game) fn tick_rocket(
        &mut self,
        index: usize,
        mut r: Rocket,
        hears_impacts: bool,
    ) -> bool {
        let (id, join) = (r.id, r.join);
        let (mut old_rotation, hit) =
            self.with_entity_held_out(id, |game| game.advance_rocket(&mut r));
        let deflected = matches!(hit, ProjectileHit::Entity { who: Who::Mob(id), .. }
            if self.mobs.iter().any(|m| m.id == id && m.alive() && m.species() == Species::Breeze));
        let canceled = if !deflected && hears_impacts && !matches!(hit, ProjectileHit::None) {
            let Some((rocket, canceled)) = self.rocket_impact(index, r, hit) else {
                return false;
            };
            r = rocket;
            canceled
        } else {
            false
        };
        let explodes_on_hit = !canceled
            && !deflected
            && match hit {
                ProjectileHit::Entity { .. } => true,
                ProjectileHit::Block { .. } => !r.explosions.is_empty(),
                ProjectileHit::None => false,
            };
        let terminal = explodes_on_hit || r.life + 1 > r.lifetime;
        let _save_lease = terminal.then(sunrise_client::join_seq::visibility::PassLease::enter);
        if terminal {
            let Some(mut owner) = self.rockets.extract(join) else {
                return false;
            };
            sunrise_client::join_seq::store_back(&mut owner, r);
            r = owner;
        }
        if !canceled {
            if let ProjectileHit::Entity {
                who: Who::Mob(id),
                at,
            } = hit
                && deflected
            {
                if r.last_deflected_by != Some(id) {
                    let rot = 170.0 + r.random.next_float() * 20.0;
                    r.rotation[0] += rot;
                    old_rotation[0] += rot;
                    r.velocity = r.velocity.scale(-0.5);
                    r.last_deflected_by = Some(id);
                    let sound = super::level_sounds::LevelSound::new(
                        "minecraft:entity.breeze.deflect",
                        [at.x, at.y, at.z],
                        1.0,
                        1.0,
                    )
                    .in_source(mod_api::SoundSource::Hostile);
                    self.play_bound_sound(sound, id, None);
                }
            } else {
                match hit {
                    ProjectileHit::Entity { who, at } => {
                        if let Who::Mob(id) = who {
                            self.redirect_projectile(id, r.owner, r.owner_uuid, r.guest, None);
                        }
                        self.explode_rocket(&r);
                        self.projectile_landed_event(r.id, r.owner, [at.x, at.y, at.z]);
                    }
                    ProjectileHit::Block { pos, face, at } => {
                        if face != [0; 3] {
                            let hitter = super::creatures::ProjectileHitter {
                                owner: r.owner,
                                guest: r.guest,
                                arrow: false,
                                kind: "minecraft:firework_rocket",
                                position: r.position,
                            };
                            let _ = self.projectile_block_impact(&hitter, pos, at, face);
                            self.target_block_hit(&hitter, pos, at, face);
                        }
                        if !r.explosions.is_empty() {
                            self.explode_rocket(&r);
                        }
                        self.projectile_block_landed_event(r.id, r.owner, pos);
                    }
                    ProjectileHit::None => {}
                }
            }
        }
        // FireworkRocketEntity.tick runs updateRotation, launch sound, life++, spark even after impact.
        r.update_rotation(old_rotation);
        let at = [r.position.x, r.position.y, r.position.z];
        if r.life == 0 && !r.silent {
            self.server_play_sound("minecraft:entity.firework_rocket.launch", at, 3.0, 1.0);
        }
        r.life += 1;
        if terminal {
            if !explodes_on_hit {
                self.explode_rocket(&r);
            }
            self.rockets.retire_extracted(id, join);
            return false;
        }
        let (from, to) = (r.previous, r.position);
        if !self.store_rocket_back(index, r) {
            return false;
        }
        self.entity_section_step(id, [from.x, from.y, from.z], [to.x, to.y, to.z]);
        true
    }
}

#[cfg(test)]
#[path = "tests/fireworks_events_tests_client.rs"]
pub(in crate::game) mod events_tests;

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::fireworks::tests::*;

    /// A flying rocket's vertices carry its own id for frame generation's and a temporal
    /// upscaler's motion vectors (`Frame::entity_motion`), so the motion matched between
    /// two frames is its flight, not only the camera's.
    #[test]
    #[ignore = "daily"]
    fn a_flying_rocket_opens_a_motion_run_of_its_own() {
        use crate::game::{Audio, Options, Progress, Settings, load_resources};
        use sunrise_core::geometry::Vec3;

        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        let res = loaded.resources;
        let mut game = super::Game::new(
            &res,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.clone(),
            )),
            None,
        )
        .unwrap();
        let camera = glam::DVec3::new(0.5, 64.0, 0.5);
        let start = Vec3::new(0.5, 64.0, 4.5);
        game.rockets
            .push(super::Rocket::remote(77, start, Vec3::ZERO));
        let id = super::Game::MOTION_OBJECT | 77;
        // One frame's motion runs: the rocket's vertices open a run under its own id, and
        // a camera-only run (id 0) closes it so nothing drawn after takes its motion.
        game.client.entity_motion.borrow_mut().clear();
        let mut out = Vec::new();
        game.rocket_geometry(camera, 1.0, &mut out, 0..game.rockets.len());
        let marks = game.client.entity_motion.borrow().clone();
        let at = marks
            .iter()
            .position(|m| m.1 == id)
            .expect("the rocket's run");
        assert_eq!(
            marks[at].0, 0,
            "the run starts at the rocket's first vertex"
        );
        assert_eq!(marks.last(), Some(&(out.len() as u32, 0)));
    }
}
