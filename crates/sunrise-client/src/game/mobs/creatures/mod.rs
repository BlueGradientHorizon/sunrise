//! Mobs in the game loop: ticking with entity snapshots, natural and
//! chunk-generation spawning (`NaturalSpawner`), mob events (attacks,
//! breeding, conversions, drops), projectiles, player interactions
//! (feeding, shearing, milking, spawn eggs, throwing) and rendering.
#[path = "../../creatures/biome_facts.rs"]
pub(in crate::game) mod biome_facts;
#[cfg(test)]
pub(in crate::game) mod borrowed_panic_tests;
pub(in crate::game) mod borrowed_tick;
pub(in crate::game) mod breeding;
pub(in crate::game) mod combat_adaptations;
pub(in crate::game) mod conversion_identity;
pub(in crate::game) mod death_tail;
pub(in crate::game) mod hoglin_target;
#[cfg(test)]
pub(in crate::game) mod last_hurt_mob_tests;
pub(in crate::game) mod living_draw;
#[allow(unused_imports)]
use living_draw::LivingFrame;
#[path = "../../creatures/mob_conversion.rs"]
pub(in crate::game) mod mob_conversion;
#[cfg(test)]
pub(in crate::game) mod mob_dispatch_panic_tests;
pub(in crate::game) mod player_loot_entity;
pub(in crate::game) mod projectile_targets;
#[allow(unused_imports)]
pub(in crate::game) use projectile_targets::{
    NonMobHit, ProjectileHitInfo, TargetQuery, is_pickable_projectile,
};
#[allow(unused_imports)]
pub(in crate::game) use sunrise_gameplay::projectile_world::ProjWorld;
pub(in crate::game) mod pufferfish_player;
#[cfg(test)]
pub(in crate::game) mod render_fixture;
pub(in crate::game) mod residency;
#[cfg(test)]
pub(in crate::game) mod retained_limits_tests;
pub(in crate::game) mod section_steps;
#[cfg(test)]
#[path = "../../creatures/spawn_egg_mob_tests.rs"]
pub(in crate::game) mod spawn_egg_mob_tests;
#[cfg(test)]
#[path = "../../creatures/spectator_view_tests.rs"]
pub(in crate::game) mod spectator_view_tests;
pub(in crate::game) mod world_view;
#[allow(unused_imports)]
use super::api_values::{TryIntoApi, TryIntoEngine};
#[allow(unused_imports)]
use super::level_sounds::LevelSound;
#[allow(unused_imports)]
use super::piston_push::Lid;
#[allow(unused_imports)]
use super::{
    Dim, Game, InteractTarget,
    entity_culling::EntityCullDecision,
    native_mob_draw::PlayerRender,
    native_projectiles::{Impact, PendingImpact, ProjectileHit, projectile_impact_event},
    view_vector,
};
#[allow(unused_imports)]
use mod_api::{EquipmentSlot, LootDrops, SoundSource};
#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use sunrise_assets::item_model::DisplayContext;
#[allow(unused_imports)]
use sunrise_client::actions::Action;
#[allow(unused_imports)]
use sunrise_client::{
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
use sunrise_client_render::render_geometry::EntitySpan;
#[allow(unused_imports)]
use sunrise_core::{
    geometry::{Aabb, Direction, Vec3, trig::Mth},
    position::BlockPos,
    random::RandomSource,
};

/// `AbstractArrow.onHitEntity` when `hurtOrSimulate` returned false.
/// An enderman that teleported (`projectileReceivesSideEffectsOnHit` is
/// false) leaves the arrow in flight. Every other refusal, including
/// `Shulker.hurtServer` while `isClosed()`, is
/// `deflect(ProjectileDeflection.REVERSE, entity, owner, false, 0.2)`.
pub(in crate::game) fn arrow_miss(dodged: bool) -> EntityHit {
    if dodged {
        EntityHit::Dodged
    } else {
        EntityHit::Refused
    }
}
pub(in crate::game) type SpawnReason = sunrise_client::mobs::spawn_rules::Reason;

/// Whether vanilla calls `finalizeSpawn` for a mob a spawn site makes, and with
/// which reason. The reason is vanilla's `EntitySpawnReason` at that site.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::game) enum SpawnSite {
    /// Vanilla calls `finalizeSpawn` with this reason: `finalize_spawn` is told.
    Finalizing(SpawnReason),
    /// Vanilla creates the mob without `finalizeSpawn` (a breeding, a block's or
    /// an item's mob): the game finalizes it as before, and no mod is told.
    Plain,
    Raw,
}

/// What a spawn site does with a mob after `finalize_spawn` was heard.
#[derive(Debug, PartialEq)]
pub(in crate::game) enum Finalized {
    /// Finalized as vanilla does; the site goes on to the class's follow-ups.
    Done,
    /// A mod skipped finalization: the mob spawns uninitialised.
    Skipped,
    /// A mod stopped the spawn: the mob is not added.
    Cancelled,
}

pub(in crate::game) mod death_source;
pub(in crate::game) mod death_weapon;
pub(in crate::game) mod destroy_rules;
#[allow(unused_imports)]
pub(in crate::game) use death_source::death_hurt_source;

/// A dying entity whose items drop (`LivingDropsEvent`).
pub(in crate::game) struct DyingDrops<'a> {
    pub(in crate::game) entity: u32,
    pub(in crate::game) kind: &'a str,
    /// The UUID of a dying player.
    pub(in crate::game) player: Option<u128>,
    /// The death's source; none when no mod listens.
    pub(in crate::game) source: Option<HurtSource<'a>>,
    /// Whether a player hurt it within 100 ticks.
    pub(in crate::game) recently_hit: bool,
}

/// A sound a native mod queued, sent when the host is free.
pub(in crate::game) struct NativeSound {
    pub dimension: &'static str,
    pub sound: String,
    pub at: [f64; 3],
    pub source: i32,
    pub volume: f32,
    pub pitch: f32,
    pub seed: i64,
    /// The connection of a player who doesn't hear it, as
    /// `Level.playSound(except, ...)`.
    pub except: Option<u32>,
    /// The entity the sound follows (`ClientboundSoundEntityPacket`), sent
    /// from `at`, where it was when the mod queued the sound.
    pub entity: Option<i32>,
    /// True when queued from inside a `play_level_sound_at_*` listener so the tick flush validates
    /// and delivers the sound without re-dispatching level-sound listeners.
    pub skip_hooks: bool,
}

/// Shared state for the mob runs in one entity pass.
pub(in crate::game) struct MobDrawFrame<'a> {
    pub(in crate::game) living: LivingFrame<'a>,
    pub(in crate::game) precheck: bool,
}

#[derive(Default)]
pub(in crate::game) struct NormalActionFrame {
    pub(in crate::game) events: std::collections::VecDeque<MobEvent>,
    pub(in crate::game) broke: Vec<u32>,
    pub(in crate::game) actor: Option<u32>,
}
#[derive(Default)]
pub(in crate::game) struct NormalMobActions {
    pub(in crate::game) root: NormalActionFrame,
    pub(in crate::game) nested: Vec<NormalActionFrame>,
}
impl NormalMobActions {
    pub(in crate::game) fn new(events: Vec<MobEvent>) -> Self {
        Self {
            root: NormalActionFrame {
                events: events.into(),
                broke: Vec::new(),
                actor: None,
            },
            nested: Vec::new(),
        }
    }
}
pub(in crate::game) struct MobOutputBatch {
    pub(in crate::game) normal: NormalMobActions,
    pub(in crate::game) guests: Vec<(u32, u32, MobEvent)>,
}
#[derive(Default)]
pub(in crate::game) struct PendingMobBatch {
    pub(in crate::game) batches: std::collections::VecDeque<MobOutputBatch>,
    pub(in crate::game) pushes: sunrise_client::mobs::PendingPushes,
}

/// Remove the current action before dispatch; retain only the unentered tail on unwind.
pub(in crate::game) struct RemainingActions<'a, T> {
    pub(in crate::game) remaining: std::vec::IntoIter<T>,
    pub(in crate::game) target: &'a mut Vec<T>,
}
impl<'a, T> RemainingActions<'a, T> {
    pub(in crate::game) fn new(target: &'a mut Vec<T>) -> Self {
        Self {
            remaining: std::mem::take(target).into_iter(),
            target,
        }
    }
}
impl<T> Iterator for RemainingActions<'_, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.remaining.next()
    }
}
impl<T> Drop for RemainingActions<'_, T> {
    fn drop(&mut self) {
        let remaining = std::mem::replace(&mut self.remaining, Vec::new().into_iter());
        *self.target = remaining.collect();
    }
}

/// Mob-system state owned by the game.
#[derive(Default)]
pub(in crate::game) struct Creatures {
    pub projectiles: sunrise_client::join_seq::JoinList<Projectile, false>,
    /// The entities a family tells the mods ticked, kept between ticks
    /// (game/modding/native_ticks.rs).
    pub tick_buffer: Vec<(u32, super::native_ticks::TickKind)>,
    /// What a family's Pre and Post are told from, kept between ticks
    /// (game/modding/native_ticks.rs).
    pub tick_hooks: super::native_ticks::TickBuffers,
    /// The guests (by connection) whose `ServerPlayer.tick` a mod's Pre canceled this tick:
    /// every part of it skips them (game/network/lan_actions/tick.rs).
    pub canceled_players: Vec<u32>,
    /// Wind bursts from a guest's turn, waiting for it to end (game/interface/wind.rs).
    pub wind_bursts: Vec<super::wind::Burst>,
    /// The wind charge whose burst is being handled, by entity id: which
    /// entities have already ticked in vanilla's spawn-ordered tick list
    /// (game/interface/wind.rs).
    pub wind_source: Option<sunrise_client::join_seq::JoinSeq>,
    /// Wind pushes held until the entity's own tick has run, by entity id
    /// (game/interface/wind.rs `apply_late_wind_pushes`).
    pub late_wind_pushes: Vec<(u32, crate::game::Vec3)>,
    /// The players a fishing hook can catch, taken as the hooks tick, and
    /// the pulls on them waiting for the hub (game/items/fishing.rs).
    pub hook_targets: Vec<super::fishing::HookTarget>,
    pub hook_pulls: Vec<(u32, sunrise_core::geometry::Vec3)>,
    /// Mace smash knockback on the other players from a guest's turn, waiting
    /// for it to end (game/items/mace.rs).
    pub smash_pushes: Vec<super::mace::SmashPush>,
    /// Sweep attacks from a guest's turn, waiting for it to end
    /// (game/network/lan_pvp.rs).
    pub sweeps: Vec<super::lan_pvp::Sweep>,
    /// The snapshot's buffer between ticks, empty: ~400 bytes a mob, so
    /// thousands of mobs would allocate and free megabytes every tick.
    pub(in crate::game) seen_buf: Vec<Seen>,
    /// Per-tick scratch retained between creature ticks.
    pub(in crate::game) event_buf: Vec<MobEvent>,
    pub(in crate::game) pickable_projectile_targets:
        Vec<sunrise_client::projectiles::TargetCandidate>,
    pub(in crate::game) arrow_hit_scratch: sunrise_client::arrows::ArrowHitScratch,
    /// For each arrow an entity hit in this projectile tick, the projectiles
    /// that ticked after it and stayed, in order: a bounced one goes back
    /// before the first still listed (`finish_arrow_hit`).
    pub(in crate::game) bounced_followers: Vec<(u32, Vec<u32>)>,
    pub(in crate::game) pending: std::collections::HashMap<&'static str, PendingMobBatch>,
    /// The mobs whose own tick ran in the last `tick_creature_entities`:
    /// `ServerLevel.tick` ticks only the entities in entity-ticking range, so
    /// what the passes after the loop do for a mob's `aiStep`, goals or brain
    /// (pickups, item searches) is for these alone.
    pub ticked: sunrise_client::fast_hash::FxHashSet<u32>,
    pub deferred_hits: bool,
    /// The game time of the spear rows below, and for each attacker the latest contact
    /// whose `setLastHurtMob` was applied then. A player's row is delivered after the
    /// attacker's other rows, so an earlier contact never overwrites a later one.
    pub(in crate::game) stab_memory: (i64, sunrise_client::fast_hash::FxHashMap<u32, usize>),
    /// The wandering trader spawner's clock.
    pub trader: super::wandering_trader::Spawner,
    /// When each player last put a parrot on a shoulder (`timeEntitySatOnShoulder`).
    pub shoulder_time: std::collections::HashMap<[i32; 4], i64>,
    /// The cat spawner (`CatSpawner`): its countdown.
    pub cat: super::cat_spawner::CatSpawner,
    /// Each level's custom spawners, built when its first tick asks (game/mobs/custom_spawners.rs).
    pub custom_spawners: std::collections::HashMap<super::Dim, mod_api::CustomSpawners>,
    /// True while `ask_level_sound` is running `play_level_sound_at_*` listeners so sounds emitted
    /// from inside a listener are not re-dispatched to level-sound hooks.
    pub asking_level_sound: bool,
}
/// `Projectile.isPickable`: a projectile in `#minecraft:redirectable_projectile`
/// (a ghast's fireball and both wind charges) is pickable, so another
/// projectile's `ProjectileUtil.getEntityHitResult` can strike it
/// (`Projectile.onHit` then redirects it). `PICKABLE_PROJECTILE_IDS + j` is
/// `projectiles[j]` during `tick_projectiles`, below the dragon's reserved ids.
#[cfg(test)]
pub(in crate::game) const PICKABLE_PROJECTILE_IDS: u32 = super::dragon::SPECIAL - 0x10000;

/// The index of the projectile a `pickable_projectile_hitboxes` id stands for.
#[cfg(test)]
pub(in crate::game) fn pickable_projectile_hit(id: u32) -> Option<usize> {
    (PICKABLE_PROJECTILE_IDS..super::dragon::SPECIAL)
        .contains(&id)
        .then(|| (id - PICKABLE_PROJECTILE_IDS) as usize)
}

#[cfg(test)]
pub(in crate::game) fn pickable_projectile_hitboxes_into(
    projectiles: &[Projectile],
    alive: &[bool],
    skip: usize,
    out: &mut Vec<(Who, Aabb)>,
) {
    out.clear();
    out.extend(
        projectiles
            .iter()
            .enumerate()
            .filter(|(j, q)| *j != skip && alive[*j] && super::deflect::redirectable(q))
            .map(|(j, q)| (Who::Mob(PICKABLE_PROJECTILE_IDS + j as u32), q.aabb())),
    );
}

/// What the server's projectile loop keeps to hand its list back for a projectile told alone,
/// made only in a tick where a mod named a projectile type: each slot's projectile by id, the
/// ids of the list's rows, where those are once a hook changed the list, and the projectile
/// whose own Post is still to tell.
pub(in crate::game) struct ProjectileHandBack {
    pub(in crate::game) slots: Vec<u32>,
    pub(in crate::game) rows: Vec<u32>,
    pub(in crate::game) places: Option<sunrise_client::fast_hash::FxHashMap<u32, usize>>,
    pub(in crate::game) told: Option<(u32, &'static str)>,
}

impl ProjectileHandBack {
    pub(in crate::game) fn new(projectiles: &[Projectile]) -> Box<Self> {
        let slots: Vec<u32> = projectiles.iter().map(|p| p.id).collect();
        Box::new(Self {
            rows: slots.clone(),
            slots,
            places: None,
            told: None,
        })
    }

    /// Where slot `slot`'s projectile is in the list now, if it is still there.
    pub(in crate::game) fn place(&self, slot: usize) -> Option<usize> {
        super::native_ticks::slot_place(&self.places, &self.slots, slot)
    }

    /// After a hook told with the list handed back changed it (it wrote, added or removed a
    /// projectile, or the tick's spawns went first on the hand-back): the loop's state of each
    /// row follows it by id. A new row is kept, and an impact whose projectile a hook took out
    /// is dropped with it.
    pub(in crate::game) fn follow(
        &mut self,
        projectiles: &[Projectile],
        keep: &mut Vec<bool>,
        impacts: &mut [Impact],
    ) {
        let places = super::native_ticks::places_of(projectiles.iter().map(|p| p.id));
        for impact in impacts {
            impact.index = places.get(&impact.before.id).copied();
        }
        let kept: sunrise_client::fast_hash::FxHashMap<u32, bool> = self
            .rows
            .iter()
            .copied()
            .zip(keep.iter().copied())
            .collect();
        self.rows.clear();
        self.rows.extend(projectiles.iter().map(|p| p.id));
        keep.clear();
        keep.extend(
            self.rows
                .iter()
                .map(|id| kept.get(id).copied().unwrap_or(true)),
        );
        self.places = Some(places);
    }
}

/// Where an arrow an entity turned back goes back among `projectiles`: before
/// the first of `followers` (the projectiles that ticked after it and stayed,
/// in order) still listed, else last. Spawns restored ahead of the survivors
/// and settled impacts can leave the list out of join order, so its place is
/// taken from the tick, not from the join numbers.
pub(in crate::game) fn bounced_place(projectiles: &[Projectile], followers: &[u32]) -> usize {
    followers
        .iter()
        .find_map(|id| projectiles.iter().position(|q| q.id == *id))
        .unwrap_or(projectiles.len())
}

#[cfg(test)]
pub(in crate::game) fn pickable_projectile_hitboxes(
    projectiles: &[Projectile],
    alive: &[bool],
    skip: usize,
) -> Vec<(Who, Aabb)> {
    let mut out = Vec::new();
    pickable_projectile_hitboxes_into(projectiles, alive, skip, &mut out);
    out
}

/// `FlameFeatureRenderer`: fire quads stacked up a burning entity's box,
/// turned to the camera around Y, at full block light. `at` is the entity's
/// feet relative to the camera.
pub(in crate::game) fn flame_geometry(
    atlas: &sunrise_client::hud::HudAtlas,
    at: glam::Vec3,
    width: f32,
    height: f32,
    camera_yaw: f32,
    light: u8,
    out: &mut Vec<Vertex>,
) {
    let (Some(fire0), Some(fire1)) = (
        atlas.sprites.get("block/fire_0").copied(),
        atlas.sprites.get("block/fire_1").copied(),
    ) else {
        return;
    };
    let s = width * 1.4;
    let mut h = height / s;
    // Mth.rotationAroundAxis(Y, camera.orientation): the camera's turn about Y.
    let turn = glam::Mat3::from_rotation_y(std::f32::consts::PI - camera_yaw.to_radians());
    let lift = 0.3 - (h as i32) as f32 * 0.02;
    let packed = (u32::from(light >> 4) * 16) | ((15 * 16) << 8);
    let (mut r, mut yo, mut zo, mut ss) = (0.5f32, 0.0f32, 0.0f32, 0);
    while h > 0.0 {
        let tex = if ss % 2 == 0 { fire0 } else { fire1 };
        let (mut u0, v0, mut u1, v1) = (tex.u0, tex.v0, tex.u1, tex.v1);
        if ss / 2 % 2 == 0 {
            std::mem::swap(&mut u0, &mut u1);
        }
        for (x, y, uv) in [
            (-r, -yo, [u1, v1]),
            (r, -yo, [u0, v1]),
            (r, 1.4 - yo, [u0, v0]),
            (-r, 1.4 - yo, [u1, v0]),
        ] {
            let p = at + turn * glam::Vec3::new(x, y, zo + lift) * s;
            out.push(Vertex {
                normal: [0; 4],
                pos: p.to_array(),
                uv,
                // HDR output: the flames shine like fire (entity.wgsl fs_hdr_mark).
                layer: u32::from(sunrise_client::mesher::INCANDESCENT) << 16,
                color: 0xFFFF_FFFF,
                light: packed,

                glint_uv: [0.0, 0.0],
            });
        }
        h -= 0.45;
        yo -= 0.45;
        r *= 0.9;
        zo -= 0.03;
        ss += 1;
    }
}
pub(in crate::game) fn seen(m: &Mob, player: Option<[i32; 4]>, items: &Items) -> Seen {
    m.seen(player, items)
}
/// Whether a mob a packet named may take the click: `mob_hit` skips a mob in
/// its death animation, the one the player rides (it shares the player's root
/// vehicle) and the camera entity the pick starts from, so a packet that names
/// one of those does nothing rather than reaching the feed, shear, leash or
/// name paths the pick path refuses.
pub(in crate::game) fn named_mob_pickable(
    alive: bool,
    id: u32,
    riding: Option<u32>,
    camera: Option<u32>,
) -> bool {
    alive && Some(id) != riding && Some(id) != camera
}

/// What an entity drew on the GPU path: the skin frame's mark before its draws, and their
/// camera-relative bounds (`cull_skinned`).
pub(in crate::game) type Skinned = Option<((usize, usize, u32), Option<(glam::Vec3, glam::Vec3)>)>;

/// What a block's `onProjectileHit` reads of the projectile that struck it:
/// a thrown projectile ([`ProjectileHitter::of`]) or a fishing bobber
/// (`FishingHook.onHitBlock` calls `Projectile.onHitBlock` too).
#[derive(Clone, Copy, Debug)]
pub(in crate::game) struct ProjectileHitter {
    /// `Projectile.getOwner`.
    pub(in crate::game) owner: Option<Who>,
    /// On a server, the guest (by connection) who owns it.
    pub(in crate::game) guest: Option<u32>,
    /// An `AbstractArrow` (a target block resets after 20 ticks, not 8).
    pub(in crate::game) arrow: bool,
    /// Its entity type, for `target_hit`'s projectile predicate.
    pub(in crate::game) kind: &'static str,
    pub(in crate::game) position: Vec3,
}
impl ProjectileHitter {
    pub(in crate::game) fn of(p: &Projectile) -> Self {
        Self {
            owner: p.owner,
            guest: p.guest,
            arrow: p.arrow.is_some(),
            kind: p.entity_type(),
            position: p.position,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::game) enum OrbOrder {
    BeforeChild,
    AfterChild,
}

impl Game<'_> {
    /// A sulfur cube's attribute map (`LivingEntity.attributes`), from its
    /// type's defaults, before its first tick adopts the body's archetype
    /// modifiers. A map already there (commands, armor) is kept.
    pub(in crate::game) fn init_sulfur_attribute_maps(&mut self) {
        for m in &mut self.mobs {
            if m.species() == sunrise_client::mobs::Species::SulfurCube
                && m.extra.armor_attributes.is_none()
            {
                m.extra.armor_attributes = self.motion.attributes.create(&m.kind.id).map(Box::new);
            }
        }
    }
    /// Biome facts at a position for finalizeSpawn and variant rules.
    pub(in crate::game) fn biome_facts(&self, x: i32, y: i32, z: i32) -> BiomeFacts {
        biome_facts::biome_facts_in(
            &self.world,
            world_biomes!(self),
            self.res.mob_data(),
            x,
            y,
            z,
        )
    }
    /// Create a mob of `kind` at a position, finalized like a natural spawn.
    pub(in crate::game) fn spawn_mob(
        &mut self,
        kind: &str,
        at: [f64; 3],
        persistent: bool,
        site: SpawnSite,
    ) -> Option<u32> {
        let mob = self.create_mob_reason(kind, at, persistent, site, None)?;
        self.insert_spawned_mob(mob)
    }
    /// [`spawn_mob`](Self::spawn_mob) for a creation that snaps the mob to a
    /// yaw (`snapTo(pos, yaw, 0)`): the facing takes no draw from the level.
    pub(in crate::game) fn spawn_mob_facing(
        &mut self,
        kind: &str,
        at: [f64; 3],
        persistent: bool,
        site: SpawnSite,
        yaw: f32,
    ) -> Option<u32> {
        let mob = self.create_mob_reason(kind, at, persistent, site, Some(yaw))?;
        self.insert_spawned_mob(mob)
    }
    /// Create a mob of `kind` at a position without `finalizeSpawn`'s rolls,
    /// matching vanilla `EntityType.create(level, reason)`.
    pub(in crate::game) fn spawn_mob_raw(
        &mut self,
        kind: &str,
        at: [f64; 3],
        persistent: bool,
    ) -> Option<u32> {
        self.spawn_mob(kind, at, persistent, SpawnSite::Raw)
    }
    /// `SummonCommand` and `SpawnEggItem`: vanilla spawns the mob, but it
    /// never calls `Mob.setPersistenceRequired` for it, so the mob is not
    /// persistent: the despawn rule holds it like any other mob, and only
    /// its NBT can make it persistent.
    pub(in crate::game) fn spawn_mob_spawned(
        &mut self,
        kind: &str,
        at: [f64; 3],
        site: SpawnSite,
    ) -> Option<u32> {
        // Dragons have a separate entity implementation, but eggs use the same spawn path.
        if matches!(kind, "ender_dragon" | "minecraft:ender_dragon") {
            return self.spawn_dragon_spawned(at, site);
        }
        // The spawn is not a natural one (no jockeys, the patrol leader roll
        // stays), but the mob is not persistent: vanilla's SummonCommand
        // never calls setPersistenceRequired. The flag is false from the
        // start, before the mob joins the level, so a mod's own write in the
        // entity_join hook survives it - clearing it after the join would
        // wipe what the hook set (Mob.setPersistenceRequired from
        // entity_join_level).
        self.spawn_mob(kind, at, false, site)
    }
    /// [`spawn_mob_spawned`](Self::spawn_mob_spawned) for a mob that starts
    /// moving or turned: `velocity` (`Entity.setDeltaMovement`) and, when
    /// given, `[yaw, pitch]` in place of the level random's yaw, set before
    /// it joins. The kind resolves as `spawn_mob_spawned`'s (`cow` is
    /// `minecraft:cow`), and the ender dragon takes its own creation, facing
    /// the given yaw; it has no momentum or pitch of its own here, so a
    /// dragon given a velocity or a pitch is refused.
    pub(in crate::game) fn spawn_mob_spawned_moving(
        &mut self,
        kind: &str,
        at: [f64; 3],
        site: SpawnSite,
        velocity: Vec3,
        rotation: Option<[f32; 2]>,
    ) -> Option<u32> {
        if matches!(kind, "ender_dragon" | "minecraft:ender_dragon") {
            if velocity != Vec3::ZERO || rotation.is_some_and(|[_, pitch]| pitch != 0.0) {
                return None;
            }
            return self.spawn_dragon_facing(at, site, rotation.map(|[yaw, _]| yaw));
        }
        // Not persistent, as `spawn_mob_spawned`'s mob.
        let mut mob = self.create_mob_reason(kind, at, false, site, rotation.map(|r| r[0]))?;
        mob.velocity = velocity;
        if let Some([_, pitch]) = rotation {
            (mob.head_pitch, mob.previous_head_pitch) = (pitch, pitch);
        }
        self.insert_spawned_mob(mob)
    }

    pub(in crate::game) fn insert_spawned_mob(&mut self, mob: Mob) -> Option<u32> {
        let id = mob.id;
        self.mobs.push(mob);
        self.native_mobs_joined(self.mobs.len() - 1, false);
        if !self.mobs.iter().any(|m| m.id == id) {
            return None;
        }
        Some(id)
    }
    /// `Mob.burnUndead` for a mob in daylight with an item on its sun
    /// protection slot (the head, or the body for a zombie horse or
    /// nautilus): `hurtAndBreak(random.nextInt(2))` on a damageable item, and
    /// when it breaks the slot empties and `onEquippedItemBroken` broadcasts
    /// the break event.
    pub(in crate::game) fn wear_sun_protection(&mut self, id: u32) {
        use sunrise_core::equipment_slot::EquipmentSlot;
        let Some(m) = self.mobs.iter().find(|m| m.id == id) else {
            return;
        };
        let slot = m.sun_protection_slot();
        let body = slot == EquipmentSlot::Body;
        let Some(stack) = m.item_by_slot(slot) else {
            return;
        };
        if !sunrise_client::stations::is_damageable(self.res.items(), &stack) {
            return;
        }
        let at = [m.position.x, m.position.y, m.position.z];
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) else {
            return;
        };
        let amount = m.random.next_int_bound(2).unwrap_or(0) as u32;
        if amount == 0 {
            return;
        }
        let (updated, broken) = self.damage_living_stack(&stack, amount, id);
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) else {
            return;
        };
        if m.item_by_slot(slot).as_ref() != Some(&stack) {
            return;
        }
        m.set_equipment_stack(slot as usize, updated);
        if broken.is_some() {
            // LivingEntity.entityEventForEquipmentBreak: head 49, body 65.
            self.lan_entity_event(id as i32, if body { 65 } else { 49 }, at);
        }
    }
    /// `LivingEntity.updateFallFlying`'s wear, `ItemStack.hurtAndBreak(1, this,
    /// slot)` on the glider `MobEvent::GliderWear` picked: Unbreaking (from the
    /// level's random, `Enchantment.modifyItemFilteredCount`) may save it, and
    /// the rest goes on the stack where it is worn. Only a part's `break_now`
    /// breaks one (`canGlideUsing` picks none a point would break): emptied, with its event.
    pub(in crate::game) fn wear_mob_glider(
        &mut self,
        id: u32,
        slot: sunrise_core::equipment_slot::EquipmentSlot,
    ) {
        let Some(stack) = self
            .mobs
            .iter()
            .find(|m| m.id == id)
            .and_then(|m| sunrise_client::mobs::finalize::equipment_stack(m, slot))
        else {
            return;
        };
        if !sunrise_client::stations::is_damageable(self.res.items(), &stack) {
            return;
        }
        let (updated, broken) = self.damage_living_stack(&stack, 1, id);
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) else {
            return;
        };
        let at = [m.position.x, m.position.y, m.position.z];
        m.set_equipment_stack(slot as usize, updated);
        if broken.is_some() {
            let event = sunrise_client::mobs::item_break::equipment_break_event(slot);
            self.lan_entity_event(id as i32, event, at);
        }
    }
    /// `LivingEntity.hurtArmor` -> `LivingEntity.doHurtEquipment`, from the hit
    /// `MobEvent::ArmorHurt` reported. Only `Horse` overrides `hurtArmor` for a
    /// mob, and its override asks for the BODY slot alone, so the body piece is
    /// the only one this wears: it loses `max(1, damage / 4)` durability,
    /// Unbreaking applied, and at its maximum the slot empties and the break
    /// event goes out (`LivingEntity.entityEventForEquipmentBreak`: 65 for the
    /// body — 49 to 52 are the four armour slots, which only a player's own
    /// pieces reach). A fire hit leaves a fire-resistant piece alone
    /// (`ItemStack.canBeHurtBy`).
    pub(in crate::game) fn hurt_mob_armor(
        &mut self,
        id: u32,
        damage: f32,
        source: &HurtSource<'_>,
    ) {
        use sunrise_core::equipment_slot::EquipmentSlot;
        if damage <= 0.0 {
            return;
        }
        let Some(m) = self.mobs.iter().find(|m| m.id == id) else {
            return;
        };
        let at = [m.position.x, m.position.y, m.position.z];
        let amount = ((damage / 4.0) as u32).max(1);
        let name = source.damage_type;
        // A piece's maximum, or none when it may not lose durability at all.
        // `doHurtEquipment` starts from the item's `equippable` component
        // (`Equippable equippable = item.get(DataComponents.EQUIPPABLE)`), so a
        // damageable item with no such component — a sword put in a horse's body
        // slot by `/item replace` — never wears; `equip_damage_on_hurt` alone
        // cannot say that, since it defaults to true when the component is absent.
        fn wearable(me: &Game<'_>, stack: &ItemStack, name: &str) -> Option<u32> {
            let info = me.res.items().get(&stack.item)?;
            if info.equip_slot.is_none()
                || !sunrise_client::item_sync::damage_on_hurt(stack, info.equip_damage_on_hurt)
                || info.max_damage == 0
                || !sunrise_client::stations::is_damageable(me.res.items(), stack)
                || !me.stack_can_be_hurt_by(stack, name).unwrap_or(true)
            {
                return None;
            }
            Some(info.max_damage)
        }
        let Some(body) = self
            .mobs
            .iter()
            .find(|m| m.id == id)
            .and_then(|m| m.item_by_slot(EquipmentSlot::Body))
        else {
            return;
        };
        if wearable(self, &body, name).is_none() {
            return;
        }
        // ArmorHurtEvent for a mob's body armor.
        let mut amount = amount;
        if mod_api::__private::listened(mod_api::HookId::ArmorHurt)
            && let Ok(body_snapshot) = body.try_into_api()
        {
            let kind = self
                .mobs
                .iter()
                .find(|m| m.id == id)
                .map(super::native_hooks::mob_kind)
                .unwrap_or_default();
            let mut entries = vec![mod_api::ArmorHurt {
                slot: EquipmentSlot::Body,
                stack: body_snapshot,
                damage: super::durability_damage(damage),
                new_damage: super::durability_damage(damage),
            }];
            let mut cancel = false;
            self.native_armor_hurt(None, id, &kind, source, &mut entries, &mut cancel);
            if cancel {
                return;
            }
            amount = super::answered_wear(&entries, EquipmentSlot::Body, amount);
            if amount == 0 {
                return;
            }
            // A mod may have swapped the piece: it is not worn by this hit.
            if self
                .mobs
                .iter()
                .find(|m| m.id == id)
                .and_then(|m| m.item_by_slot(EquipmentSlot::Body))
                != Some(body.clone())
            {
                return;
            }
        }
        let (updated, broken) = self.damage_living_stack(&body, amount, id);
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) else {
            return;
        };
        if m.item_by_slot(EquipmentSlot::Body).as_ref() != Some(&body) {
            return;
        }
        m.set_equipment_stack(EquipmentSlot::Body as usize, updated);
        if broken.is_some() {
            // LivingEntity.onEquippedItemBroken through the level's broadcast.
            let event =
                sunrise_client::mobs::item_break::equipment_break_event(EquipmentSlot::Body);
            self.lan_entity_event(id as i32, event, at);
        }
    }
    /// Finalize a converted frog or witch before it joins the level.
    pub(in crate::game) fn finalize_converted(&mut self, mob: &mut Mob) {
        // A mod that stops a conversion's finalization leaves the mob as converted.
        self.finalize_with(mob, SpawnReason::Conversion, None);
        mob.persistent = true;
    }
    /// `Mob.convertTo(type, ConversionParams.single(...))`: a new mob of
    /// `kind`, made without spawn finalization unless its conversion asks, takes
    /// its place with what `conversion::single` carries over, and the old
    /// mob's riders move onto it. A zombie that comes out of it rolls
    /// `Zombie.handleAttributes(special, CONVERSION)` (the
    /// `postConversionStep` of its `ConversionTracker`). `pre_checked` skips
    /// `LivingConversionEvent.Pre`, which the lightning asks before it picks
    /// its outcome.
    pub(in crate::game) fn convert_to(
        &mut self,
        from: &Mob,
        kind: &str,
        params: sunrise_client::mobs::conversion::Params,
        pre_checked: bool,
        after: impl FnOnce(&mut Self, &mut Mob),
    ) -> Option<u32> {
        self.convert_to_moved(from, kind, params, pre_checked, None, after)
    }
    /// Default position for a summon command (3 blocks in front of the player).
    pub(in crate::game) fn default_summon_position(&self) -> Result<[f64; 3]> {
        let p = self.player.as_ref().ok_or("no player yet")?;
        let d = view_vector(self.yaw, 0.0);
        Ok([
            p.position.x + d[0] * 3.0,
            p.position.y,
            p.position.z + d[2] * 3.0,
        ])
    }
    /// The `summon` command: spawn a mob in front of the player or at a position.
    pub(in crate::game) fn summon_command_mob(
        &mut self,
        kind: &str,
        at: Option<[f64; 3]>,
        baby: bool,
    ) -> Result<u32> {
        let at = match at {
            Some(a) => a,
            None => self.default_summon_position()?,
        };
        let id = self
            .spawn_mob_spawned(kind, at, SpawnSite::Finalizing(SpawnReason::Command))
            .ok_or_else(|| format!("unknown or unsupported mob {kind}"))?;
        if baby && let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
            m.extra.age = if m.species() == Species::Sniffer {
                sunrise_client::mobs::sniffer::BABY_START_AGE
            } else {
                -24000
            };
        }
        Ok(id)
    }
    /// The `mobs` command: nearby mobs with position, health and running goals.
    pub(in crate::game) fn control_mobs(&self, radius: f64) -> serde_json::Value {
        let center = self.player.as_ref().map_or(Vec3::ZERO, |p| p.position);
        self.control_mobs_at(radius, center)
    }
    pub(in crate::game) fn control_mobs_at(&self, radius: f64, center: Vec3) -> serde_json::Value {
        let list: Vec<serde_json::Value> = self
            .mobs
            .iter()
            .filter(|m| m.distance_sqr(center) <= radius * radius)
            .map(|m| {
                let mut state = serde_json::json!({
                    "id": m.id,
                    "kind": m.kind.id,
                    "pos": [m.position.x, m.position.y, m.position.z],
                    "health": m.health,
                    "baby": m.is_baby(),
                    "target": m.ai.target.map(|t| format!("{t:?}")),
                    "goals": m.ai.goals.running_names(),
                    "held": m.extra.main_hand_id(),
                    "mount": m.extra.mount,
                    "rider": m.extra.rider,
                    // A player's input this tick, and whether it steers.
                    "ridden": m.rider.map(|r| r.steer),
                    "controlled": sunrise_client::mobs::equine::controlled(m),
                    "saddled": m.extra.saddled(),
                    "enchants": sunrise_client::mobs::gear::gear(m).map(|g| g.slots.iter().map(|s| s.iter().map(|(e, l)| format!("{e}:{l}")).collect::<Vec<_>>()).collect::<Vec<_>>()),
                    "ints": m.extra.ints,
                    "pois": [m.extra.home, m.extra.bed, m.extra.meeting],
                    "profession": m.extra.profession.index(),
                    "targets": m.ai.targets.running_names(),
                    "path": m.ai.nav.path.as_ref().map(|p| p.nodes.len() - p.next.min(p.nodes.len())),
                    "on_ground": m.ground.on_ground,
                    "variant": m.extra.variant,
                    "color": m.extra.color,
                    "sheared": m.extra.sheared,
                    "burning": m.fire_ticks > 0,
                    "size": m.extra.size,
                    "leash": m.extra.leash.map(|h| format!("{h:?}")),
                    "tame": m.extra.tame,
                    // TamableAnimal.isInSittingPose, what this side draws: a server's client
                    // has only that bit of DATA_FLAGS_ID, never the saved order to sit.
                    "sitting": m.extra.sitting_pose,
                    "temper": if sunrise_client::mobs::equine::is_equine(m.species()) {
                        sunrise_client::mobs::equine::temper(m)
                    } else { i32::from(m.extra.variant3) },
                    "chested": m.extra.chested,
                });
                // Set apart: one json! with every key goes past the default macro recursion
                // limit, and sunrise-client can't raise it (#8169).
                state["head"] = m.extra.head_id().into();
                state["body"] = m.extra.body_id().into();
                state["warden"] = serde_json::json!(sunrise_client::mobs::warden::describe(m));
                state["villager"] = serde_json::json!(sunrise_client::mobs::villager::ext(m).map(|e| serde_json::json!({
                    "food_level": e.food_level,
                    "inventory": e.inventory.iter().map(|s| format!("{}x{}", s.count, s.item)).collect::<Vec<_>>(),
                    "gossips": e.gossips.len(),
                    "last_slept": e.last_slept,
                    "raid": e.raid,
                })));
                state["raider"] = serde_json::json!(sunrise_client::mobs::raider::state(m).map(|s| serde_json::json!({
                    "raid": s.raid_id,
                    "wave": s.wave,
                    "leader": s.patrol_leader,
                    "patrolling": s.patrolling,
                    "patrol_target": s.patrol_target,
                    "vehicle": m.extra.mount,
                })));
                // Drawn seated (Entity.isPassenger).
                state["passenger"] = m.extra.passenger.into();
                state["guardian_beam"] = serde_json::json!(sunrise_client::mobs::guardian::beam(m, 0.0));
                state
            })
            .collect();
        serde_json::json!({ "count": self.mobs.len(), "near": list, "projectiles": self.creatures.projectiles.len() })
    }
    /// Highest motion-blocking (no leaves) block + 1 (Heightmap MOTION_BLOCKING_NO_LEAVES).
    pub(in crate::game) fn motion_top(&self, x: i32, z: i32) -> i32 {
        self.motion_top_in(&self.world, self.dim, x, z)
    }
    /// [`Self::motion_top`] in the loaded level `world` of `dim`.
    pub(in crate::game) fn motion_top_in(
        &self,
        world: &sunrise_client::world::World,
        dim: Dim,
        x: i32,
        z: i32,
    ) -> i32 {
        let top = if dim == Dim::Nether { 127 } else { 319 };
        for y in (world.bottom_y()..=top).rev() {
            let f = self.res.facts().get(world.block(x, y, z));
            if (f.full_collision || f.fluid != Fluid::None) && !f.name.ends_with("leaves") {
                return y + 1;
            }
        }
        world.bottom_y()
    }
    /// What the generator finalizes this level's structure mobs with.
    pub(in crate::game) fn share_spawn_conditions(&self) {
        if let Some(terrain) = self.end.locator() {
            terrain.set_spawn_conditions(self.difficulty_id() as u8, self.world.day_time);
        }
    }
    /// `Mob.finalizeSpawn` behind `FinalizeSpawnEvent`: the one fire point for
    /// the mobs the game spawns where vanilla calls `finalizeSpawn`. The class's
    /// follow-ups (raider flags and the like) are part of `finalizeSpawn`, so a
    /// site skips them on [`Finalized::Skipped`].
    pub(in crate::game) fn finalize_with(
        &mut self,
        mob: &mut Mob,
        reason: SpawnReason,
        spawner: Option<mod_api::SpawnSource>,
    ) -> Finalized {
        let mut group = None;
        let (outcome, difficulty) = self.finalize_spawn_event(mob, reason, spawner, &mut group);
        if outcome == Finalized::Done {
            self.finalize_grouped(mob, &difficulty, reason, group);
        }
        outcome
    }
    /// `Mob.finalizeSpawn`'s event, the half listeners own: they hear the
    /// spawn, cancel it, take it over, set its difficulty, edit the mob
    /// through its lend and change the pack's `group`, which finalization then
    /// reads. The decision and the difficulty the spawn finalizes with come
    /// back together.
    pub(in crate::game) fn finalize_spawn_event(
        &self,
        mob: &mut Mob,
        reason: SpawnReason,
        spawner: Option<mod_api::SpawnSource>,
        group: &mut Option<Group>,
    ) -> (Finalized, spawning::LocalDifficulty) {
        let at = [mob.position.x, mob.position.y, mob.position.z];
        let difficulty = self.local_difficulty(at);
        if !mod_api::__private::listened(mod_api::HookId::FinalizeSpawn) {
            return (Finalized::Done, difficulty);
        }
        let kind = mob.kind.id.clone();
        self.finalize_spawn_lent(&kind, at, reason, spawner, difficulty, Some(mob), group)
    }
    /// `Mob.getMaxSpawnClusterSize` as the entity parts of the finalized `mob`'s type answer it
    /// (`EntityPart::spawn_cluster_size`), asked with `mob` lent as it stands before it joins the
    /// level: the first part in dispatch order to answer wins, and none answering leaves vanilla's
    /// `vanilla` to the spawner. A type with no part costs one table lookup. Server-side.
    pub(in crate::game) fn spawn_cluster_size(
        &self,
        mob: &mut Mob,
        reason: SpawnReason,
        spawner: Option<mod_api::SpawnSource>,
        vanilla: u32,
    ) -> Option<u32> {
        if !mod_api::__private::has_entity_parts() {
            return None;
        }
        let parts = mod_api::__private::entity_parts();
        let kind = mob.kind.id.clone();
        let list = parts.entity_parts(&kind);
        if list.is_empty() {
            return None;
        }
        let at = [mob.position.x, mob.position.y, mob.position.z];
        self.lend_unspawned(&kind, at, Some(mob), |lent, _| {
            list.iter().find_map(|attached| {
                let _guard =
                    mod_api::content::crash::enter_native_mod(mod_api::content::crash::ModCall {
                        id: attached.mod_id(),
                        hook: "spawn_cluster_size",
                        part: Some(attached.type_name()),
                    });
                attached
                    .part()
                    .spawn_cluster_size(self, lent, reason, spawner, vanilla)
            })
        })
    }
    /// The shared finalization decision for a mob type the game keeps apart
    /// from mobs (the dragon), before the entity joins the level: its lend has
    /// a kind and a position and no mob.
    pub(in crate::game) fn finalize_spawn_at(
        &self,
        kind: &str,
        at: [f64; 3],
        reason: SpawnReason,
        spawner: Option<mod_api::SpawnSource>,
    ) -> (Finalized, spawning::LocalDifficulty) {
        let difficulty = self.local_difficulty(at);
        if !mod_api::__private::listened(mod_api::HookId::FinalizeSpawn) {
            return (Finalized::Done, difficulty);
        }
        self.finalize_spawn_lent(kind, at, reason, spawner, difficulty, None, &mut None)
    }
    /// Fires `finalize_spawn` with `mob` lent to it and the group data as the
    /// API's; a group the listener changed comes back as the game's.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::game) fn finalize_spawn_lent(
        &self,
        kind: &str,
        at: [f64; 3],
        reason: SpawnReason,
        spawner: Option<mod_api::SpawnSource>,
        difficulty: spawning::LocalDifficulty,
        mob: Option<&mut Mob>,
        group: &mut Option<Group>,
    ) -> (Finalized, spawning::LocalDifficulty) {
        let mut heard = mod_api::Difficulty::from(difficulty);
        let (mut spawn_cancelled, mut skip) = (false, false);
        let given = group
            .as_ref()
            .and_then(|g| super::native_unspawned_mob::native_spawn_group(g, self.lend_epoch()));
        let mut native_group = given.clone();
        self.lend_unspawned(kind, at, mob, |mob, edits| {
            self.native_finalize_spawn(mod_api::__private::FinalizeSpawnCall {
                dimension: self.dim.id(),
                kind,
                reason,
                pos: at,
                spawner,
                difficulty: &mut heard,
                spawn_cancelled: &mut spawn_cancelled,
                mob,
                edits,
                group: &mut native_group,
                cancel: &mut skip,
            });
        });
        if native_group != given {
            *group = native_group
                .as_ref()
                .and_then(super::native_unspawned_mob::spawn_group);
        }
        if spawn_cancelled {
            return (Finalized::Cancelled, difficulty);
        }
        if skip {
            return (Finalized::Skipped, difficulty);
        }
        (Finalized::Done, heard.into())
    }
    /// `ServerLevel.getCurrentDifficultyAt` in the level in place.
    pub(in crate::game) fn local_difficulty(&self, at: [f64; 3]) -> spawning::LocalDifficulty {
        local_difficulty_in(&self.world, at)
    }
    /// Generation's animals for the chunks generated since, once lit: chunk
    /// generation places them whether the level ticks or not.
    pub(in crate::game) fn spawn_fresh_chunks(&mut self) {
        if !self.world.chunks.has_fresh() {
            return;
        }
        let fresh: Vec<(i32, i32)> = self
            .world
            .chunks
            .fresh_keys_ordered()
            .into_iter()
            .filter(|k| self.world.chunks.get(k).is_some_and(|c| c.fresh && c.lit))
            .collect();
        for key in fresh {
            if let Some(c) = self.world.chunks.get_mut(&key) {
                Arc::make_mut(c).fresh = false;
            }
            self.world.chunks.remove_fresh(&key);
            self.spawn_generation_creatures(key.0, key.1);
        }
    }

    pub(in crate::game) fn apply_shulker_lid_push(
        &mut self,
        shulker: u32,
        area: Aabb,
        delta: [f64; 3],
    ) {
        let lid = Lid::Shulker(shulker);
        let movement = Vec3::new(delta[0], delta[1], delta[2]);
        if self.remote.is_some() {
            self.lid_push_local(lid, area, movement);
        } else {
            self.lid_push(lid, area, movement);
        }
    }

    pub(in crate::game) fn deliver_pending_mob_actions(
        &mut self,
        level_id: &'static str,
    ) -> Result<()> {
        let mut pending = self.creatures.pending.remove(level_id).unwrap_or_default();
        let delivery = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
            while let Some(batch) = pending.batches.front_mut() {
                self.creatures.deferred_hits = true;
                let result = self.handle_mob_events_pending(&mut batch.normal);
                self.creatures.deferred_hits = false;
                result?;
                self.lan_guest_mob_events(&mut batch.guests);
                self.creatures.event_buf = pending
                    .batches
                    .pop_front()
                    .unwrap()
                    .normal
                    .root
                    .events
                    .into();
            }
            if !pending.pushes.is_empty() {
                for mob in &mut self.mobs {
                    sunrise_client::mobs::apply_mob_pushes(mob, &mut pending.pushes);
                }
                // As in the producer's successful final sweep, removed targets
                // have no remaining owner to receive a push.
                pending.pushes = Default::default();
            }
            Ok(())
        }));
        self.creatures.pending.insert(level_id, pending);
        match delivery {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(in crate::game) fn finish_creature_entities(
        &mut self,
        laps: &mut sunrise_client::spans::Laps,
        primable: &[(u32, i32)],
    ) -> Result<()> {
        self.drain_entity_inside();
        laps.lap("creature_events");
        self.ordered_before(super::ordered_tick::Batch::Projectiles)?;
        self.tick_projectiles()?;
        self.drain_entity_inside();
        self.tick_fangs();
        laps.lap("creature_projectiles");
        self.tick_allays();
        self.tick_pandas();
        self.tick_foxes();
        self.tick_dolphins();
        self.tick_mob_looting();
        self.tick_breeze_effects();
        self.tick_sniffer_server();
        self.piglin_pickups();
        self.sulfur_cube_pickups();
        self.sulfur_cube_power_priming(primable);
        laps.lap("creature_systems");
        // Hurt and death sounds of damage dealt outside the mob ticks.
        let mut queued = Vec::new();
        for m in &mut self.mobs {
            queued.append(&mut m.outbox);
        }
        self.handle_mob_events(queued)?;
        // The enderman teleports those hurts asked for (`Enderman.hurtServer`).
        self.settle_hurt_teleports()?;
        laps.lap("creature_outbox");
        // And for the sizes the ticks changed (a pose, a baby grown, a scale).
        self.native_mob_sizes();
        self.native_entity_sizes();
        Ok(())
    }

    /// `LivingEntity.tickDeath` broadcasts the poof before `remove`, which
    /// also splits a large cube before it reaches `Entity.setRemoved`.
    pub(in crate::game) fn finish_mob_death_animation(
        &mut self,
        id: u32,
        at: Vec3,
        _width: f64,
        _height: f64,
        split: Option<Mob>,
    ) {
        let at = at.components();
        self.lan_entity_event(id as i32, 60, at);
        if let Some(mob) = split {
            self.split_dead_cube(mob);
        }
    }

    /// Run the owning-level steps of a suspended hit, then let its caller see the real result.
    pub(in crate::game) fn complete_mob_hurt(
        &mut self,
        index: usize,
        hurt: bool,
    ) -> Option<(usize, bool)> {
        self.complete_mob_hurt_with_weapon(index, hurt, None)
    }

    /// `AbstractCubeMob.remove`: split only as the dead body is removed.
    pub(in crate::game) fn split_dead_cube(&mut self, mut dying: Mob) {
        if matches!(
            dying.species(),
            Species::Slime | Species::MagmaCube | Species::SulfurCube
        ) && dying.extra.size > 1
        {
            self.split_cube_children(&mut dying);
        }
    }
    /// The killer's side of a mob's death: `ServerPlayer.awardKillScore`'s
    /// statistics and scores, and the `player_killed_entity` trigger.
    pub(in crate::game) fn credit_kill(&mut self, m: &Mob) {
        // Player.killedEntity: the killed statistic goes to the causing
        // entity, so a kill by a mob (a tame wolf credited to its owner)
        // leaves it out; ServerPlayer.awardKillScore counts the rest.
        if !killed_by_mob(m) {
            self.award_stat("killed", &m.kind.id, 1);
        }
        self.award_custom_stat("mob_kills", 1);
        self.score_player_killed(super::commands::Target::Mob(m.id));
        let view = self.mob_entity(m);
        let blow = self.last_blow(m);
        self.trigger_killed(&view, blow.as_ref());
    }
    /// The damage source a mob died to (`die`'s), its direct entity included when the mob
    /// recorded one ([`Mob::last_direct`]).
    pub(in crate::game) fn last_blow(
        &self,
        m: &Mob,
    ) -> Option<sunrise_client::advancements::DamageView> {
        let (d, _) = m.last_damage?;
        let mut view = self.damage_view(d);
        if let Some(kind) = m.last_direct {
            view.direct = Some(sunrise_client::advancements::EntityView {
                kind: kind.into(),
                position: d.from.unwrap_or_default(),
                ..Default::default()
            });
            view.is_direct = false;
        }
        Some(view)
    }
    /// A mob's damage source as advancement predicates see it: its type, the
    /// causing entity, and the projectile when one dealt it.
    pub(in crate::game) fn damage_view(
        &self,
        d: Damage,
    ) -> sunrise_client::advancements::DamageView {
        let source = match d.attacker {
            // Its callers are KilledTrigger's: the source as their killing_blow sees it.
            Some(Who::Player) => Some(self.player_entity_for_any(&[
                "minecraft:player_killed_entity",
                "minecraft:kill_mob_near_sculk_catalyst",
            ])),
            Some(Who::Mob(id)) => self
                .mobs
                .iter()
                .find(|o| o.id == id)
                .map(|o| self.mob_entity(o)),
            None => None,
        };
        let projectile = match d.kind {
            DamageKind::Arrow => Some("minecraft:arrow"),
            DamageKind::Fireball => Some("minecraft:fireball"),
            DamageKind::WitherSkull => Some("minecraft:wither_skull"),
            DamageKind::Fireworks => Some("minecraft:firework_rocket"),
            _ => None,
        };
        let direct = match projectile {
            Some(kind) => Some(sunrise_client::advancements::EntityView {
                kind: kind.into(),
                position: d.from.unwrap_or_default(),
                ..Default::default()
            }),
            None => source.clone(),
        };
        sunrise_client::advancements::DamageView {
            kind: source_damage_type(d).to_owned(),
            // DamageSource.isDirect: the causing entity is the direct one.
            is_direct: projectile.is_none(),
            direct,
            source,
            ..Default::default()
        }
    }
    /// `LivingEntity.dropExperience`: the experience a dying mob or player is worth once
    /// the mods have had their say; 0 when they cancel it.
    pub(in crate::game) fn living_experience_drop(
        &mut self,
        entity: u32,
        kind: &str,
        attacker: Option<u128>,
        original: i32,
    ) -> i32 {
        let (mut amount, mut cancel) = (original, false);
        self.native_living_experience_drop(
            entity,
            kind,
            attacker,
            original,
            &mut amount,
            &mut cancel,
        );
        if cancel { 0 } else { amount.max(0) }
    }

    /// Adds the items a dying entity dropped to the level, each at its own
    /// position. With mods, they first hear the list (`native_living_drops`):
    /// edited or canceled.
    pub(in crate::game) fn drop_death_items(
        &mut self,
        dying: &DyingDrops<'_>,
        stacks: Vec<(ItemStack, [f64; 3], i32)>,
        pickup_delay: u32,
        mut velocity: impl FnMut(&mut Self) -> Option<[f64; 3]>,
    ) {
        let Some(source) = &dying.source else {
            for (stack, position, age) in stacks {
                let mut random = super::spill::prepare_item_random(None);
                random.state.age = age;
                let motion = velocity(self).unwrap_or(random.velocity);
                self.add_item_entity_prepared(
                    stack,
                    position,
                    Some(motion),
                    pickup_delay,
                    Some(random),
                );
            }
            return;
        };
        // Vanilla builds each item entity (and draws its motion) when it
        // collects the drop, before the event sees the list.
        let mut drops: Vec<super::spill::BlockItemDrop> = stacks
            .into_iter()
            .map(|(stack, position, age)| {
                let random = super::spill::prepare_item_random(None);
                super::spill::BlockItemDrop {
                    random: Some(random),
                    stack,
                    position,
                    velocity: velocity(self).unwrap_or(random.velocity),
                    pickup_delay,
                    state: super::spill::DropState {
                        age,
                        ..Default::default()
                    },
                }
            })
            .filter(|drop| drop.stack.count > 0 && &*drop.stack.item != "minecraft:air")
            .collect();
        let mut cancel = false;
        if mod_api::__private::listened(mod_api::HookId::LivingDrops)
            && let Ok(mut snapshots) = drops.try_into_api()
        {
            self.native_living_drops(mod_api::__private::LivingDropsCall {
                entity: dying.entity,
                kind: dying.kind,
                player: dying.player,
                source,
                recently_hit: dying.recently_hit,
                drops: &mut snapshots,
                cancel: &mut cancel,
            });
            if let Ok(mut next) = snapshots.try_into_engine(self.items()) {
                for (drop, original) in next.iter_mut().zip(&drops) {
                    drop.random = original.random;
                }
                drops = next;
            } else {
                cancel = false;
            }
        }
        if cancel {
            return;
        }
        for drop in drops {
            if drop.stack.count > 0 && &*drop.stack.item != "minecraft:air" {
                self.add_item_drop(drop);
            }
        }
    }
    /// Apply reactions that need the game in `hurtServer`, without waiting for
    /// a mob tick: a Creaking's heart response or a Warden's anger and targeting.
    /// Wardens can react even when the base hurt call returned false.
    pub(in crate::game) fn settle_mob_hurt(&mut self, id: u32) {
        use sunrise_client::mobs::{creaking, warden};

        let Some(index) = self.mobs.iter().position(|m| {
            m.id == id
                && (creaking::has_pending_hurt(m)
                    || m.extra
                        .warden
                        .as_ref()
                        .is_some_and(|st| st.has_pending_hurt()))
        }) else {
            return;
        };
        if self.mobs[index].species() == Species::Creaking {
            self.finish_creaking_hurt(index);
            return;
        }
        let mut events = Vec::new();
        let uuid = self.level.player_uuid;
        let others: Vec<_> = self
            .mobs
            .iter()
            .map(|m| seen(m, uuid, self.items()))
            .collect();
        let mut mob = self.mobs.swap_remove(index);
        {
            let world = self.mob_world_base(&others);
            warden::handle_hurt(&mut mob, &world, &mut events);
        }
        self.mobs.push(mob);
        let last = self.mobs.len() - 1;
        if index < last {
            self.mobs.swap(index, last);
        }
        if !events.is_empty() {
            let _ = self.handle_mob_events(events);
        }
    }

    pub(in crate::game) fn settle_creaking_hurt(&mut self, id: u32) {
        if let Some(index) = self.mob_index.find(&self.mobs, id)
            && sunrise_client::mobs::creaking::has_pending_hurt(&self.mobs[index])
        {
            self.finish_creaking_hurt(index);
        }
    }

    pub(in crate::game) fn finish_creaking_hurt(&mut self, index: usize) {
        let mob = &mut self.mobs[index];
        let (id, join) = (mob.id, mob.join);
        let mut events = Vec::new();
        let reaction = sunrise_client::mobs::creaking::take_hurt(mob, &mut events);
        let _ = self.handle_mob_events(events);
        if let Some((home, by_player)) = reaction
            && self
                .mob_object(id)
                .is_some_and(|mob| mob.join == join && !mob.removed)
        {
            self.creaking_hurt(home, id, by_player);
        }
    }

    /// `Entity.igniteForSeconds`: `seconds * 20` ticks of fire, scaled by the
    /// target's burning-time attribute.
    pub(in crate::game) fn ignite_entity(&mut self, target: Who, seconds: i32) -> Result<()> {
        match target {
            Who::Player => {
                if !self.creative
                    && let Some(p) = self.player.as_mut()
                {
                    p.ignite_for_ticks(seconds * 20)?;
                }
            }
            Who::Mob(id) => {
                if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                    m.ignite_for_ticks(seconds * 20);
                }
            }
        }
        Ok(())
    }
    pub(in crate::game) fn handle_mob_events(&mut self, mut events: Vec<MobEvent>) -> Result<()> {
        self.handle_mob_events_reusable(&mut events)
    }

    pub(in crate::game) fn handle_mob_events_reusable(
        &mut self,
        events: &mut Vec<MobEvent>,
    ) -> Result<()> {
        let mut actions = NormalMobActions::new(std::mem::take(events));
        let result = self.handle_mob_events_pending(&mut actions);
        *events = actions.root.events.into();
        result
    }

    /// `TurtleEggBlock.decreaseEggs` after an entity's step or landing won its
    /// roll (`destroyEgg`): the break sound and the remaining eggs (a stack of
    /// one is destroyed after it).
    pub(in crate::game) fn crack_turtle_egg(&mut self, pos: [i32; 3]) -> Result<()> {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let eggs = self
            .pack
            .assets
            .registry
            .state(state)
            .filter(|s| s.name == "minecraft:turtle_egg")
            .and_then(|s| s.properties.get("eggs"))
            .and_then(|e| e.parse::<i32>().ok());
        if let Some(eggs) = eggs {
            let at = BlockPos::new(pos[0], pos[1], pos[2]);
            self.with_level(|b, level, ticks| {
                b.turtle_egg_destroyed_by_player(level, ticks, at, state);
                if eggs <= 1 {
                    b.destroy_block(level, ticks, at, false);
                }
            });
            self.process_level_events()?;
        }
        Ok(())
    }
    /// `TurtleEggBlock.randomTick` at the last hatch stage: one baby turtle
    /// per egg, a day from grown, with the egg as its home (the sound, the
    /// crumbs and the block's removal came from the level).
    pub(in crate::game) fn hatch_turtles(
        &mut self,
        pos: [i32; 3],
        state: sunrise_world::registry::BlockStateId,
    ) {
        let eggs = self
            .pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get("eggs"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(1);
        for i in 0..eggs {
            let at = [
                f64::from(pos[0]) + 0.3 + f64::from(i) * 0.2,
                f64::from(pos[1]),
                f64::from(pos[2]) + 0.3,
            ];
            if let Some(id) = self.spawn_mob_raw("minecraft:turtle", at, true)
                && let Some(m) = self.mobs.iter_mut().find(|m| m.id == id)
            {
                m.extra.age = -24000;
                m.extra.home = Some(pos);
                m.yaw = 0.0;
                m.body_yaw = 0.0;
                m.head_yaw = 0.0;
            }
        }
    }
    pub(in crate::game) fn apply_setup(&mut self, id: u32, setup: &SpawnSetup) {
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) else {
            return;
        };
        if setup.baby {
            m.extra.age = -24000;
        }
        if setup.nausea {
            m.add_effect("minecraft:nausea", 200, 0);
        }
        if setup.size > 0 {
            m.extra.size = setup.size;
        }
        m.yaw = setup.yaw;
        m.body_yaw = setup.yaw;
        m.head_yaw = setup.yaw;
        m.velocity = Vec3::new(setup.velocity[0], setup.velocity[1], setup.velocity[2]);
        if let Some(h) = setup.health {
            m.health = h;
        }
        if let Some(v) = setup.variant {
            m.extra.variant = v;
        }
        if let Some(owner) = setup.owner_mob {
            m.extra.counter2 = owner as i32;
        }
        if let Some(home) = setup.home {
            m.extra.home = Some(home);
        }
        if let Some(life) = setup.life {
            m.extra.egg_time = life;
        }
    }
    /// `Animal.finalizeSpawnChildFromBreeding`'s single experience orb, drawn
    /// from the breeding parent after the rule check.
    pub(in crate::game) fn award_breeding_experience(&mut self, parent: u32, at: [f64; 3]) {
        let mob_drops = self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::MOB_DROPS);
        if let Some(amount) = self
            .mobs
            .iter_mut()
            .find(|m| m.id == parent)
            .and_then(|m| sunrise_client::mobs::breeding_experience(m, mob_drops))
        {
            self.add_orb(at, amount);
        }
    }
    pub(in crate::game) fn breeding_orb(
        &mut self,
        parent: u32,
        at: [f64; 3],
        experience: sunrise_client::mobs::BreedExperience,
    ) {
        match experience {
            sunrise_client::mobs::BreedExperience::Deferred => {
                self.award_breeding_experience(parent, at)
            }
            sunrise_client::mobs::BreedExperience::Drawn(Some(amount)) => self.add_orb(at, amount),
            sunrise_client::mobs::BreedExperience::Drawn(None) => {}
        }
    }
    /// `ServerPlayer.tellNeutralMobsThatIDied`: the mobs within 32 blocks
    /// across and 10 up and down get `NeutralMob.playerDied`, when the
    /// `forgive_dead_players` rule is on. A mob's `Who::Player` is the guest
    /// it is bound to in `bindings` (game/network/lan_mobs.rs), else the host's
    /// player, so only the mobs whose binding is `conn` (`None` for the
    /// host's player) hear it.
    pub(in crate::game) fn tell_neutral_mobs_player_died(
        &mut self,
        feet: [f64; 3],
        conn: Option<u32>,
    ) {
        // A client of someone else's world has the server's mobs only to show.
        if self.remote.is_some()
            || !self
                .ui
                .rules
                .bool_at(sunrise_client::gamerules::vanilla::FORGIVE_DEAD_PLAYERS)
        {
            return;
        }
        let block = feet.map(f64::floor);
        let area = Aabb::new(
            Vec3::new(block[0], block[1], block[2]),
            Vec3::new(block[0] + 1.0, block[1] + 1.0, block[2] + 1.0),
        )
        .inflate(Vec3::new(32.0, 10.0, 32.0));
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let ids: Vec<u32> = self
            .mobs
            .iter()
            .filter(|m| {
                host.mob_players.get(&m.id).copied() == conn
                    && m.bounds().intersects(area)
                    && sunrise_client::mobs::ai::forgives_dead_player(m)
            })
            .map(|m| m.id)
            .collect();
        for id in ids {
            let Some(epoch) = self.native_lifecycle.epoch() else {
                if let Some(mob) = self.mobs.iter_mut().find(|m| m.id == id) {
                    sunrise_client::mobs::ai::forgive_dead_player(mob);
                }
                continue;
            };
            let mob_ref = mod_api::MobRef::new(mod_api::EntityRef::new(epoch, id));
            let change = self
                .native_mob_target_event(mob_ref, None, false)
                .and_then(|target| self.native_mob_bind_target(id, target))
                .map_or(
                    sunrise_client::mobs::TargetChange::Keep,
                    sunrise_client::mobs::TargetChange::Set,
                );
            if let Some(mob) = self.mobs.iter_mut().find(|m| m.id == id) {
                sunrise_client::mobs::ai::forgive_dead_player_with_change(mob, change);
            }
        }
    }

    pub(in crate::game) fn finish_live_breeding(
        &mut self,
        p: &Mob,
        partner: u32,
        child: Mob,
        experience: sunrise_client::mobs::BreedExperience,
    ) -> Option<u32> {
        let parent = p.id;
        let order = if sunrise_client::mobs::orb_after_child(p.species()) {
            OrbOrder::AfterChild
        } else {
            OrbOrder::BeforeChild
        };
        self.finalize_breeding(p, partner, Some(&child), order);
        let at = [p.position.x, p.position.y, p.position.z];
        match order {
            OrbOrder::AfterChild => {
                let joined = self.insert_spawned_mob(child);
                self.mob_event_particles(parent, 18);
                self.breeding_orb(parent, at, experience);
                joined
            }
            OrbOrder::BeforeChild => {
                self.breeding_orb(parent, at, experience);
                self.insert_spawned_mob(child)
            }
        }
    }
    /// A status effect from a mob, arrow or splash potion (`addEffect`).
    pub(in crate::game) fn give_player_effect(
        &mut self,
        effect: &'static str,
        ticks: i32,
        amplifier: i32,
    ) {
        self.give_mob_effect(effect, ticks, amplifier, false);
    }
    /// A mob's effect on the player. Creative and spectator players shrug off
    /// the damaging effects here, but the nautilus's ambient Breath is plain
    /// `addEffect` and lands in every game mode.
    pub(in crate::game) fn give_mob_effect(
        &mut self,
        effect: &'static str,
        ticks: i32,
        amplifier: i32,
        ambient: bool,
    ) {
        let immune = self.creative || self.spectator();
        if let Some(instance) = mob_effect_instance(effect, ticks, amplifier, ambient, immune) {
            self.apply_player_effect(instance);
        }
    }
    /// `WitherRoseBlock.entityInside` for the player: on a server level that
    /// isn't peaceful, `getBeeInteractionEffect` (Wither, 40 ticks) unless the
    /// player is invulnerable to wither damage (creative, spectator). A
    /// server's player gets it from the server.
    pub(in crate::game) fn wither_rose_contact(&mut self) {
        if self.remote.is_some() || self.ui.difficulty == 0 {
            return;
        }
        self.give_player_effect("minecraft:wither", 40, 0);
    }

    /// Hands the tick's lifted projectiles back for a hook told about one of them, so its handles
    /// read the live rows, and takes them again. Whether they came back as they went: the same
    /// rows in the same order, unwritten (the list's write stamp), with no spawn of the tick put
    /// before them.
    pub(in crate::game) fn projectiles_told(
        &mut self,
        projectiles: sunrise_client::join_seq::JoinList<Projectile, false>,
        tell: impl FnOnce(&mut Self),
    ) -> (sunrise_client::join_seq::JoinList<Projectile, false>, bool) {
        let no_spawns = self.creatures.projectiles.is_empty();
        self.creatures.projectiles.restore(
            projectiles,
            sunrise_client::join_seq::RestoreOrder::SpawnsFirst,
        );
        let restored = self.creatures.projectiles.stamp();
        tell(self);
        let same = no_spawns && self.creatures.projectiles.stamp() == restored;
        (self.creatures.projectiles.lift(), same)
    }

    /// Single-projectile live vanilla tick loop (when no impact/alone hook requires batching).
    pub(in crate::game) fn tick_projectiles_live(
        &mut self,
        through: u32,
        ticking: impl Fn(i32, i32) -> bool,
        vanish: bool,
        round: super::native_ticks::TickRound,
    ) -> Result<()> {
        let cohort: Vec<JoinSeq> = self
            .creatures
            .projectiles
            .iter()
            .filter(|p| p.id <= through)
            .map(|p| p.join)
            .collect();
        for join in cohort {
            let Some(pos) = self.creatures.projectiles.locate(join) else {
                continue;
            };
            if round.canceled(self.creatures.projectiles[pos].id) {
                continue;
            }
            let at = self.creatures.projectiles[pos].position;
            if !ticking(at.x.floor() as i32, at.z.floor() as i32) {
                self.creatures.projectiles.update(pos, |p| p.previous = at);
                continue;
            }
            let _save_lease = self
                .creatures
                .projectiles
                .visibility_lease()
                .ok_or_else(sunrise_client::join_seq::visibility::PassLease::enter);
            let Some(mut p) = self.creatures.projectiles.extract(join) else {
                continue;
            };
            let pid = p.id;
            if vanish
                && matches!(p.kind, ProjectileKind::EnderPearl)
                && p.owner == Some(Who::Player)
                && match p.guest {
                    Some(c) => self
                        .lan
                        .as_ref()
                        .and_then(|h| h.hub.guest(c))
                        .is_some_and(|g| g.dead),
                    None => self.player_view().is_some_and(|pv| !pv.alive),
                }
            {
                self.creatures.projectiles.retire_extracted(pid, join);
                continue;
            }
            let section_from = p.position;
            let mob_shot = !matches!(p.owner, Some(Who::Player));
            let pvp = !mob_shot && super::lan_pvp::meets_players(&p.kind);
            let shooter = p.guest.filter(|_| !mob_shot).and_then(|c| {
                self.lan
                    .as_ref()
                    .and_then(|h| h.hub.guest(c))
                    .map(|g| Who::Mob(g.entity_id as u32))
            });
            if shooter.is_some() {
                p.owner = shooter;
            }
            let use_guests = mob_shot || pvp;
            let query = TargetQuery::for_projectile(&p, use_guests);
            let mut scratch = std::mem::take(&mut self.creatures.arrow_hit_scratch);
            let (keep, hit, snow_contacts, bubble_tops) = {
                let live = self.live_proj_world(query, &[]);
                let (keep, hit) = if p.arrow.is_some() {
                    let rain = self.arrow_rain();
                    let scene = self.arrow_scene(&rain);
                    sunrise_client::arrows::tick_to_block_contact_live(
                        &mut p,
                        &scene,
                        &live,
                        false,
                        &mut scratch,
                    )
                } else {
                    p.tick_before_hit(&live)
                };
                let mut snow = Vec::new();
                if (p.is_thrown() && !matches!(p.kind, ProjectileKind::LlamaSpit))
                    || p.is_arrow() && p.arrow.as_ref().is_none_or(|a| !a.no_physics)
                {
                    for block in sunrise_client::powder_snow::contacts(
                        p.previous,
                        p.position,
                        p.bounds(),
                        false,
                        |at| live.w.block_name(at.x, at.y, at.z) == "minecraft:powder_snow",
                    ) {
                        snow.push((block, p.position, p.velocity));
                    }
                }
                let mut bubbles = Vec::new();
                if keep && hit == Hit::None && matches!(p.kind, ProjectileKind::EnderPearl) {
                    let (at, h) = (p.position, 0.125);
                    sunrise_client::motion::bubble_column_push_over(
                        live.w.world,
                        live.w.registry,
                        live.w.facts,
                        live.w.collision,
                        Aabb::new(
                            Vec3::new(at.x - h, at.y, at.z - h),
                            Vec3::new(at.x + h, at.y + 2.0 * h, at.z + h),
                        ),
                        &mut p.velocity,
                        |bpos| bubbles.push(bpos),
                    );
                }
                (keep, hit, snow, bubbles)
            };
            self.creatures.arrow_hit_scratch = scratch;
            if shooter.is_some() {
                p.owner = Some(Who::Player);
            }
            for (block, position, movement) in snow_contacts {
                self.powder_snow_particle(block, position, movement);
            }
            if let Some(at) = p.take_shoot_event() {
                self.projectile_shoot_event(p.owner, p.guest, [at.x, at.y, at.z]);
            }
            for splash in p.water_splashes.drain(..) {
                self.apply_item_splash(splash);
            }
            if let Some((hurt_at, is_lava)) = p.take_bullet_hurt() {
                let at = hurt_at.components();
                let silent = self.entity_silent(pid);
                let ctx = sunrise_client::sculk::Context {
                    owner: p.owner,
                    owner_uuid: p.owner_uuid,
                    ..self.entity_event_context(pid, None)
                };
                self.shulker_bullet_hurt(at, silent);
                self.creatures.projectiles.retire_extracted(pid, join);
                drop(_save_lease);
                self.post_game_event("minecraft:entity_damage", at, ctx);
                if is_lava && !silent {
                    self.lava_hurt_sound(at);
                }
                continue;
            }
            // Clients generate projectile smoke trails from synced entity state.
            for bpos in bubble_tops {
                self.bubble_column_particles(bpos);
            }
            let mut shot_after_hit = false;
            let mut pending_arrow_events = Vec::new();
            if let Some(a) = p.arrow.as_mut() {
                for event in a.events.drain(..) {
                    if event == sunrise_client::arrows::ArrowEvent::ShootEvent {
                        if matches!(hit, Hit::Block { .. }) {
                            shot_after_hit = true;
                        } else {
                            self.projectile_shoot_event(p.owner, p.guest, p.position.components());
                        }
                    } else {
                        pending_arrow_events.push(event);
                    }
                }
            }
            for event in pending_arrow_events {
                self.arrow_event(p.position, event);
            }
            let wind = matches!(
                p.kind,
                ProjectileKind::WindCharge | ProjectileKind::BreezeWindCharge
            );
            if wind
                && keep
                && let Some(ceiling) = self.wind_ceiling_contact(&p)
            {
                self.creatures.projectiles.retire_extracted(pid, join);
                drop(_save_lease);
                self.apply_projectile_hit(&p, ceiling)?;
                continue;
            }
            if matches!(hit, Hit::Entity { .. }) && (!keep || p.arrow.is_some()) {
                let followers: Vec<u32> = self
                    .creatures
                    .projectiles
                    .iter()
                    .skip(pos.min(self.creatures.projectiles.len()))
                    .map(|q| q.id)
                    .collect();
                self.creatures.bounced_followers.push((pid, followers));
            }
            if keep {
                self.sweep_projectile_inside(&mut p);
                self.lava_mark_hurt(&mut p);
                let to = p.position;
                let place = pos.min(self.creatures.projectiles.len());
                if hit == Hit::None {
                    self.creatures.projectiles.put_back(place, p);
                    drop(_save_lease);
                    self.entity_section_step(pid, section_from.components(), to.components());
                    continue;
                }
                let hit_snapshot = p.clone();
                self.creatures.projectiles.put_back(place, p);
                drop(_save_lease);
                self.entity_section_step(pid, section_from.components(), to.components());
                let result = self.apply_projectile_hit(&hit_snapshot, hit);
                if result.is_ok() && shot_after_hit {
                    self.projectile_shoot_event(
                        hit_snapshot.owner,
                        hit_snapshot.guest,
                        hit_snapshot.position.components(),
                    );
                }
                self.creatures.bounced_followers.clear();
                result?;
                continue;
            }
            if hit == Hit::None {
                self.creatures.projectiles.retire_extracted(pid, join);
                continue;
            }
            let result = self.apply_projectile_hit(&p, hit);
            if let Some(re_idx) = self.creatures.projectiles.locate(join) {
                self.creatures.projectiles.update(re_idx, |live| {
                    if !live.attachment_lifetime.is_bound() {
                        std::mem::swap(&mut live.attachment_lifetime, &mut p.attachment_lifetime);
                    }
                });
            } else {
                self.creatures.projectiles.retire_extracted(pid, join);
            }
            drop(_save_lease);
            if result.is_ok() && shot_after_hit {
                self.projectile_shoot_event(p.owner, p.guest, p.position.components());
            }
            self.creatures.bounced_followers.clear();
            result?;
        }
        self.flush_lan_explosions();
        let flushed = self.flush_wind_bursts();
        self.native_tick_post_batch(round);
        flushed
    }

    pub(in crate::game) fn apply_projectile_hits(
        &mut self,
        hits: Vec<ProjectileHit>,
        vehicle_winds: &mut super::wind::VehicleWinds,
        mut pending: FxHashMap<JoinSeq, PendingImpact>,
        hears_impact: bool,
    ) -> Result<()> {
        for ProjectileHit {
            projectile: p,
            hit,
            shot_after_hit,
        } in hits
        {
            let (id, join) = (p.id, p.join);
            if hears_impact && self.projectile_removal_pending(id) {
                continue;
            }
            let contact = pending.remove(&join);
            let deferred_arrow =
                contact.is_some() && p.arrow.is_some() && matches!(hit, Hit::Block { .. });
            let wind_flight = vehicle_winds.take(id, hit);
            let landed = match (contact, wind_flight) {
                (Some(contact), Some(before)) => {
                    self.apply_vehicle_wind_impact(p, hit, contact, before)
                }
                (Some(contact), None) => self
                    .apply_captured_impact(p, hit, contact)
                    .map(|p| (p, hit)),
                (None, Some(before)) if self.wind_vehicle_broken(hit) => {
                    let from = before.position;
                    let (mut p, keep, hit) = self.broken_vehicle_retick(before);
                    if keep {
                        if let Some(ceiling) = self.wind_ceiling_contact(&p) {
                            Some((p, ceiling))
                        } else {
                            self.sweep_projectile_inside(&mut p);
                            self.lava_mark_hurt(&mut p);
                            let (pid, to) = (p.id, p.position);
                            self.reinsert_continuing_projectile(p);
                            self.entity_section_step(pid, from.components(), to.components());
                            None
                        }
                    } else {
                        (hit != Hit::None).then_some((p, hit))
                    }
                }
                (None, _) => Some((p, hit)),
            };
            if let Some((p, hit)) = landed {
                if let Err(error) = self.apply_projectile_hit(&p, hit) {
                    if self.creatures.projectiles.locate(join).is_none() {
                        self.creatures.projectiles.retire_extracted(id, join);
                    }
                    return Err(error);
                }
                if shot_after_hit {
                    self.projectile_shoot_event(p.owner, p.guest, p.position.components());
                }
            }
            if deferred_arrow {
                self.finish_deferred_arrow_tick(id, join);
            }
        }
        Ok(())
    }

    pub(in crate::game) fn apply_projectile_hit(&mut self, p: &Projectile, hit: Hit) -> Result<()> {
        // Arrows dispatch each entity in `finish_arrow_step` directly so mixed mob/player batches
        // preserve shooter context and exact pierced entity ids.
        if matches!(p.kind, ProjectileKind::Arrow { .. }) && matches!(hit, Hit::Entity { .. }) {
            return self.projectile_hit(p, hit);
        }
        // A player's snowball/egg/thrown on another player lands as the victim's hurt.
        if let Some(victim) = self.projectile_pvp_victim(p, hit) {
            return self.projectile_hits_player(p, hit, victim);
        }
        // A guest's projectile lands as theirs: a pearl takes them, what it
        // hurts answers to them. One hitting the host stays the host's hit.
        match p.guest {
            Some(conn)
                if !matches!(
                    hit,
                    Hit::Entity {
                        who: Who::Player,
                        ..
                    }
                ) =>
            {
                self.guest_projectile_hit(conn, p, hit)?;
            }
            _ => self.projectile_hit(p, hit)?,
        }
        // LlamaSpit.tick lands the hit before replacing the live velocity with
        // its pre-hit movement, scaled by drag and affected by gravity.
        if p.kind == ProjectileKind::LlamaSpit
            && matches!(hit, Hit::Entity { .. })
            && !self.projectile_removal_pending(p.id)
            && let Some(i) = self.creatures.projectiles.locate(p.join)
        {
            let (from, to) = self.creatures.projectiles.update(i, |spit| {
                let from = spit.position;
                spit.finish_spit_motion(p.velocity);
                (from, spit.position)
            });
            self.entity_section_step(p.id, from.components(), to.components());
        }
        Ok(())
    }
    /// `LivingEntity.hurtServer` for a snowball, egg or pearl (`kind`) that hurt the mob `id`,
    /// 0 damage included: the accepted hit already credited its owner; for a player,
    /// also fire PLAYER_HURT_ENTITY.
    pub(in crate::game) fn thrown_hurt_mob(
        &mut self,
        p: &Projectile,
        id: u32,
        kind: &str,
        amount: f32,
    ) {
        if p.owner == Some(Who::Player)
            && let Some(m) = self.mobs.iter().find(|m| m.id == id)
        {
            let victim = self.mob_entity(m);
            let direct = sunrise_client::advancements::EntityView {
                kind: kind.into(),
                position: [p.position.x, p.position.y, p.position.z],
                ..Default::default()
            };
            self.player_hit(&victim, "minecraft:thrown", Some(direct), amount);
        }
    }
    /// `projectile_hit` for a projectile the guest `conn` shot, run as them;
    /// as the host's when they have left.
    pub(in crate::game) fn guest_projectile_hit(
        &mut self,
        conn: u32,
        p: &Projectile,
        hit: Hit,
    ) -> Result<()> {
        let result = self.as_guest(conn, |g| g.projectile_hit(p, hit));
        // The mob hit now answers to the guest (LivingEntity.lastHurtByPlayer).
        if result.is_some()
            && self
                .lan
                .as_ref()
                .and_then(|host| host.hub.guest(conn))
                .is_some_and(|guest| guest.participates_online())
            && let Hit::Entity {
                who: Who::Mob(id), ..
            } = hit
            && self.mobs.iter().any(|m| m.id == id)
            && let Some(host) = self.lan.as_mut()
        {
            host.mob_players.insert(id, conn);
        }
        match result {
            Some(r) => r,
            None => self.projectile_hit(p, hit),
        }
    }
    /// `EnchantmentHelper.doPostAttackEffects` after the player's shot `p`
    /// (its `direct` entity, a hit of `damage_type`) hurt the mob `who`: the
    /// victim's own post-attack enchantments answer the player (Thorns).
    pub(in crate::game) fn shot_post_attack(
        &mut self,
        p: &Projectile,
        who: Who,
        direct: &str,
        damage_type: &str,
    ) {
        if let Who::Mob(id) = who {
            self.mob_post_attack(id, p.owner, Some(direct), Some(damage_type));
        }
    }
    /// `BellBlock.onProjectileHit`: a projectile that strikes the bell proper
    /// rings it, and a player's shot counts as the player's ring.
    pub(in crate::game) fn bell_projectile_hit(
        &mut self,
        p: &ProjectileHitter,
        pos: [i32; 3],
        at: Vec3,
        face: [i32; 3],
    ) {
        let Some(face) = Direction::ALL.into_iter().find(|d| d.normal() == face) else {
            return;
        };
        let block = BlockPos::new(pos[0], pos[1], pos[2]);
        let rang = self
            .with_level(|b, level, ticks| b.bell_projectile_hit(level, ticks, block, face, at.y));
        if rang {
            let _ = self.process_level_events();
            if p.owner == Some(Who::Player) {
                self.award_custom_stat("bell_ring", 1);
            }
        }
    }
    /// `AmethystBlock.onProjectileHit`: a projectile that strikes an amethyst
    /// block rings `block.amethyst_block.chime` there, pitched by the level random.
    pub(in crate::game) fn amethyst_projectile_hit(&mut self, pos: [i32; 3]) {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        if &*self.res.facts().get(state).name != "minecraft:amethyst_block" {
            return;
        }
        let pitch = 0.5 + self.world.random.next_float() * 1.2;
        self.level_events
            .push(sunrise_world::level::LevelEvent::Sound {
                pos: BlockPos::new(pos[0], pos[1], pos[2]),
                sound: "minecraft:block.amethyst_block.chime".into(),
                source: None,
                volume: 1.0,
                pitch,
            });
        let _ = self.process_level_events();
    }
    /// `onProjectileHit` of chorus flowers, decorated pots and speleothems
    /// (pointed dripstone, sulfur spikes): an impact projectile
    /// (`#impact_projectiles`) breaks the block when `Projectile.mayInteract`
    /// and `Projectile.mayBreak` allow it, which takes the
    /// `projectiles_can_break_blocks` rule and a shooter that may break blocks
    /// (`canBreakBlockInAdventureMode`: a resolved mob's arrow needs its
    /// actual projectile predicate and mob_griefing). Other owner/kind paths
    /// retain their existing policy. A pot is cracked first (flags 260).
    pub(in crate::game) fn projectile_breaks_block(&mut self, p: &Projectile, pos: [i32; 3]) {
        use sunrise_client::command::args::GameMode;
        let impact = matches!(
            p.kind,
            ProjectileKind::Arrow { .. }
                | ProjectileKind::Trident
                | ProjectileKind::Snowball
                | ProjectileKind::Thrown(_)
                | ProjectileKind::Egg
                | ProjectileKind::SmallFireball
                | ProjectileKind::LargeFireball { .. }
                | ProjectileKind::WitherSkull { .. }
                | ProjectileKind::WindCharge
                | ProjectileKind::BreezeWindCharge
        );
        use sunrise_client::arrows::OwnerLookup;
        let shooter_may_break = match p.owner {
            Some(Who::Player) => self.mode != GameMode::Adventure,
            Some(Who::Mob(_)) => {
                matches!(p.kind, ProjectileKind::Arrow { .. })
                    && self
                        .ui
                        .rules
                        .bool_at(sunrise_client::gamerules::vanilla::MOB_GRIEFING)
                    && p.arrow
                        .as_ref()
                        .and_then(|a| a.can_break.as_ref())
                        .is_some_and(|predicate| {
                            super::game_mode::projectile_predicate_supported(predicate)
                                && self.adventure_predicate_at(predicate, pos)
                        })
            }
            None => !p.owner_uuid.is_some_and(|u| self.non_player_entity(u)),
        };
        if !impact
            || !shooter_may_break
            || !self
                .ui
                .rules
                .bool_at(sunrise_client::gamerules::vanilla::PROJECTILES_CAN_BREAK_BLOCKS)
        {
            return;
        }
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        // `destroyBlock(pos, true, projectile)`; a speleothem's names no entity.
        let mut entity = Some(p.id);
        match &*self.res.facts().get(state).name {
            "minecraft:chorus_flower"
                if matches!(
                    p.kind,
                    ProjectileKind::Arrow { .. } | ProjectileKind::Snowball
                ) =>
            {
                // `ChorusFlowerBlock.onProjectileHit` supplies the projectile,
                // even when it has no owner, as the direct block's loot entity.
                let _ = self.destroy_block_by(pos, Some(p.id));
                return;
            }
            "minecraft:chorus_flower" => {}
            // SpeleothemBlock.onProjectileHit (pointed dripstone and sulfur spikes): a
            // thrown trident faster than 0.6 breaks it.
            "minecraft:pointed_dripstone" | "minecraft:sulfur_spike"
                if matches!(p.kind, ProjectileKind::Trident)
                    && p.arrow.as_ref().is_some_and(|a| a.hit_speed > 0.6) =>
            {
                entity = None;
            }
            "minecraft:decorated_pot" => {
                let registry = &self.pack.assets.registry;
                let Some(mut pot) = registry.state(state).cloned() else {
                    return;
                };
                pot.properties.insert("cracked".into(), "true".into());
                let Ok(cracked) = registry.resolve(&pot) else {
                    return;
                };
                self.with_level(|b, level, ticks| b.set_block(level, ticks, at, cracked, 260));
            }
            _ => return,
        }
        let _ = self.destroy_block_by(pos, entity);
    }
    /// `Level.isRainingAt` for arrows.
    pub(in crate::game) fn arrow_rain(&self) -> impl Fn(BlockPos) -> bool + '_ {
        move |p: BlockPos| self.raining_at(p)
    }
    /// The level as arrows see it.
    pub(in crate::game) fn arrow_scene<'s>(
        &'s self,
        rain: &'s dyn Fn(BlockPos) -> bool,
    ) -> sunrise_client::arrows::ArrowScene<'s, 's> {
        sunrise_client::arrows::ArrowScene {
            world: &self.world,
            registry: self.pack.assets.registry,
            facts: self.res.facts(),
            collision: self.collision,
            fluids: &self.motion.fluids,
            loop_context: &self.loop_context,
            raining: rain,
            mob_griefing: self
                .ui
                .rules
                .bool_at(sunrise_client::gamerules::vanilla::MOB_GRIEFING),
            tnt_explodes: self
                .ui
                .rules
                .bool_at(sunrise_client::gamerules::vanilla::TNT_EXPLODES),
            mob_hooks: self
                .grief_hooked()
                .then_some(self as &dyn sunrise_client::mobs::MobHooks),
            owners: self,
        }
    }
    /// Flush queued arrow sounds, particles, block reactions, drops, and shoot events.
    pub(in crate::game) fn flush_arrow_events(&mut self, arrow: &mut Projectile) {
        let Some(a) = arrow.arrow.as_mut() else {
            return;
        };
        let (events, item) = (std::mem::take(&mut a.events), a.pickup_item.clone());
        for event in events {
            if event == sunrise_client::arrows::ArrowEvent::Drop {
                self.spawn_at_location(arrow.position, f64::from(0.1f32), item.clone());
            } else if event == sunrise_client::arrows::ArrowEvent::ShootEvent {
                self.projectile_shoot_event(arrow.owner, arrow.guest, arrow.position.components());
            } else {
                self.arrow_event(arrow.position, event);
            }
        }
    }

    /// Re-insert a projectile that remains alive after entity hit dispatch, preserving tick order.
    pub(in crate::game) fn reinsert_continuing_projectile(&mut self, projectile: Projectile) {
        let followers = self
            .creatures
            .bounced_followers
            .iter()
            .position(|(id, _)| *id == projectile.id)
            .map(|k| self.creatures.bounced_followers.swap_remove(k).1)
            .unwrap_or_default();
        let projectiles = &mut self.creatures.projectiles;
        if let Some(existing) = projectiles.locate(projectile.join) {
            projectiles.store_back(existing, projectile);
            return;
        }
        let index = bounced_place(projectiles, &followers);
        projectiles.insert(index, projectile);
    }

    /// The rest of the tick of an arrow that hit an entity (`arrows::finish_entity_hit`).
    pub(in crate::game) fn finish_arrow_hit(&mut self, mut arrow: Projectile, outcome: EntityHit) {
        let keep = {
            let rain = self.arrow_rain();
            sunrise_client::arrows::finish_entity_hit(&mut arrow, &self.arrow_scene(&rain), outcome)
        };
        self.flush_arrow_events(&mut arrow);
        if keep {
            self.reinsert_continuing_projectile(arrow);
        } else if self.creatures.projectiles.locate(arrow.join).is_none() {
            self.creatures
                .projectiles
                .retire_extracted(arrow.id, arrow.join);
        }
    }

    /// Dispatch the same crystal and dragon-part hit for arrows and mob tridents.
    pub(in crate::game) fn special_arrow_hit(
        &mut self,
        arrow: &mut Projectile,
        id: u32,
        amount: f32,
    ) -> EntityHit {
        let owner = self.resolved_projectile_owner(arrow.owner, arrow.guest);
        let kind = self.projectile_damage_kind(arrow);
        let damage_type = self.projectile_damage_type(arrow);
        // EndCrystal.hurtServer can push this arrow before its next hit or deflection.
        // Lend the contact state to the level walk, as for an impact callback.
        let crystal = self.is_crystal_id(id);
        let join = arrow.join;
        if crystal {
            let mut contact = arrow.clone();
            std::mem::swap(
                &mut contact.attachment_lifetime,
                &mut arrow.attachment_lifetime,
            );
            let followers = self
                .creatures
                .bounced_followers
                .iter()
                .find(|(id, _)| *id == arrow.id)
                .map_or(&[][..], |(_, followers)| followers.as_slice());
            let projectiles = &mut self.creatures.projectiles;
            let index = bounced_place(projectiles, followers);
            projectiles.put_back(index, contact);
        }
        let hurt = self.hit_special_full(
            id,
            amount,
            owner,
            arrow.guest,
            kind,
            Some(damage_type),
            Some(super::dragon::DirectHit::from_projectile(arrow)),
        );
        if crystal {
            if !self.projectile_removal_pending(arrow.id)
                && let Some(live) = self.creatures.projectiles.extract(join)
            {
                *arrow = live;
            } else if let Some(a) = arrow.arrow.as_mut() {
                a.removed = true;
            }
        }
        match hurt {
            super::dragon::Hurt::Landed { .. } => EntityHit::Hurt,
            super::dragon::Hurt::Refused => EntityHit::Refused,
            super::dragon::Hurt::Shrugged => {
                if let Some(a) = arrow.arrow.as_mut() {
                    a.ignite_for_seconds(1.0);
                }
                EntityHit::Refused
            }
        }
    }

    /// Whether `contact` is the attacker's latest affected spear contact applied this tick.
    pub(in crate::game) fn latest_stab_contact(&mut self, attacker: u32, contact: usize) -> bool {
        let now = self.world.game_time;
        let (at, latest) = &mut self.creatures.stab_memory;
        if *at != now {
            *at = now;
            latest.clear();
        }
        let seen = latest.entry(attacker).or_insert(contact);
        if contact < *seen {
            return false;
        }
        *seen = contact;
        true
    }

    /// `setLastHurtMob` at the arrow/wind charge's onHitEntity site.
    pub(in crate::game) fn remember_projectile_target(&mut self, p: &Projectile, who: Who) {
        let target = match who {
            Who::Player => self.player.as_ref().map(|body| body.entity_id as u32),
            Who::Mob(id) => self.last_hurt_living_target(id).map(|_| id),
        };
        match p.owner {
            Some(Who::Mob(id)) => self.remember_mob_target(id, target),
            Some(Who::Player) => {
                if let Some(shooter) = self.projectile_player_owner_id(p.guest) {
                    self.remember_player_projectile_target(shooter, target);
                }
            }
            None => {
                // These living bodies can be assigned as owners, but the projectile's
                // combat shortcut (`Who`) only represents mobs and players.
                let Some(owner) = p.owner_uuid.and_then(|uuid| {
                    self.target_by_uuid(sunrise_client::lan::playerdata::ints_uuid(uuid))
                }) else {
                    return;
                };
                match owner {
                    super::commands::Target::Stand(id) => {
                        if let Some(stand) = self.armor_stands.iter_mut().find(|s| s.id == id) {
                            stand.last_hurt_mob = target.map(|id| (id, stand.tick_count));
                        }
                    }
                    super::commands::Target::Dragon(id) => {
                        if let Some(dragon) = self.dragon.as_mut().filter(|d| d.id == id) {
                            dragon.last_hurt_mob = target.map(|id| (id, dragon.ticks));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// A projectile's hit, landed in the attack of its owner.
    pub(in crate::game) fn projectile_hit(&mut self, p: &Projectile, hit: Hit) -> Result<()> {
        let attack = self.projectile_attack(p);
        let result = self.with_attack(attack, |g| g.projectile_hit_landed(p, hit));
        self.settle_dealt_absorbed(p.owner == Some(Who::Player));
        result?;
        if let Hit::Entity {
            who: Who::Mob(id), ..
        } = hit
        {
            self.settle_mob_hurt(id);
        }
        // The hit's hurt teleports an enderman or a shulker now (`hurtServer`).
        self.settle_hurt_teleports()
    }
    /// `Snowball`/`ThrownEgg.onHit`: entity event 3 for the guests, before the
    /// projectile is discarded (the clients lose it when the tracker next runs).
    pub(in crate::game) fn thrown_break_event(&mut self, id: u32, at: [f64; 3]) {
        if id != 0 {
            self.lan_entity_event(id as i32, 3, at);
        }
    }
    /// `AgeableMob.mobInteract`: whether a click with `item` on `m` gets to
    /// the golden dandelion check (`canUseGoldenDandelion`, which a tadpole
    /// calls with `isBaby` true). A panda that is scared or on its back and an
    /// armadillo in its shell answer in their own `mobInteract` first.
    pub(in crate::game) fn age_lock_takes(&self, m: &Mob, item: &str) -> bool {
        let species = m.species();
        let baby = if species == Species::Tadpole {
            true
        } else if sunrise_client::mobs::finalize::ageable(species) {
            m.is_baby()
                && !(species == Species::Panda
                    && (sunrise_client::mobs::panda::scared(m)
                        || sunrise_client::mobs::panda::is_on_back(m)))
                && !(species == Species::Armadillo && sunrise_client::mobs::armadillo::scared(m))
        } else {
            false
        };
        let cannot = self.items().entity_type_tags.has_bit(
            m.kind.flags.entity_tag_slot,
            sunrise_client::items::EntityTypeTags::CANNOT_BE_AGE_LOCKED,
        );
        sunrise_client::mobs::ai::can_use_golden_dandelion(
            item,
            baby,
            m.extra.age_lock_particle_timer,
            cannot,
        )
    }
    /// The mob a normal interaction can reach: a packet's exact validated ID,
    /// or the crosshair's mob before its blocking shape.
    pub(in crate::game) fn mob_interaction_target(
        &self,
        target: Option<InteractTarget>,
    ) -> Option<usize> {
        let (index, distance, _) = (match target {
            // The id the packet named, under the filters the crosshair pick
            // applies (`mob_hit`): a mob in its death animation is not clicked
            // by either path, so naming one in a packet must not reach the
            // feed, shear, leash or name paths either.
            Some(t) => Self::named_entity(&self.mobs, t.id, |m| m.id)
                .filter(|&i| {
                    let m = &self.mobs[i];
                    named_mob_pickable(m.alive(), m.id, self.riding, self.live_remote_camera_id())
                        && !self.is_pick_camera(m.id)
                })
                .map(|i| (i, 0.0, [0.0; 3])),
            None => self.mob_hit(),
        })?;
        // Only a client's own pick is stopped by a block: the range check passed.
        if target.is_none()
            && !self.uses_stored_pick()
            && let Some(hit) = self.pick_unfiltered()
            && let Some(eye) = self.eye_position()
        {
            let d = ((hit.point[0] - eye[0]).powi(2)
                + (hit.point[1] - eye[1]).powi(2)
                + (hit.point[2] - eye[2]).powi(2))
            .sqrt();
            if d < distance {
                return None;
            }
        }
        Some(index)
    }
    /// `SnowGolem.mobInteract` and `Bogged.mobInteract` with shears: the
    /// pumpkin or the mushrooms come off (the shearing loot table), the shear
    /// game event is posted and the tool takes one damage.
    pub(in crate::game) fn interact_shear_mob(&mut self, index: usize, shears: bool) -> bool {
        let m = &self.mobs[index];
        if !shears || !m.alive() || m.extra.sheared {
            return false;
        }
        let (sound, table, height) = match m.species() {
            Species::SnowGolem => (
                "minecraft:entity.snow_golem.shear",
                "minecraft:shearing/snow_golem",
                m.eye_height(),
            ),
            Species::Bogged => (
                "minecraft:entity.bogged.shear",
                "minecraft:shearing/bogged",
                f64::from(m.height()),
            ),
            _ => return false,
        };
        let at = m.position;
        let mob = (m.id, super::native_hooks::mob_kind(m));
        let tool = self.held().cloned();
        // shear: level.playSound(null, this, sound, PLAYERS, 1, 1).
        let shear =
            LevelSound::new(sound, [at.x, at.y, at.z], 1.0, 1.0).in_source(SoundSource::Players);
        self.play_bound_sound(shear, mob.0, None);
        self.mobs[index].extra.sheared = true;
        let mut stacks = self.sequence_loot(table, table, tool.as_ref());
        let origin = [at.x, at.y, at.z];
        let player = Some(self.native_acting_id());
        self.native_use_drops(
            table,
            origin,
            None,
            Some((mob.0, &mob.1)),
            player,
            tool.as_ref(),
            &mut stacks,
        );
        for stack in stacks {
            self.spawn_at_location(at, height, stack);
        }
        let ctx = self.who_context(Some(sunrise_client::mobs::Who::Player), None);
        self.post_game_event("minecraft:shear", [at.x, at.y, at.z], ctx);
        self.damage_held_tool();
        true
    }
    /// `ItemStack.hurtAndBreak(1, ...)` on the held tool: Unbreaking, break sound and stat.
    pub(in crate::game) fn damage_held_tool(&mut self) {
        self.damage_held(1);
    }
    /// `Mob.checkAndHandleImportantInteractions` with a spawn egg of mob
    /// `index`'s own kind (`SpawnEggItem.spawnOffspringFromSpawnEgg`): a baby
    /// at the mob's feet, taking the parent's variant as `getBreedOffspring`
    /// does, and one egg used. False (PASS) when the mob has no baby form.
    pub(in crate::game) fn spawn_egg_offspring(&mut self, index: usize) -> bool {
        let Some(stack) = self.held().cloned() else {
            return false;
        };
        let parent = &self.mobs[index];
        if !parent.alive()
            || !stack.item.ends_with("_spawn_egg")
            || sunrise_client::block_entity_item::item_entity_data(&stack, self.res.items())
                .is_none_or(|data| data.kind != parent.kind.id)
            || self.ui.difficulty == 0 && !parent.kind.allowed_in_peaceful()
            || !sunrise_client::mobs::species::can_be_a_baby(parent.species())
            // WanderingTrader.getBreedOffspring returns null: no offspring, the
            // click carries on to trading.
            || parent.species() == Species::WanderingTrader
        {
            return false;
        }
        let (kind, at) = (
            parent.kind.id.clone(),
            [parent.position.x, parent.position.y, parent.position.z],
        );
        let (species, variant, color) =
            (parent.species(), parent.extra.variant, parent.extra.color);
        // `MushroomCow.getBreedOffspring(level, this)`: the egg's mate is the
        // parent itself, so the colour always rolls `nextInt(1024) == 0` to
        // flip, and every other roll spends the coin flip that can only pick
        // that same colour again. Both come from the parent's own random.
        let variant = if species == Species::Mooshroom {
            let random = &mut self.mobs[index].random;
            if random.next_int_bound(1024).unwrap_or(1) == 0 {
                if variant == 1 { 0 } else { 1 }
            } else {
                let _ = random.next_boolean();
                variant
            }
        } else {
            variant
        };
        let target = self.mob_entity(&self.mobs[index]);
        // SpawnEggItem.spawnOffspringFromSpawnEgg: `type.create(level, SPAWN_ITEM_USE)`
        // creates the baby without finalizeSpawn, and `snapTo(pos, 0, 0)` faces
        // it south without a level-random draw.
        let Some(child) = self.create_mob_reason(&kind, at, false, SpawnSite::Raw, Some(0.0))
        else {
            return false;
        };
        let Some(id) = self.insert_spawned_mob(child) else {
            return false;
        };
        if let Some(child) = self.mobs.iter_mut().find(|m| m.id == id) {
            sunrise_client::mobs::finalize::spawn_egg_offspring(child, species, variant, color);
        }
        // `Mob.checkAndHandleImportantInteractions` then calls
        // `onOffspringSpawnedFromEgg`: `Fox`'s makes the baby trust the player
        // who used the egg (a fox egg used on a block trusts nobody).
        if species == Species::Fox {
            let spawner = self.acting_player_uuid();
            if let Some(child) = self.mobs.iter_mut().find(|m| m.id == id) {
                sunrise_client::mobs::fox::add_trusted_entity(child, spawner);
            }
        }
        self.trigger_item_entity("minecraft:player_interacted_with_entity", &stack, &target);
        self.consume_held();
        // `Mob.interact`: the egg's `SUCCESS_SERVER` consumes the click, so the
        // interaction posts `ENTITY_INTERACT` with the player, at the mob.
        let ctx = self.who_context(Some(Who::Player), None);
        self.post_game_event("minecraft:entity_interact", at, ctx);
        true
    }
    /// `SpawnEggItem.use`: the SOURCE_ONLY ray must hit a liquid block.
    pub(in crate::game) fn spawn_egg_fluid_hit(&self) -> Option<super::Hit> {
        let hit = self.bucket_hit()?;
        matches!(
            &*self.block_name(hit.pos),
            "minecraft:water" | "minecraft:lava"
        )
        .then_some(hit)
    }
    /// Shared peaceful-mode eligibility for spawn eggs.
    pub(in crate::game) fn spawn_egg_allowed(&self, kind: &str) -> bool {
        self.ui.difficulty != 0
            || self
                .res
                .mob_types()
                .get(kind)
                .is_none_or(|t| t.allowed_in_peaceful())
    }
    /// `EntityType.updateCustomEntityTag`: the egg's `minecraft:entity_data` on
    /// the mob it spawned, once the guards let it through, as
    /// `TypedEntityData.loadInto`: the mob's own compound, the data merged over
    /// it, loaded back into the same mob (`/data merge entity`'s loader).
    pub(in crate::game) fn apply_spawn_egg_entity_data(
        &mut self,
        id: u32,
        data: &sunrise_client::block_entity_item::ItemEntityData<'_>,
    ) {
        use sunrise_core::nbt::Tag;
        // A type-only component has no custom fields to apply.
        if data.compound.len() == 1 || !self.acting_player_is_op() && data.op_only() {
            return;
        }
        let Some(i) = self
            .mobs
            .iter()
            .position(|m| m.id == id && m.kind.id == data.kind)
        else {
            return;
        };
        if let Tag::Compound(mut tag) = self.mob_tree_nbt(i) {
            sunrise_client::data_cmd::merge_compound(&mut tag, data.compound);
            // Nothing to report: the mob's own compound always loads back.
            let _ = self.apply_entity_nbt(super::commands::Target::Mob(id), Tag::Compound(tag));
        }
    }
    /// A beam target's centre (half its height), interpolated.
    pub(in crate::game) fn beam_target(&self, who: Option<Who>, alpha: f64) -> Option<glam::DVec3> {
        match who? {
            Who::Player => {
                let p = self.player.as_ref()?;
                let (a, b) = (self.previous_position, p.position);
                Some(glam::DVec3::new(
                    a.x + (b.x - a.x) * alpha,
                    a.y + (b.y - a.y) * alpha + f64::from(p.dimensions.height) * 0.5,
                    a.z + (b.z - a.z) * alpha,
                ))
            }
            Who::Mob(id) => {
                let m = self.mobs.iter().find(|m| m.id == id)?;
                Some(glam::DVec3::new(
                    m.previous.x + (m.position.x - m.previous.x) * alpha,
                    m.previous.y
                        + (m.position.y - m.previous.y) * alpha
                        + f64::from(m.height()) * 0.5,
                    m.previous.z + (m.position.z - m.previous.z) * alpha,
                ))
            }
        }
    }

    pub(in crate::game) fn mob_run_geometry(
        &self,
        frame: &MobDrawFrame<'_>,
        range: std::ops::Range<usize>,
        out: &mut Vec<Vertex>,
    ) {
        if frame.precheck {
            self.mob_run::<true>(&frame.living, range, out);
        } else {
            self.mob_run::<false>(&frame.living, range, out);
        }
    }

    pub(in crate::game) fn mob_run<const PRECHECK: bool>(
        &self,
        frame: &LivingFrame<'_>,
        range: std::ops::Range<usize>,
        out: &mut Vec<Vertex>,
    ) {
        for mob in &self.mobs[range] {
            self.living_geometry::<PRECHECK>(mob, frame, out);
        }
    }

    #[cfg(test)]
    pub(in crate::game) fn mob_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
    ) {
        let frame = self.begin_mob_geometry(camera, alpha);
        self.mob_run_geometry(&frame, 0..self.mobs.len(), out);
        self.mark_motion(out.len(), 0);
        self.other_creature_geometry(camera, alpha, out);
    }

    pub(in crate::game) fn other_creature_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
    ) {
        self.elder_apparition_geometry(alpha, out);
        self.spawner_geometry(camera, alpha, out);
        let live = self
            .creatures
            .projectiles
            .iter()
            .filter(|a| !self.hides_as_camera(a.id))
            .map(|a| {
                let at = [
                    a.previous.x + (a.position.x - a.previous.x) * alpha,
                    a.previous.y + (a.position.y - a.previous.y) * alpha,
                    a.previous.z + (a.position.z - a.previous.z) * alpha,
                ];
                (a, at)
            });
        self.projectile_geometry(camera, alpha, live, out);
    }
}
/// `ThrownItemRenderer.submit` turns the item by `CameraRenderState.orientation`
/// (`Camera.setRotation`): the billboard's right and up axes are the camera's.
/// They stay continuous at pitch ±90, and in third-person front view they are
/// the turned-round camera's, not the player's.
pub(in crate::game) fn thrown_item_axes(
    camera_type: sunrise_client::camera::CameraType,
    yaw: f32,
    pitch: f32,
) -> (glam::Vec3, glam::Vec3) {
    let orientation = super::camera_rotation(camera_type, yaw, pitch);
    (orientation * glam::Vec3::X, orientation * glam::Vec3::Y)
}
/// The four corners of the item billboard, in the order its UVs are laid out.
/// `ThrownItemRenderer` only scales and billboards it; the lift is
/// `item/generated`'s ground display transform, `translation [0, 2, 0]` of 16
/// units = 0.125 blocks, which is `0.125 * scale` == `0.5 * s`.
pub(in crate::game) fn thrown_item_quad(
    at: glam::Vec3,
    right: glam::Vec3,
    up: glam::Vec3,
    scale: f32,
    lift: bool,
) -> [glam::Vec3; 4] {
    let s = 0.25 * scale;
    let rise = if lift { 0.5 * s } else { 0.0 };
    let centre = at + up * rise;
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|k| centre + right * k.0 * s + up * k.1 * s)
}
/// Whether vanilla draws the kind as `ThrownItemRenderer`'s item billboard, the
/// only ones that take that lift. Llama spit shares this quad but
/// `LlamaSpitRenderer` is a model, with its own offset, and a fireball's item
/// is drawn as the item model that carries the lift itself.
pub(in crate::game) fn is_item_billboard(kind: &ProjectileKind) -> bool {
    matches!(
        kind,
        ProjectileKind::Snowball | ProjectileKind::Egg | ProjectileKind::EnderPearl
    )
}
/// `Bucketable.getBucketItemStack` and `getPickupSound` of a mob that a water
/// bucket picks up.
pub(in crate::game) fn bucketable(species: Species) -> Option<(&'static str, &'static str)> {
    let fish = "minecraft:item.bucket.fill_fish";
    Some(match species {
        Species::Cod => ("minecraft:cod_bucket", fish),
        Species::Salmon => ("minecraft:salmon_bucket", fish),
        Species::Pufferfish => ("minecraft:pufferfish_bucket", fish),
        Species::TropicalFish => ("minecraft:tropical_fish_bucket", fish),
        Species::Tadpole => (
            "minecraft:tadpole_bucket",
            "minecraft:item.bucket.fill_tadpole",
        ),
        Species::Axolotl => (
            "minecraft:axolotl_bucket",
            "minecraft:item.bucket.fill_axolotl",
        ),
        _ => return None,
    })
}
#[allow(unused_imports)]
pub(in crate::game) use sunrise_client::health::{damage_type, melee, source_damage_type};
/// A live item as a loot stack: ordinary enchantments and the complete saved
/// component patch, including removals. The TOOL getter resolves its prototype.
pub(in crate::game) fn to_loot(stack: &ItemStack) -> sunrise_world::loot::LootStack {
    let components = match sunrise_client::item_data::component_snapshot(stack) {
        Some(sunrise_core::nbt::Tag::Compound(patch)) => patch
            .into_iter()
            .map(|(id, value)| {
                let id = id.to_string_lossy();
                if let Some(base) = id.strip_prefix('!') {
                    (base.to_owned(), None)
                } else {
                    (id, Some(value))
                }
            })
            .collect(),
        _ => Vec::new(),
    };
    sunrise_world::loot::LootStack {
        item: stack.item.to_string(),
        count: stack.count as i32,
        damage: stack.damage as i32,
        enchantments: stack
            .data()
            .map(|data| data.enchantments.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|(e, l)| (e.to_string(), *l as i32))
            .collect(),
        potion: None,
        map: false,
        exploration: None,
        ominous_amplifier: None,
        stew_effects: Vec::new(),
        names: Vec::new(),
        components,
    }
}
/// A generated loot stack as a Sunrise item stack.
pub(in crate::game) fn from_loot(stack: &sunrise_world::loot::LootStack) -> ItemStack {
    let mut item = ItemStack {
        item: Arc::from(stack.item.as_str()),
        count: stack.count.max(1) as u32,
        damage: stack.damage.max(0) as u32,
        extra: None,
    };
    for (id, level) in &stack.enchantments {
        item.enchant(id, (*level).max(0) as u32);
    }
    if let Some(potion) = &stack.potion {
        item = item.with_potion(potion);
    }
    if let Some(amplifier) = stack.ominous_amplifier.filter(|a| *a > 0) {
        item.data_mut().ominous_amplifier = Some(amplifier as u8);
    }
    for (id, text) in &stack.names {
        item.data_mut().set_loot_name(id, text);
    }
    for (id, value) in &stack.components {
        item.data_mut().set_loot_component(id, value);
    }
    item
}
/// The instance a mob effect gives the player; `None` for a creative or
/// spectator player (`immune`) unless it is ambient (the nautilus's Breath).
pub(in crate::game) fn mob_effect_instance(
    effect: &'static str,
    ticks: i32,
    amplifier: i32,
    ambient: bool,
    immune: bool,
) -> Option<sunrise_client::effects::EffectInstance> {
    if immune && !ambient {
        return None;
    }
    let id = effect.strip_prefix("minecraft:").unwrap_or(effect);
    let mut instance = sunrise_client::effects::EffectInstance::new(id, ticks, amplifier);
    instance.ambient = ambient;
    Some(instance)
}

include!("mob_query_hooks.rs");

impl Game<'_> {
    /// `EventHooks.canEntityGrief(level, entity)`: the `mob_griefing` rule,
    /// which a mod may change for `entity` (`EntityMobGriefingEvent`). No
    /// entity: the rule alone, as NeoForge.
    pub(in crate::game) fn can_entity_grief(&self, entity: Option<u32>) -> bool {
        let rule = self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::MOB_GRIEFING);
        entity.map_or(rule, |entity| self.entity_grief(entity, rule))
    }
    /// Whether a large fireball's blast lights fire (`LargeFireball.onHit`):
    /// [`Self::can_entity_grief`] for its owner, as `getOwner()` gives it. That
    /// is the mob that shot it, or the player who owns it (the guest on `guest`,
    /// else the player in place). An owner that is gone, or none, reads the rule.
    pub(in crate::game) fn large_fireball_fire(
        &self,
        owner: Option<Who>,
        guest: Option<u32>,
    ) -> bool {
        self.can_entity_grief(match owner {
            Some(Who::Mob(id)) => Some(id),
            Some(Who::Player) => self
                .player_entity_id_by_connection(guest)
                .map(|id| id as u32),
            None => None,
        })
    }
    /// Whether a mod may answer `can_entity_grief` differently from the rule:
    /// the sites that skip their whole work while griefing is off skip it only
    /// when no mod listens.
    pub(in crate::game) fn grief_hooked(&self) -> bool {
        self.native_authoritative()
            && mod_api::__private::listened(mod_api::HookId::EntityMobGriefing)
    }
    /// [`Self::can_entity_grief`] for `entity`, with the rule already read.
    pub(in crate::game) fn entity_grief(&self, entity: u32, rule: bool) -> bool {
        if !self.grief_hooked() {
            return rule;
        }
        let Some(epoch) = self.native_lifecycle.epoch() else {
            return rule;
        };
        let mut can_grief = rule;
        self.native_entity_mob_griefing(mod_api::EntityRef::new(epoch, entity), &mut can_grief);
        can_grief
    }
}

#[cfg(test)]
#[path = "mob_griefing_tests.rs"]
pub(in crate::game) mod mob_griefing_tests;

#[cfg(test)]
pub(in crate::game) mod control_mobs_sitting_tests;

#[cfg(test)]
pub(in crate::game) mod living_visibility_tests;

#[cfg(test)]
pub(in crate::game) mod enchanted_entity_loot_tests;

#[cfg(test)]
pub(in crate::game) mod enderman_anger_tests;

#[cfg(test)]
pub(in crate::game) mod local_sound_event_tests;

#[cfg(test)]
pub(in crate::game) mod general_creature_tests;

/// Whether the killing blow's causing entity (`DamageSource.getEntity`) was a
/// mob, such as a tame wolf whose owner gets the kill credit.
pub(in crate::game) fn killed_by_mob(m: &Mob) -> bool {
    m.last_damage
        .is_some_and(|(d, _)| matches!(d.attacker, Some(Who::Mob(_))))
}

/// `Animal.finalizeSpawnChildFromBreeding`'s player: the one who fed the
/// parent, or else the one who fed the partner (`getLoveCause`). `Some(None)`
/// is the host, `Some(Some(conn))` a guest. Both animals are spent.
/// `DyeColor.getMixedColor`: the colour of the crafting recipe that mixes
/// the two dyes, else one of the two picked at random.
pub(in crate::game) fn dye_mix(
    context: &sunrise_world::native_load_context::LoopContext,
    recipes: &Recipes,
    random: &mut impl RandomSource,
    a: u8,
    b: u8,
) -> u8 {
    if let Some(mixed) = dye_color_mix_in_recipes(context, recipes, a, b) {
        return mixed;
    }
    if random.next_boolean() { a } else { b }
}

/// `DyeColor.findColorMixInRecipes`: the dye colour a two-dye crafting recipe
/// makes, when its result is a dye. The two dyes are the grid's first row
/// (`CraftingInput.of(2, 1, ...)`) and the recipe manager answers for it; the
/// result's `_dye` item names the colour.
pub(in crate::game) fn dye_color_mix_in_recipes(
    context: &sunrise_world::native_load_context::LoopContext,
    recipes: &Recipes,
    a: u8,
    b: u8,
) -> Option<u8> {
    let dye = |c: u8| {
        let name = sunrise_client::mobs::render::DYE_NAMES[usize::from(c) & 15];
        Some(ItemStack::new(&format!("minecraft:{name}_dye"), 1))
    };
    let grid = [dye(a), dye(b), None, None];
    let (_, result) = recipes.find(context, &grid, 2)?;
    let name = result
        .item
        .strip_prefix("minecraft:")?
        .strip_suffix("_dye")?;
    let index = sunrise_client::mobs::render::DYE_NAMES
        .iter()
        .position(|n| *n == name)?;
    u8::try_from(index).ok()
}

pub(in crate::game) fn breeding_credit(
    love_cause: &mut std::collections::HashMap<u32, Option<u32>>,
    parent: u32,
    partner: u32,
) -> Option<Option<u32>> {
    let from_parent = love_cause.remove(&parent);
    let from_partner = love_cause.remove(&partner);
    from_parent.or(from_partner)
}

#[cfg(test)]
pub(in crate::game) mod mob_subscription_tests;

#[cfg(test)]
pub(in crate::game) mod shulker_arrow_tests;

#[cfg(test)]
pub(in crate::game) mod breeding_tests;

#[cfg(test)]
pub(in crate::game) mod projectile_batch_tests;

#[cfg(test)]
pub(in crate::game) mod breeze_random_tests;

#[cfg(test)]
pub(in crate::game) mod damage_type_tests;

#[cfg(test)]
pub(in crate::game) mod pearl_tests;

#[cfg(test)]
pub(in crate::game) mod knockback_tests;

#[cfg(test)]
pub(in crate::game) mod dye_mix_tests;

impl sunrise_client::arrows::OwnerLookup for Game<'_> {
    fn non_player_entity(&self, uuid: [i32; 4]) -> bool {
        let found = self.target_by_uuid(sunrise_client::lan::playerdata::ints_uuid(uuid));
        !matches!(
            found,
            None | Some(super::commands::Target::Player | super::commands::Target::Guest(_))
        )
    }
}

#[cfg(test)]
pub(in crate::game) mod conversion_subscriber_tests;

#[cfg(test)]
pub(in crate::game) mod living_drops_tests;

#[cfg(test)]
pub(in crate::game) mod armor_destroy_tests;

#[cfg(test)]
pub(in crate::game) mod conversion_tag_tests;

#[cfg(test)]
pub(in crate::game) mod pickable_projectile_tests;

#[cfg(test)]
pub(in crate::game) mod mob_pearl_hook_tests;

#[cfg(test)]
pub(in crate::game) mod breeding_orb_tests;

#[cfg(test)]
pub(in crate::game) mod enchanted_block_loot_breaker_tests;

#[cfg(test)]
#[path = "rabbit_colour_tests.rs"]
pub(in crate::game) mod rabbit_colour_tests;

#[cfg(test)]
pub(in crate::game) mod chorus_projectile_drop_tests;

#[cfg(test)]
pub(in crate::game) mod loaded_arrow_shot_tests;

#[cfg(test)]
pub(in crate::game) mod loaded_mob_arrow_permission_tests;

#[cfg(test)]
pub(in crate::game) mod h169_glide_travel_tests;

/// `ServerLevel.getCurrentDifficultyAt` in `world`: the difficulty, the level's clock, and
/// the inhabited time and moon of the chunk at `at` (both 0 when it isn't loaded).
pub(in crate::game) fn local_difficulty_in(
    world: &sunrise_client::world::World,
    at: [f64; 3],
) -> spawning::LocalDifficulty {
    use sunrise_world::food::Difficulty;
    let base = match world.health_rules.difficulty {
        Difficulty::Peaceful => 0,
        Difficulty::Easy => 1,
        Difficulty::Normal => 2,
        Difficulty::Hard => 3,
    };
    let key = ((at[0].floor() as i32) >> 4, (at[2].floor() as i32) >> 4);
    let (inhabited, moon) = if world.chunks.contains_key(&key) {
        let phase = world.day_time.div_euclid(24000).rem_euclid(8) as usize;
        (
            world.inhabited.get(&key).copied().unwrap_or(0),
            spawning::MOON_BRIGHTNESS[phase],
        )
    } else {
        (0, 0.0)
    };
    spawning::LocalDifficulty::new(base, world.day_time, inhabited, moon)
}
#[cfg(test)]
#[path = "../../entities/tests/leash_without_item_criterion_tests.rs"]
pub(in crate::game) mod leash_without_item_criterion_tests;
#[cfg(test)]
pub(in crate::game) mod pending_birth_matrix_tests;
#[cfg(test)]
pub(in crate::game) mod pending_birth_tests;

#[cfg(test)]
#[path = "../../entities/tests/leash_without_item_postbaseline_controls.rs"]
pub(in crate::game) mod leash_without_item_postbaseline_controls;

#[cfg(test)]
#[path = "../tests/revenge_home_tests.rs"]
pub(in crate::game) mod revenge_home_tests;

pub(in crate::game) mod knockback;

#[cfg(test)]
pub(in crate::game) mod death_loot_context_tests;

#[cfg(test)]
pub(in crate::game) mod breeding_heart_tests;

#[cfg(test)]
#[path = "frame_projectile_tests.rs"]
pub(in crate::game) mod frame_projectile_tests;

#[cfg(test)]
#[path = "projectile_targets_tests.rs"]
pub(in crate::game) mod projectile_targets_tests;

#[cfg(test)]
#[path = "projectiles_120_124_tests.rs"]
pub(in crate::game) mod projectiles_120_124_tests;
