//! The `level_light` area: stored and sky light, and relighting. Host method declarations; access defaults and game forwarders are generated.
#![allow(unused_variables)]
use crate::*;
/// The handles' reads of the `level_light` area, implemented by the game and reached through [`Backend`].
// hidden: a handle's backend, reached through `Backend`; mods call the handle, never this trait
#[doc(hidden)]
#[sunrise_mod_api_macros::host_area]
pub trait LevelLightBackend {
    /// Side: both.
    /// `Level.getSkyDarken`: the sky-light reduction from dimension, time and weather.
    /// Reads any loaded server level, including parked levels, or the current client level.
    /// None for a stale or unavailable level. Does not load chunks.
    #[access(WorldOnly)]
    fn level_get_sky_darken(&self, level: LevelRef) -> Option<i32>;
    /// Side: both.
    /// The stored light of one layer at `pos` (`BlockAndLightGetter.getBrightness`), 0 to 15.
    /// None as for `block_state`: a position that isn't loaded has no light to read. On a
    /// client, the light of the level it shows, as the server sent it for a chunk it holds.
    #[access(WorldOnly)]
    fn level_brightness(&self, level: LevelRef, layer: LightLayer, pos: [i32; 3]) -> Option<i32>;
    /// Side: both.
    /// `BlockAndLightGetter.canSeeSky`: whether the level's published sky light at `pos` is 15.
    /// Works in an open server level or the current client level; stale or unavailable levels
    /// return None. Never loads chunks. See `LevelHost::can_see_sky` for the light storage rules
    /// at its boundaries.
    #[access(WorldOnly)]
    fn level_can_see_sky(&self, level: LevelRef, pos: [i32; 3]) -> Option<bool>;
}
/// The handles' writes of the `level_light` area, implemented by the game and reached through [`BackendMut`].
// hidden: a handle's backend, reached through `Backend`; mods call the handle, never this trait
#[doc(hidden)]
#[sunrise_mod_api_macros::host_area]
pub trait LevelLightBackendMut {
    /// Side: server.
    /// Check the block light at `pos` again (`LevelLightEngine.checkBlock`), at the level's
    /// next light update, as an edit there is checked; checking the same position again before
    /// then queues nothing. A block declaring `dynamic_light` is asked `light_emission` again
    /// first: the light engine never asks on its own, so a part whose answer reads more than
    /// the block's state (the block below, the depth) calls this when that changes. Clients take the new light from the server's light update and ask
    /// their own parts again. False, with nothing queued, off the level in place, for a stale
    /// handle, on a client and for a position that isn't loaded.
    #[access(WorldOnly)]
    fn level_check_light(&mut self, level: LevelRef, pos: [i32; 3]) -> bool;
    /// Side: server.
    /// Queues the loaded chunks `chunks` (chunk coordinates) of any open server level (active or
    /// parked), and the loaded chunks around them, to be lit again whole from their blocks, for a
    /// mod that wrote blocks by other means (`edit_blocks` queues the same relight for its own
    /// chunks). Accepted at once and never lit during the call: the owning level's light engine
    /// schedules and batches the queued columns across its light updates (matching vanilla
    /// `ThreadedLevelLightEngine`), and a column whose eight neighbours are not all loaded waits
    /// until they are. Re-queuing a column whose earlier light job is still in flight invalidates
    /// the stale result so the column is lit from its current blocks. The light that changed goes
    /// to the players watching that chunk's level, as any relight's does. There is no completion
    /// event and no per-call count of finished chunks: a mod that needs the new light reads it on
    /// a later tick (`level_brightness`), and `chunk_resend` sends the chunk with whatever light
    /// has landed when its level broadcasts changes. Returns how many columns were newly queued,
    /// the ring included; a column not loaded, or already waiting in the queue, is not counted.
    /// Each refusal queues nothing and is named by `ChunkRefusal`: `Stale` (stale handle or
    /// unloaded level) and `Client`.
    #[access(WorldOnly)]
    fn level_relight_chunks(
        &mut self,
        level: LevelRef,
        chunks: &[[i32; 2]],
    ) -> Result<u32, ChunkRefusal>;
}
