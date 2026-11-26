use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::{LegacyRandom, RandomSource},
};
use sunrise_level::random_source;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::game) enum State {
    Flying,
    Bobbing,
    Hooked(u32),
}

pub(in crate::game) struct Hook {
    pub(in crate::game) uuid: [i32; 4],
    pub(in crate::game) attachment_lifetime: sunrise_mod_runtime::attachments::AttachmentLifetime,
    pub(in crate::game) join: sunrise_sim::join_seq::JoinSeq,
    pub(in crate::game) caster: Option<u32>,
    pub(in crate::game) caster_generation: sunrise_sim::join_seq::JoinSeq,
    /// Its entity id, which a server's players track it under.
    pub(in crate::game) id: u32,
    pub(in crate::game) position: Vec3,
    pub(in crate::game) previous: Vec3,
    pub(in crate::game) velocity: Vec3,
    pub(in crate::game) state: State,
    /// `FishingHook.hookedIn`: set in `onHitEntity`, transitioned to `State::Hooked` on the next tick.
    pub(in crate::game) hooked_in: Option<u32>,
    /// `Projectile.leftOwner`: until true, caster's root-vehicle tree is excluded from hits.
    pub(in crate::game) left_owner: bool,
    /// `Entity.yRot` updated on `Projectile.updateRotation` and `ProjectileDeflection.REVERSE`.
    pub(in crate::game) yaw: f32,
    /// `Entity.xRot` updated on `Projectile.updateRotation`.
    pub(in crate::game) pitch: f32,
    pub(in crate::game) last_deflected_by: Option<u32>,
    pub(in crate::game) needs_sync: bool,
    pub(in crate::game) tick_count: i32,
    pub(in crate::game) life: i32,
    pub(in crate::game) ground: sunrise_world::ground_support::GroundState,
    pub(in crate::game) nibble: i32,
    pub(in crate::game) time_until_lured: i32,
    pub(in crate::game) time_until_hooked: i32,
    pub(in crate::game) fish_angle: f32,
    pub(in crate::game) out_of_water: i32,
    pub(in crate::game) open_water: bool,
    pub(in crate::game) luck: i32,
    pub(in crate::game) lure: i32,
    /// The fishing medium it last floated in outside water (`native_fishing_medium`): the
    /// table its catch rolls and the wait it draws. None in water.
    pub(in crate::game) medium: Option<HookMedium>,
    /// The position a server's update put it heading for.
    pub(in crate::game) lerp: HookLerp,
    pub(in crate::game) random: LegacyRandom,
}

sunrise_mod_runtime::impl_attachment_lifetime!(Hook);

impl sunrise_sim::join_seq::Joined for Hook {
    fn join_seq(&self) -> sunrise_sim::join_seq::JoinSeq {
        self.join
    }
    fn set_join_seq(&mut self, join: sunrise_sim::join_seq::JoinSeq) {
        self.join = join;
    }
}

/// What a hook keeps of the mod's fishing medium it floats in, on the server.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::game) struct HookMedium {
    /// The mod that answered.
    pub(in crate::game) mod_id: &'static str,
    /// The table the mod answered, as it answered it.
    pub(in crate::game) answered: Option<String>,
    /// The table a catch rolls: the answer's, once checked; None rolls vanilla's.
    pub(in crate::game) loot_table: Option<String>,
    pub(in crate::game) lure_ticks: Option<[i32; 2]>,
}

pub(in crate::game) const SIZE: f64 = 0.25;

pub(in crate::game) fn bounds(p: Vec3) -> Aabb {
    let h = SIZE / 2.0;
    Aabb::new(
        Vec3::new(p.x - h, p.y, p.z - h),
        Vec3::new(p.x + h, p.y + SIZE, p.z + h),
    )
}

/// The medium a mod answered, when the hook is in it: in the block of its
/// surface or below, as a hook is in water anywhere in a water block. A
/// surface that is not finite is no medium.
pub(in crate::game) fn inside_medium(position: Vec3, medium: &mod_api::FishingMedium) -> bool {
    medium.surface.is_finite() && position.y < medium.surface.floor() + 1.0
}

/// `FishingHook.syncronizedRandom`, reseeded every tick from the hook's UUID and the game
/// time so the server and its clients draw alike. Kept off the level's random,
/// whose draws must stay in vanilla's order.
pub(in crate::game) fn synchronized_random(uuid: [i32; 4], game_time: i64) -> LegacyRandom {
    let least = (i64::from(uuid[2]) << 32) | i64::from(uuid[3] as u32);
    LegacyRandom::new(least ^ game_time)
}

/// `FishingHook.onSyncedDataUpdated` on DATA_BITING: the downward plunge,
/// `-0.4F * Mth.nextFloat(syncronizedRandom, 0.6F, 1.0F)`, in float as vanilla.
pub(in crate::game) fn bite_impulse(r: &mut impl RandomSource) -> f64 {
    // Mth.nextFloat(random, min, max): `random.nextFloat() * (max - min) + min`.
    f64::from(-0.4f32 * (r.next_float() * (1.0f32 - 0.6f32) + 0.6f32))
}

/// `FishingHook.onHitBlock`: `getDeltaMovement().normalize().scale(hit.distanceTo(this))`,
/// where `HitResult.distanceTo` is the *squared* distance to the hook: the bobber
/// springs back a little off a wall instead of stopping dead.
pub(in crate::game) fn rebound(velocity: Vec3, hit: Vec3, at: Vec3) -> Vec3 {
    velocity.normalize().scale(hit.distance_squared(at))
}

/// The `onGround` of a server's update (`handleMoveEntity`), except that a
/// hook resting on the ground stays so while the update does not lift it: the
/// server's tracker can send the flag a tick behind the landing.
pub(in crate::game) fn resting(was: bool, at: Vec3, target: [f64; 3], sent: bool) -> bool {
    sent || (was && target[1] <= at.y + 1.0e-3)
}

/// `LinearInterpolationHandler`: where a server's update has a hook heading
/// and the steps left (`DEFAULT_INTERPOLATION_STEPS` 3).
#[derive(Default)]
pub(in crate::game) struct HookLerp {
    pub(in crate::game) target: Vec3,
    pub(in crate::game) steps: u32,
}

impl HookLerp {
    /// `interpolateTo`: a new target restarts the steps, the same one does not.
    pub(in crate::game) fn start(&mut self, target: [f64; 3]) {
        let target = Vec3::new(target[0], target[1], target[2]);
        if self.steps == 0 || self.target != target {
            self.target = target;
            self.steps = 3;
        }
    }
    /// `doInterpolate`: a step of the way there.
    pub(in crate::game) fn step(&mut self, at: Vec3) -> Vec3 {
        if self.steps == 0 {
            return at;
        }
        let n = f64::from(self.steps);
        self.steps -= 1;
        Vec3::new(
            at.x + (self.target.x - at.x) / n,
            at.y + (self.target.y - at.y) / n,
            at.z + (self.target.z - at.z) / n,
        )
    }
    /// `applyPredictedMovement`: what the hook moved by itself carries the
    /// target along while it is heading there.
    pub(in crate::game) fn moved(&mut self, by: Vec3) {
        if self.steps > 0 {
            self.target = Vec3::new(
                self.target.x + by.x,
                self.target.y + by.y,
                self.target.z + by.z,
            );
        }
    }
}

impl Hook {
    pub(in crate::game) fn bounds(&self) -> Aabb {
        bounds(self.position)
    }
    /// `FishingHook.tick` returns before `applyEffectsFromBlocks` while it
    /// holds an entity.
    pub(in crate::game) fn applies_block_effects(&self) -> bool {
        !matches!(self.state, State::Hooked(_))
    }
    pub(in crate::game) fn id(&self) -> u32 {
        self.id
    }
    /// `FishingHook.getHookedIn`, by entity id.
    pub(in crate::game) fn hooked_in(&self) -> Option<u32> {
        self.hooked_in.or(match self.state {
            State::Hooked(id) => Some(id),
            _ => None,
        })
    }
    /// `FishingHook.isOpenWaterFishing`.
    pub(in crate::game) fn open_water(&self) -> bool {
        self.open_water
    }
    /// A server's update of this client's own hook (`handleMoveEntity` also
    /// sets `onGround`).
    pub(in crate::game) fn move_to(&mut self, target: [f64; 3], on_ground: bool) {
        self.ground.on_ground = resting(self.ground.on_ground, self.position, target, on_ground);
        self.lerp.start(target);
    }
    /// This client's own hook as its server tracks it: drawn, never ticked here.
    pub(in crate::game) fn remote(id: u32, at: Vec3, uuid: [i32; 4]) -> Self {
        let mut random = random_source::entity();
        // Entity initializes a local UUID before recreateFromPacket replaces it.
        let _local_uuid = sunrise_sim::save::insecure_uuid(&mut random);
        let _synchronized = random_source::create();
        Self {
            uuid,
            attachment_lifetime: Default::default(),
            join: sunrise_sim::join_seq::JoinSeq::UNJOINED,
            caster: None,
            caster_generation: sunrise_sim::join_seq::JoinSeq::UNJOINED,
            id,
            position: at,
            previous: at,
            velocity: Vec3::ZERO,
            state: State::Flying,
            hooked_in: None,
            left_owner: false,
            yaw: 0.0,
            pitch: 0.0,
            last_deflected_by: None,
            needs_sync: false,
            tick_count: 0,
            life: 0,
            ground: Default::default(),
            nibble: 0,
            time_until_lured: 0,
            time_until_hooked: 0,
            fish_angle: 0.0,
            out_of_water: 0,
            open_water: true,
            luck: 0,
            lure: 0,
            medium: None,
            lerp: HookLerp::default(),
            random,
        }
    }
}
