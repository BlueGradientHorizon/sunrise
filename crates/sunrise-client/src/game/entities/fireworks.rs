//! Firework rockets (`FireworkRocketItem`, `FireworkRocketEntity`): launching
//! from a block face, boosting a gliding player, the 1.15 horizontal
//! acceleration and 0.04 lift, the flight-duration lifetime, explosion
//! damage with line of sight, and the client explosion (`createFireworks`).
#[allow(unused_imports)]
use super::{Game, Hit, dragon::FightWho, item_draw::owner_at, view_vector};
#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use sunrise_assets::item_model::DisplayContext;
#[allow(unused_imports)]
use sunrise_client::{
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
use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::RandomSource,
};

pub(in crate::game) const FIREWORK_EXPLODE_EVENT: i8 = 17;

#[path = "fireworks/impact.rs"]
pub(in crate::game) mod impact;

#[derive(Clone)]
pub(in crate::game) struct Rocket {
    /// Entity-owned registration metadata; content clones are detached.
    pub(in crate::game) attachment_lifetime: sunrise_mod_runtime::attachments::AttachmentLifetime,
    /// `Entity.yRot` and `xRot` (`/tp`, `/data get ... Rotation`).
    pub rotation: [f32; 2],
    /// When it joined its list (`sunrise_client::join_seq`); set by the list.
    pub(in crate::game) join: sunrise_client::join_seq::JoinSeq,
    /// Its entity id: from the entity counter on the server, the server's on
    /// a client.
    pub id: u32,
    pub(in crate::game) position: Vec3,
    pub(in crate::game) previous: Vec3,
    pub(in crate::game) velocity: Vec3,
    pub(in crate::game) life: i32,
    pub(in crate::game) lifetime: i32,
    pub(in crate::game) explosions: Arc<Vec<FireworkExplosion>>,
    /// Boosting the player (attachedToEntity).
    pub(in crate::game) attached: bool,
    /// On a server, the guest (by connection) who launched it, or whom it
    /// boosts: `owner`'s `Who::Player` then means them, not the host.
    pub(in crate::game) guest: Option<u32>,
    /// `Projectile.getOwner`: the player who launched it, credited with
    /// what its blast hurts.
    pub(in crate::game) owner: Option<Who>,
    /// `Projectile.getOwner` when `owner` can't name it: the UUID of an owner
    /// that is neither a player nor a mob.
    pub(in crate::game) owner_uuid: Option<[i32; 4]>,
    /// `Projectile.leftOwner`: until then it can't hit its owner.
    pub(in crate::game) left_owner: bool,
    /// `Projectile.lastDeflectedBy`: prevents repeated deflection by the same mob.
    pub(in crate::game) last_deflected_by: Option<u32>,
    /// `DATA_ATTACHED_TO_TARGET`: the server entity the rocket follows;
    /// boosting happens when it is the local player.
    pub(in crate::game) attached_to: Option<u32>,
    /// `DATA_SHOT_AT_ANGLE` (dispensers, crossbows): constant velocity.
    pub(in crate::game) angled: bool,
    /// `DATA_ID_FIREWORKS_ITEM`: the stack it was launched from.
    pub(in crate::game) item: Arc<ItemStack>,
    /// `Entity.random`: what a mod's `Projectile.deflect` draws from.
    pub(in crate::game) random: sunrise_core::random::LegacyRandom,
    /// `Entity.onGround`: whether its last move was stopped going down (or
    /// the one it was loaded with, until it moves).
    pub(in crate::game) ground: sunrise_world::ground_support::GroundState,
    /// `Projectile.hasBeenShot`: its first tick posts `projectile_shoot`.
    pub(in crate::game) has_been_shot: bool,
    /// `Entity.isSilent`: no launch sound.
    pub(in crate::game) silent: bool,
    /// `Entity.fallDistance`.
    pub(in crate::game) fall_distance: f64,
    /// `Entity.stuckSpeedMultiplier`: a cobweb or powder snow it went through
    /// slows its next move.
    pub(in crate::game) stuck: Vec3,
    /// `Entity.wasTouchingWater`: entering water splashes.
    pub(in crate::game) was_touching_water: bool,
    /// `Entity.firstTick`: no splash on the tick it appears in water.
    pub(in crate::game) first_tick: bool,
    /// What `entity_size` answered at its last size refresh (`EntityEvent.Size`): its box takes
    /// the answer while it holds for its vanilla size.
    pub(in crate::game) size_answer: Option<sunrise_client::pose::SizeAnswer>,
}
impl Rocket {
    /// A rocket a server sent (`handleAddEntity`): placed and moved by it.
    pub(in crate::game) fn remote(id: u32, position: Vec3, velocity: Vec3) -> Self {
        let item = ItemStack::new("minecraft:firework_rocket", 1);
        let (_, explosions) = fireworks_of(&item);
        Self {
            size_answer: None,
            attachment_lifetime: Default::default(),
            rotation: [0.0, 0.0],
            join: Default::default(),
            id,
            position,
            previous: position,
            velocity,
            life: 0,
            lifetime: i32::MAX,
            explosions: explosions.into(),
            attached: false,
            guest: None,
            owner: None,
            owner_uuid: None,
            left_owner: false,
            last_deflected_by: None,
            attached_to: None,
            angled: false,
            item: item.into(),
            random: sunrise_client::projectiles::unique_random(position),
            ground: Default::default(),
            // The server posts the shot.
            has_been_shot: true,
            silent: false,
            fall_distance: 0.0,
            stuck: Vec3::ZERO,
            was_touching_water: false,
            first_tick: true,
        }
    }
    /// `Projectile.setOwner`; the UUID is kept only for an owner `who` can't name.
    pub(in crate::game) fn set_owner(
        &mut self,
        who: Option<Who>,
        uuid: Option<[i32; 4]>,
        guest: Option<u32>,
    ) {
        (self.owner, self.guest) = (who, guest);
        self.owner_uuid = uuid.filter(|_| who.is_none());
    }
    /// `FireworkRocketEntity.addAdditionalSaveData`; the owner when it is
    /// `player`, or an entity that is neither a player nor a mob.
    pub(in crate::game) fn to_nbt(
        &self,
        uuid: [i32; 4],
        player: Option<[i32; 4]>,
        raw: Option<&sunrise_core::nbt::Tag>,
        items: &crate::items::Items,
    ) -> sunrise_core::nbt::Tag {
        sunrise_client::entity_nbt::firework_rocket_to_nbt(
            &sunrise_client::entity_nbt::RocketSave {
                rotation: self.rotation,
                position: self.position,
                motion: self.velocity,
                life: self.life,
                lifetime: self.lifetime,
                item: &self.item,
                angled: self.angled,
                owner: if matches!(self.owner, Some(Who::Player)) {
                    player
                } else {
                    self.owner_uuid
                },
                left_owner: self.left_owner,
                on_ground: self.ground.on_ground,
                has_been_shot: self.has_been_shot,
                fall_distance: self.fall_distance,
            },
            uuid,
            raw,
            items,
        )
    }
    /// A rocket loaded from its NBT (`/summon`, a saved chunk).
    pub(in crate::game) fn from_nbt(r: RocketNbt) -> Self {
        let item = r
            .item
            .unwrap_or_else(|| ItemStack::new("minecraft:firework_rocket", 1));
        let (_, explosions) = fireworks_of(&item);
        Self {
            size_answer: None,
            attachment_lifetime: Default::default(),
            rotation: r.rotation,
            join: Default::default(),
            id: 0,
            position: r.position,
            previous: r.position,
            velocity: r.motion,
            life: r.life,
            lifetime: r.lifetime,
            explosions: explosions.into(),
            attached: false,
            guest: None,
            owner: r.owner,
            owner_uuid: r.owner_uuid.filter(|_| r.owner.is_none()),
            left_owner: r.left_owner,
            last_deflected_by: None,
            attached_to: None,
            angled: r.angled,
            item: item.into(),
            random: sunrise_client::projectiles::unique_random(r.position),
            ground: sunrise_world::ground_support::GroundState::from_ground_flag(r.on_ground),
            has_been_shot: r.has_been_shot,
            silent: r.silent,
            fall_distance: r.fall_distance,
            stuck: Vec3::ZERO,
            was_touching_water: false,
            first_tick: true,
        }
    }
    /// `Entity.load` over a live rocket (`/data merge`, `/data modify`): what
    /// [`Self::from_nbt`] reads replaces its own; it keeps its id and what it
    /// boosts.
    pub(in crate::game) fn load_nbt(&mut self, r: RocketNbt) {
        let loaded = Self::from_nbt(r);
        self.rotation = loaded.rotation;
        self.has_been_shot = loaded.has_been_shot;
        self.silent = loaded.silent;
        self.position = loaded.position;
        self.previous = loaded.position;
        self.velocity = loaded.velocity;
        self.life = loaded.life;
        self.lifetime = loaded.lifetime;
        self.explosions = loaded.explosions;
        self.item = loaded.item;
        self.angled = loaded.angled;
        self.left_owner = loaded.left_owner;
        self.ground = loaded.ground;
        self.fall_distance = loaded.fall_distance;
    }
    /// One tick of a server's client: the rocket flies on by itself between
    /// the server's updates (a rocket that follows an entity is placed by the
    /// caller).
    pub(in crate::game) fn remote_step(&mut self, motion: sunrise_client::motion::BodyMotion<'_>) {
        self.previous = self.position;
        if self.attached_to.is_none() {
            let mut scene = SceneCollisions {
                world: motion.world,
                binding: motion.collision,
                subject: None,
            };
            if let Some((_, wanted, moved, restored)) = self.free_flight(&mut scene, motion.context)
            {
                let movement = self.flight_contact(motion, wanted, moved);
                self.velocity = motion
                    .restitute(
                        (self.id, "minecraft:firework_rocket"),
                        movement,
                        0.0,
                        0.98_f32,
                        self.velocity,
                        self.fall_distance,
                    )
                    .velocity;
                motion.apply_block_speed(self.position, &mut self.velocity);
                motion.step_on(
                    (self.id, "minecraft:firework_rocket"),
                    (self.position, self.bounds()),
                    &self.ground,
                    &mut self.velocity,
                );
                self.velocity = restored;
            }
        }
        self.life += 1;
    }
    /// The end of `Projectile.shoot`: yaw and pitch are set straight from the
    /// shot's motion (the old rotation follows, as `rotation` is the old one
    /// on the next tick).
    pub(in crate::game) fn turn_to_shot(&mut self) {
        let v = self.velocity;
        let horizontal = (v.x * v.x + v.z * v.z).sqrt();
        let atan2 = sunrise_core::geometry::trig::atan2_degrees;
        self.rotation = [atan2(v.x, v.z), atan2(v.y, horizontal)];
    }
    /// The move of a rocket attached to no entity: sped up and rising, or an
    /// angled one's own motion.
    pub(in crate::game) fn next_motion(&self) -> Vec3 {
        if self.angled {
            self.velocity
        } else {
            let acceleration = if self.ground.horizontal_collision {
                1.0
            } else {
                1.15
            };
            Vec3::new(
                self.velocity.x * acceleration,
                self.velocity.y + 0.04,
                self.velocity.z * acceleration,
            )
        }
    }
    /// `EntityType.FIREWORK_ROCKET.getDimensions()`: width, height and eye height.
    pub(in crate::game) const DIMENSIONS: [f32; 3] = [0.25, 0.25, 0.2125];
    /// `EntityType.FIREWORK_ROCKET`: 0.25 x 0.25.
    pub(in crate::game) fn bounds(&self) -> Aabb {
        let vanilla = Self::bounds_at(self.position);
        sunrise_client::pose::SizeAnswer::sized_box(
            self.size_answer,
            Self::DIMENSIONS,
            self.position,
            vanilla,
        )
    }
    /// [`Self::bounds`] at `position`.
    pub(in crate::game) fn bounds_at(position: Vec3) -> Aabb {
        let h = 0.125;
        Aabb::new(
            Vec3::new(position.x - h, position.y, position.z - h),
            Vec3::new(position.x + h, position.y + 2.0 * h, position.z + h),
        )
    }
    /// `FireworkRocketEntity.tick` for a rocket attached to no entity: speed
    /// up (an angled one keeps its motion) and rise, then move (`Entity.move`:
    /// a cobweb or powder snow scales this move only). Returns where it moved
    /// from, the move it asked for and the one it made.
    pub(in crate::game) fn free_flight(
        &mut self,
        scene: &mut SceneCollisions,
        context: &sunrise_world::native_load_context::LoopContext,
    ) -> Option<(Vec3, Vec3, Vec3, Vec3)> {
        let bounds = self.bounds();
        let motion = self.next_motion();
        let mut wanted = motion;
        self.velocity = motion;
        if self.stuck.length_squared() > 1.0e-7 {
            wanted = Vec3::new(
                wanted.x * self.stuck.x,
                wanted.y * self.stuck.y,
                wanted.z * self.stuck.z,
            );
            self.stuck = Vec3::ZERO;
            self.velocity = Vec3::ZERO;
        }
        let from = self.position;
        let mut gate = sunrise_client::move_gate::armed(
            context,
            self.id,
            "minecraft:firework_rocket",
            sunrise_world::entity_move::MoverType::SelfMovement,
            wanted,
        );
        let wanted = gate.delta();
        let Ok(moved) =
            sunrise_client::move_gate::scene_resolve(&mut gate, scene, bounds, wanted, 0.0, false)
        else {
            return None;
        };
        self.position = Vec3::new(
            self.position.x + moved.x,
            self.position.y + moved.y,
            self.position.z + moved.z,
        );
        Some((from, wanted, moved, motion))
    }

    pub(in crate::game) fn flight_contact(
        &mut self,
        motion: sunrise_client::motion::BodyMotion<'_>,
        wanted: Vec3,
        moved: Vec3,
    ) -> sunrise_client::motion::BodyMove {
        let authoritative =
            motion.context.side() == sunrise_world::native_load_context::LoopSide::Server;
        motion.record_move(
            sunrise_client::motion::BodyCollision {
                footprint: (self.position, self.bounds()),
                movement: (wanted, moved),
                authoritative,
                context: sunrise_client::motion::BodyMotion::entity_context(self.id, self.position),
                subject: None,
            },
            &mut self.ground,
        );
        let effect = motion.legacy_contact(self.position, &self.ground);
        sunrise_client::motion::BodyMove {
            wanted,
            moved,
            effect,
        }
    }
}
/// `damageAmount * (float)Math.sqrt((5.0 - this.distanceTo(target)) / 5.0)`
/// from the rocket at `from` to a target at `to`; `Entity.distanceTo` sums
/// the squared offsets in floats.
pub(in crate::game) fn blast_damage(damage: f32, from: Vec3, to: Vec3) -> f32 {
    let (x, y, z) = (
        (from.x - to.x) as f32,
        (from.y - to.y) as f32,
        (from.z - to.z) as f32,
    );
    let distance = f64::from(x * x + y * y + z * z).sqrt() as f32;
    damage * ((5.0 - f64::from(distance)) / 5.0).sqrt() as f32
}
pub(in crate::game) fn fireworks_of(stack: &ItemStack) -> (u8, Vec<FireworkExplosion>) {
    // The item's default component: flight 1, no explosions.
    stack
        .data()
        .and_then(|d| d.fireworks.clone())
        .unwrap_or((1, Vec::new()))
}
/// `FireworkRocketEntity(Level, double, double, double, ItemStack)`, after the UUID:
/// the motion's two triangles, then the lifetime's two `nextInt`s, all from `random`.
pub(in crate::game) fn rocket_draws(
    random: &mut sunrise_core::random::LegacyRandom,
    flight: u8,
) -> (Vec3, i32) {
    let tri = |r: &mut sunrise_core::random::LegacyRandom| r.triangle(0.0, 0.002297);
    let velocity = Vec3::new(tri(random), 0.05, tri(random));
    let lifetime = 10 * (1 + i32::from(flight))
        + random.next_int_bound(6).unwrap_or(0)
        + random.next_int_bound(7).unwrap_or(0);
    (velocity, lifetime)
}
impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// A rocket built from `own`, the rocket's own random (`Entity.random`): the
    /// UUID, the motion and the lifetime draw from it in that order, and a rocket
    /// that is then shot (`Projectile.shoot`) goes on drawing from the same one.
    /// `Entity.getHandHoldingItemAngle(Items.FIREWORK_ROCKET)` for the local
    /// player: half a block out towards the hand holding the rocket while
    /// gliding, zero otherwise. A rocket only in the offhand is in the arm
    /// opposite the main one.
    pub(in crate::game) fn rocket_hand_angle(&self) -> [f64; 3] {
        if !self.player.as_ref().is_some_and(|p| p.fall_flying) {
            return [0.0; 3];
        }
        let is_rocket =
            |s: Option<&ItemStack>| s.is_some_and(|s| &*s.item == "minecraft:firework_rocket");
        let main = self.stack_in(self.selected);
        let offhand_only = is_rocket(self.offhand.as_ref()) && !is_rocket(main);
        let right = offhand_only == self.settings.main_hand_left;
        let arm = if right { 80 } else { -80 };
        let v = view_vector(self.yaw + arm as f32, 0.0);
        [v[0] * 0.5, v[1] * 0.5, v[2] * 0.5]
    }
    /// `FireworkRocketEntity.tick` for a rocket attached to the local player:
    /// a glide is boosted towards the look, and the rocket sits at the rocket
    /// hand with the player's motion.
    pub(in crate::game) fn follow_own_player(&mut self, r: &mut Rocket) {
        let hand = self.rocket_hand_angle();
        // getLookAngle reads Mth's float sine table, as the server does; the
        // f64 sin/cos of super::view_vector drift the boost off the server's
        // path and trip the movement check while gliding under a rocket.
        let look = sunrise_world::view_direction::view_vector(self.pitch, self.yaw);
        let Some(p) = self.player.as_mut() else {
            return;
        };
        if p.fall_flying {
            let m = p.velocity;
            p.velocity = Vec3::new(
                m.x + look.x * 0.1 + (look.x * 1.5 - m.x) * 0.5,
                m.y + look.y * 0.1 + (look.y * 1.5 - m.y) * 0.5,
                m.z + look.z * 0.1 + (look.z * 1.5 - m.z) * 0.5,
            );
        }
        r.position = p.position.add_vector(Vec3::new(hand[0], hand[1], hand[2]));
        r.velocity = p.velocity;
    }
}

/// `FireworkRocketItem.useOn` returns PASS while the player is fall flying,
/// leaving the boost to `use` (a guest's UseItemOn must not launch a rocket).
pub(in crate::game) fn use_on_passes(fall_flying: bool) -> bool {
    fall_flying
}

#[cfg(test)]
#[path = "tests/fireworks_events_tests.rs"]
pub(in crate::game) mod events_tests;

#[cfg(test)]
pub(in crate::game) mod tests {

    /// `Projectile.shoot` turns a shot rocket to its motion at once.
    #[test]
    fn a_shot_rocket_starts_turned_to_its_motion() {
        use sunrise_core::geometry::Vec3;
        let mut rocket = super::Rocket::remote(1, Vec3::ZERO, Vec3::new(0.0, 0.5, 0.0));
        rocket.turn_to_shot();
        assert!((rocket.rotation[1] - 90.0).abs() < 1e-3);
        let mut rocket = super::Rocket::remote(1, Vec3::ZERO, Vec3::new(1.0, 0.0, 1.0));
        rocket.turn_to_shot();
        assert!((rocket.rotation[0] - 45.0).abs() < 1e-4);
        assert!(rocket.rotation[1].abs() < 1e-4);
    }

    /// A rocket draws its UUID's two longs first, then the motion's two triangles
    /// (four doubles), then `nextInt(6)` and `nextInt(7)` for the lifetime, all from
    /// the one random. The expected values come from `java.util.Random` replayed
    /// outside the game: a rocket whose UUID draw were skipped would read 30, 22
    /// and 26 here.
    #[test]
    fn a_rockets_lifetime_follows_its_uuid_and_motion_draws() {
        use sunrise_core::random::LegacyRandom;
        for (seed, flight, lifetime) in [
            (1, 1, 26),
            (12_345, 1, 26),
            (987_654_321, 1, 25),
            (1, 3, 46),
        ] {
            let mut own = LegacyRandom::new(seed);
            sunrise_client::save::insecure_uuid(&mut own);
            assert_eq!(
                super::rocket_draws(&mut own, flight).1,
                lifetime,
                "seed {seed}"
            );
        }
    }

    #[test]
    fn use_on_passes_only_while_gliding() {
        assert!(super::use_on_passes(true));
        assert!(!super::use_on_passes(false));
    }

    /// `FireworkRocketItem.use` on the server, while fall-flying: a lead on a
    /// mob within the 32-block box drops as an item and one
    /// `minecraft:item.lead.break` follows the player on NEUTRAL, heard by the
    /// acting player too. A lead outside that box, or held for another player,
    /// stays. Standing still does neither.
    #[test]
    #[ignore = "daily"]
    fn a_gliding_firework_drops_leads_and_plays_lead_break() {
        use crate::game::{Options, Progress, Settings, load_resources};
        use sunrise_client::mobs::{Mob, leash::Holder};
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
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let res = loaded.resources;
        let mut game = crate::game::ServerGame::new(
            &res,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            None,
        )
        .unwrap();
        game.player = Some(
            sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::new(10.5, 64.0, -3.25))
                .unwrap(),
        );
        let feet = {
            let p = game.player.as_ref().expect("a player").position;
            [p.x, p.y, p.z]
        };
        let kind = game
            .res
            .mob_types()
            .get("minecraft:cow")
            .expect("cow")
            .clone();
        // A server action keeps the guest identity while its hub is swapped out.
        game.side.lan_acting = Some(11);
        game.side.lan_acting_entity = Some(37);
        let me = game.acting_uuid();
        let someone_else = [me[0].wrapping_add(1), me[1], me[2], me[3]];
        let mut leashed = |id, at: Vec3, owner: [i32; 4]| {
            let mut mob = Mob::new(id, kind.clone(), at, 0.0);
            mob.extra.leash = Some(Holder::Player);
            mob.extra.leash_player = Some(owner);
            game.mobs.push(mob);
        };
        let here = Vec3::new(feet[0] + 1.0, feet[1], feet[2]);
        leashed(1, here, me);
        leashed(2, Vec3::new(feet[0] + 40.0, feet[1], feet[2]), me);
        leashed(3, here, someone_else);
        game.inventory[game.selected] = Some(super::ItemStack::new("minecraft:firework_rocket", 1));
        let stack = game.inventory[game.selected].clone().unwrap();
        game.start_audio_capture();

        assert!(!game.boost_firework(&stack));
        assert_eq!(game.mobs[0].extra.leash, Some(Holder::Player));
        assert!(game.dropped.is_empty());
        assert!(game.side.pending_sounds.is_empty());

        game.player.as_mut().unwrap().fall_flying = true;
        assert!(game.boost_firework(&stack));
        assert!(
            game.mobs[0].extra.leash.is_none(),
            "the mob on the lead comes free"
        );
        assert_eq!(game.mobs[1].extra.leash, Some(Holder::Player));
        assert_eq!(game.mobs[2].extra.leash, Some(Holder::Player));
        let leads: Vec<_> = game
            .dropped
            .iter()
            .filter(|d| &*d.stack.item == "minecraft:lead")
            .collect();
        assert_eq!(leads.len(), 1);
        let heard = std::mem::take(&mut game.side.pending_sounds);
        let snaps: Vec<_> = heard
            .iter()
            .filter(|s| s.event == "minecraft:item.lead.break")
            .collect();
        assert_eq!(snaps.len(), 1);
        let snap = snaps[0];
        assert_eq!((snap.position, snap.volume, snap.pitch), (feet, 1.0, 1.0));
        assert_eq!(
            snap.entity,
            Some(37),
            "the snap must follow the acting guest"
        );
        assert_eq!(snap.source, 6, "SoundSource.NEUTRAL");
        assert_eq!(snap.except_entity, None, "vanilla excludes no player");
        let (packet, range) =
            sunrise_client::lan::containers::captured_sound_packet(snap, None, 42);
        let sunrise_protocol::packets::play::Clientbound::SoundEntity(packet) = packet else {
            panic!("a firework lead snap must use the entity-bound packet");
        };
        assert_eq!((packet.id, packet.source, range), (37, 6, 16.0));
        assert_eq!(packet.sound.1.unwrap().0, "minecraft:item.lead.break");
    }
}
#[cfg(test)]
#[path = "fireworks/hit_attempt_tests.rs"]
pub(in crate::game) mod hit_attempt_tests;

sunrise_client::impl_joined!(Rocket);
sunrise_mod_runtime::impl_attachment_lifetime!(Rocket);
