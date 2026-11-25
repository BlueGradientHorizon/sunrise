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
#[cfg(test)]
pub(in crate::game) mod last_hurt_mob_tests;
pub(in crate::game) mod living_draw;
use living_draw::LivingFrame;
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
#[cfg(test)]
pub(in crate::game) mod retained_limits_tests;
#[cfg(test)]
#[path = "../../creatures/spawn_egg_mob_tests.rs"]
pub(in crate::game) mod spawn_egg_mob_tests;
#[cfg(test)]
#[path = "../../creatures/spectator_view_tests.rs"]
pub(in crate::game) mod spectator_view_tests;
pub(in crate::game) mod world_view;
#[allow(unused_imports)]
use super::api_values::TryIntoApi;
use super::{Dim, Game, InteractTarget, native_projectiles::Impact, view_vector};
use std::sync::Arc;
#[allow(unused_imports)]
use sunrise_client::{
    Result,
    arrows::EntityHit,
    blocks::Fluid,
    crafting::Recipes,
    health::{Attack, HurtSource, Hurting, Knock},
    items::{ItemStack, Items},
    mesher::Vertex,
    mobs::{
        Damage, DamageKind, Mob, MobEvent, MobWorld, PlayerView, Seen, SpawnSetup, Species, Who,
        spawning::{self, BiomeFacts},
    },
    projectiles::{Hit, Projectile, ProjectileKind},
};
use sunrise_core::{
    geometry::{Aabb, Vec3},
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

pub(in crate::game) fn arrow_sound_name(sound: sunrise_client::arrows::ArrowSound) -> &'static str {
    use sunrise_client::arrows::ArrowSound;
    match sound {
        ArrowSound::Splash => "minecraft:entity.generic.splash",
        ArrowSound::HitGround => "minecraft:entity.arrow.hit",
        ArrowSound::CrossbowHit => "minecraft:item.crossbow.hit",
        ArrowSound::Extinguish => "minecraft:entity.generic.extinguish_fire",
        ArrowSound::TridentHitGround => "minecraft:item.trident.hit_ground",
        ArrowSound::TridentHit => "minecraft:item.trident.hit",
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

impl<__Side: crate::game::Side> Game<'_, __Side> {
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
    /// `ServerLevel.getCurrentDifficultyAt` in the level in place.
    pub(in crate::game) fn local_difficulty(&self, at: [f64; 3]) -> spawning::LocalDifficulty {
        local_difficulty_in(&self.world, at)
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
        self.side_handle_mob_events_reusable(events)
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

    /// `Level.isRainingAt` for arrows.
    pub(in crate::game) fn arrow_rain(&self) -> impl Fn(BlockPos) -> bool + '_ {
        move |p: BlockPos| self.raining_at(p)
    }
    /// The level as arrows see it, parameterized by side-owned hooks and owner table.
    pub(in crate::game) fn arrow_scene_with<'s>(
        &'s self,
        rain: &'s dyn Fn(BlockPos) -> bool,
        mob_hooks: Option<&'s dyn sunrise_client::mobs::MobHooks>,
        owners: &'s dyn sunrise_client::arrows::OwnerLookup,
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
            mob_hooks,
            owners,
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
                    named_mob_pickable(
                        m.alive(),
                        m.id,
                        self.riding,
                        self.side_live_remote_camera_id(),
                    ) && !self.side_is_pick_camera(m.id)
                })
                .map(|i| (i, 0.0, [0.0; 3])),
            None => self.mob_hit(),
        })?;
        // Only a client's own pick is stopped by a block: the range check passed.
        if target.is_none()
            && !self.side_uses_stored_pick()
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
    /// Shared peaceful-mode eligibility for spawn eggs.
    pub(in crate::game) fn spawn_egg_allowed(&self, kind: &str) -> bool {
        self.ui.difficulty != 0
            || self
                .res
                .mob_types()
                .get(kind)
                .is_none_or(|t| t.allowed_in_peaceful())
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
