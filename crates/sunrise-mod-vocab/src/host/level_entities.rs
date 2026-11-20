//! The `level_entities` area: entities and players in a level, and what a level spawns as items and experience. Host method declarations; access defaults and game forwarders are generated.
#![allow(unused_variables)]
use crate::*;
/// The handles' reads of the `level_entities` area, implemented by the game and reached through [`Backend`].
// hidden: a handle's backend, reached through `Backend`; mods call the handle, never this trait
#[doc(hidden)]
#[sunrise_mod_api_macros::host_area]
pub trait LevelEntitiesBackend {
    /// Side: server.
    /// Visit members of the server level's entity lookup (`ServerLevel.getAllEntities`),
    /// including spectators and entities outside any spatial query. Dragon parts are not
    /// lookup members. Vehicles being removed are skipped.
    /// The visitor can read the supplied handle through the host. Return
    /// `ControlFlow::Break(())` to stop and get `Ok(ControlFlow::Break(()))` from the walk.
    /// Completion, including an empty walk, returns `Ok(ControlFlow::Continue(()))`.
    ///
    /// A stale level returns `LevelReadError::Stale`. A call outside server world access,
    /// a parked level, or an unavailable member returns `LevelReadError::Unavailable`
    /// before any visit. In the block-entity item-tick view, unavailable members are
    /// skipped instead.
    /// Order is lookup admission order, preserved across temporary extraction;
    /// removal and re-admission moves an entity to the end. An entity spawned
    /// during play is admitted as it spawns, even when its id comes later. A
    /// chunk's entities join in the order it stored them: section by section,
    /// each in the order its entities entered it, as vanilla stores them. A
    /// server section crossing is recorded as the entity moves
    /// (`Callback.onMove`), whether or not a mod listens: a move out and back is
    /// two crossings, and a passenger enters after its vehicle. Minecarts,
    /// boats, projectiles, items, primed TNT, falling blocks, fireworks, orbs and
    /// the other server entities that report after their own update count
    /// several moves within one tick as one, from where it was to where it
    /// ended; a block piston's push is recorded only while a mod listens to
    /// `entity_entering_section`. Entities kept outside the residency families
    /// (lingering potion clouds, lightning bolts, eyes of ender) are not
    /// members.
    /// The walk does not build a result buffer. Outside the item-tick view it checks
    /// that all eligible members resolve before invoking the visitor, even if it would
    /// stop on the first member.
    #[access(WorldOnly)]
    fn level_visit_all_entities(
        &self,
        level: LevelRef,
        visit: &mut dyn FnMut(EntityRef) -> core::ops::ControlFlow<()>,
    ) -> Result<core::ops::ControlFlow<()>, LevelReadError>;
    /// Side: both.
    /// Visit readable entities whose bounding boxes intersect this box (`Level.getEntities`).
    /// Face-only contact does not intersect. Inverted bounds are swapped on each axis,
    /// as in vanilla's `AABB`. The visitor can read each handle through the same host.
    /// Return `ControlFlow::Break(())` to stop; the walk returns `Ok(ControlFlow::Break(()))`.
    /// Full completion, including an empty box, returns `Ok(ControlFlow::Continue(()))`.
    /// No result list is built and an early stop does not walk later matches.
    ///
    /// A stale level returns `LevelReadError::Stale`. Non-finite bounds, an inactive
    /// server level, a client that has not logged in or is not displaying this level,
    /// or unavailable world access return `LevelReadError::Unavailable`. A refused
    /// walk does not call the visitor.
    ///
    /// The walk uses entity-list order, then item frames and paintings in join order
    /// (frames first on ties), then the dragon's body. On the server, block, item and text
    /// displays, markers, interactions and fishing hooks come next, then placed guests
    /// other than spectators. Displays and markers use zero-sized physical boxes,
    /// not rendering bounds. The client walks the entities it tracks, including tracked
    /// players, then its own player if not spectating. The dragon's intersecting parts
    /// come last on either side, in vanilla part-table order.
    ///
    /// Unresolved handles are skipped, including held-out server entities. Tridents
    /// that do not resolve, eyes of ender and leash knots are not visited. The projectile
    /// list is temporarily absent during `hears_projectile_impacts`; it is visible again
    /// during `projectile_impact`.
    #[access(WorldOnly)]
    fn level_visit_entities_in(
        &self,
        level: LevelRef,
        min: [f64; 3],
        max: [f64; 3],
        visit: &mut dyn FnMut(EntityRef) -> core::ops::ControlFlow<()>,
    ) -> Result<core::ops::ControlFlow<()>, LevelReadError>;
    /// Side: server.
    /// `ServerLevel.players`: the online players in this level, parked or in
    /// place, each once under their own UUID, also during a guest's turn. Fake
    /// players are not listed, as vanilla's list leaves out connectionless
    /// players; a player displaced by a nested guest turn is not listed either,
    /// since its handle cannot resolve until that turn ends. None on a client
    /// and for a stale handle.
    #[access(WorldOnly)]
    fn level_online_players(&self, level: LevelRef) -> Option<Vec<PlayerRef>>;
    /// Side: server.
    /// Every entity the segment from `from` to `to` passes through, nearest first: the test
    /// `ProjectileUtil.getEntityHitResult` makes on each candidate, each entity's box grown by
    /// `margin` on every side and clipped against the segment. A ray that starts inside a box
    /// does not hit that entity, as in vanilla. `exclude` (the shooter) is skipped. Vanilla's
    /// filter (`canHitEntity`, `isPickable`) is the caller's: take the first hit it accepts.
    /// Candidates are those `visit_entities_in` finds, so the same kinds are left out. None
    /// for a negative or non-finite margin, a non-finite point, or a refused spatial walk.
    #[access(WorldOnly)]
    fn level_ray_trace_entities(
        &self,
        level: LevelRef,
        from: [f64; 3],
        to: [f64; 3],
        margin: f64,
        exclude: Option<EntityRef>,
    ) -> Option<Vec<EntityRayHit>>;
    /// Side: both.
    /// The nearest accepted hit on the segment from `from` to `to`, without collecting hits.
    /// Uses the candidates of `visit_entities_in` in the segment's bounds padded by `margin`.
    /// Each candidate's box is grown by `margin` before clipping. A ray starting inside a
    /// grown box does not hit it, and `exclude` (for example, the shooter) is skipped.
    ///
    /// `accept` runs once per geometric hit in spatial traversal order, not distance order.
    /// It can read the hit entity through the same host. Among the hits it accepts, the
    /// smallest squared distance from `from` wins; an exact tie keeps the first visited hit.
    ///
    /// Returns `Ok(None)` when no hit is accepted. A stale level returns
    /// `LevelReadError::Stale`. A negative or non-finite margin, non-finite endpoints or
    /// padded bounds, an inactive or undisplayed level, or unavailable world access return
    /// `LevelReadError::Unavailable`. A refused query does not call `accept`.
    #[access(WorldOnly)]
    fn level_ray_trace_nearest_entity(
        &self,
        level: LevelRef,
        from: [f64; 3],
        to: [f64; 3],
        margin: f64,
        exclude: Option<EntityRef>,
        accept: &mut dyn FnMut(EntityRayHit) -> bool,
    ) -> Result<Option<EntityRayHit>, LevelReadError>;
}
/// The handles' writes of the `level_entities` area, implemented by the game and reached through [`BackendMut`].
// hidden: a handle's backend, reached through `Backend`; mods call the handle, never this trait
#[doc(hidden)]
#[sunrise_mod_api_macros::host_area]
pub trait LevelEntitiesBackendMut {
    /// Side: server.
    /// Drops `stack` as a broken block drops it (`Block.popResource`): an
    /// item entity at a random point within a quarter block of the middle of
    /// `pos` (three draws from the level random, made for an empty stack and
    /// with `block_drops` off too), with its own random motion and the
    /// default pickup delay of 10 ticks. None, with nothing spawned, for an
    /// empty stack or an unknown item, with the `block_drops` rule off, on a
    /// client, for a stale handle, a level not in place, once the entity ids run out, or an
    /// item a mod kept out (`entity_join_level`).
    /// Inside a transaction its join waits for the root commit, so a mod keeping the item out is not
    /// known at the call: the handle is returned and reads stale from the commit on.
    #[access(WorldOnly)]
    fn level_pop_resource(
        &mut self,
        level: LevelRef,
        pos: [i32; 3],
        stack: ItemStack,
    ) -> Result<Option<EntityRef>, StackWriteError>;
    /// Side: server.
    /// Spawns `stack` as an item entity at `x y z` (`new ItemEntity(level, x,
    /// y, z, stack)` and `addFreshEntity`): its own random motion (up 0.2, up
    /// to 0.1 either way across), with `pickup_delay` ticks before it can be
    /// picked up, as `ItemEntity.setPickupDelay` at construction (vanilla
    /// passes 10 for a block's drop, 40 for a player's throw and 32767 for
    /// never). None, with nothing
    /// spawned, for an empty stack or an unknown item, a value that isn't
    /// finite, on a client, for a stale handle, a level not in place, or an
    /// item a mod kept out (`entity_join_level`).
    /// Inside a transaction its join waits for the root commit, so a mod keeping the item out is not
    /// known at the call: the handle is returned and reads stale from the commit on.
    #[access(WorldOnly)]
    fn level_spawn_item(
        &mut self,
        level: LevelRef,
        x: f64,
        y: f64,
        z: f64,
        stack: ItemStack,
        pickup_delay: u32,
    ) -> Result<Option<EntityRef>, StackWriteError>;
    /// Side: server.
    /// `spawn_item` with the item entity's motion set to
    /// `motion` (blocks a tick), as `new ItemEntity(level, x, y, z, stack,
    /// deltaX, deltaY, deltaZ)`, and the same `pickup_delay` ticks before it
    /// can be picked up (`ItemEntity.setPickupDelay`). None in the same
    /// cases, and for a motion that isn't finite.
    #[access(WorldOnly)]
    fn level_spawn_item_with_motion(
        &mut self,
        level: LevelRef,
        x: f64,
        y: f64,
        z: f64,
        stack: ItemStack,
        motion: [f64; 3],
        pickup_delay: u32,
    ) -> Result<Option<EntityRef>, StackWriteError>;
    /// Side: server.
    /// Awards `amount` experience at `x y z` as vanilla's `ExperienceOrb.award`
    /// does: orbs of the largest sizes first (2477, 1237, ... 3, 1), each
    /// drawing the id `tryMergeToExisting` compares from the level random and
    /// merging into an orb of the same value whose box meets the 1 block box
    /// around `x y z`, or else a new orb with its own random motion. The
    /// `block_drops` rule doesn't apply (vanilla's blocks check it before
    /// they award). True once the orbs are awarded, even if a mod keeps a new
    /// one out (`entity_join_level`). False, with no orbs, for 0, a
    /// value that isn't finite, on a client, for a stale handle or a level
    /// not in place.
    #[access(WorldOnly)]
    fn level_award_experience(
        &mut self,
        level: LevelRef,
        x: f64,
        y: f64,
        z: f64,
        amount: u32,
    ) -> bool;
}
