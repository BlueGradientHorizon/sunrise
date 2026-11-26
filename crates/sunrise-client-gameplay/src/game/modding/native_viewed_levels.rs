//! A client mod's reads of the levels the server opened for viewing (#3981, gap 10), as Immersive
//! Portals' `ClientWorldLoader.getWorld(dim)` hands out the far `ClientLevel`: on a client,
//! `host.level(dim)` also names each level the server keeps open on `sunrise:levels`, and
//! `block_state`, `brightness`, `clip`, `collision_boxes` and `has_collisions` read its world
//! (`viewed_worlds`), as do the reads built on `level_block_state`: `block`, `block_property`
//! and the both-sided fluid reads (`fluid_shape`, `fluid_height`, `fluid_height_for_camera`,
//! `fluid_can_be_replaced_with`). `get_enchant_power_bonus` uses the viewed level as context.
//! A read never writes and never loads a column: a column the
//! world does not hold is None. The handle answers None from the server's close of the level,
//! and from a Login, a Respawn or a new configuration phase, which drop every viewed world. A
//! `LevelRef` names a level and the client level's epoch, not one open of the view: a close and
//! a later open within the same epoch make the old handle read again, while a Login, a
//! reconfiguration or a Respawn into another level starts a new epoch, so the old handle stays
//! stale and a mod asks `host.level` again (a same-level Respawn keeps the epoch). Writes,
//! server-only reads, the other reads and the entity reads (until step 9d) refuse it.
use crate::game::ClientGame;
use mod_api::{DimensionKey, LevelRef};
use sunrise_level::world::World;

impl ClientGame<'_> {
    /// On a client, the key of the level `dimension` names: the one it shows, or one the server
    /// opened for viewing while that view is open. None before login.
    pub(in crate::game) fn client_level_key(&self, dimension: &str) -> Option<DimensionKey> {
        let remote = self.client.remote.as_ref().filter(|r| r.logged_in)?;
        if remote.level == dimension {
            return Some(remote.level_key);
        }
        remote.viewed.key_of(dimension)
    }
    /// On a client, the world a handle names now: the player's own, or a viewed level's while
    /// the server keeps it open. None before login, for a stale handle and for any other level.
    pub(in crate::game) fn client_view_world(&self, level: LevelRef) -> Option<&World> {
        let remote = self.client.remote.as_ref().filter(|r| r.logged_in)?;
        if self.common.native_lifecycle.epoch() != Some(level.epoch()) {
            return None;
        }
        if level.dimension() == remote.level_key {
            return Some(&self.common.world);
        }
        remote.viewed.world_of(level.dimension())
    }
    /// On a client, the handle the player's own level answers to now (its key and the client
    /// level's epoch): what a world view naming a level is compared with, resolved with the
    /// same rules as `client_view_world`. None before login.
    fn client_own_level(&self) -> Option<LevelRef> {
        let remote = self.client.remote.as_ref().filter(|r| r.logged_in)?;
        Some(LevelRef::new(
            self.common.native_lifecycle.epoch()?,
            remote.level_key,
        ))
    }
    /// The frusta of the frame's camera `[fov_degrees, yaw, pitch]` and its world views, which are
    /// asked for here: each view's level handle is resolved once, first, against the player's own.
    pub(in crate::game::client_side) fn world_view_frustum(
        &mut self,
        camera: glam::DVec3,
        [fov_degrees, yaw, pitch]: [f32; 3],
        bob: glam::Mat4,
        depth_far: f32,
    ) -> Option<sunrise_client::gfx::JoinedFrusta> {
        let own_level = self.client_own_level();
        let renderer = self.client.renderer.0.as_mut()?;
        renderer.set_world_view_own_level(own_level);
        Some(renderer.camera_frustum(camera, fov_degrees, yaw, pitch, bob, depth_far))
    }
    pub(in crate::game) fn client_level_view_world(&self, level: LevelRef) -> Option<&World> {
        self.client_view_world(level)
    }

    pub(in crate::game) fn client_collision_scan_world(
        &self,
        level: LevelRef,
    ) -> Option<(&World, bool)> {
        self.client_view_world(level).map(|w| (w, true))
    }

    pub(in crate::game) fn client_level_handle_current(&self, level: LevelRef) -> bool {
        self.common.native_lifecycle.epoch() == Some(level.epoch()) && level.generation() == 0
    }

    pub(in crate::game) fn client_level_datapack_set(
        &self,
        level: LevelRef,
    ) -> Option<std::sync::Arc<mod_api::DatapackRegistrySet>> {
        if self.common.native_client_level() != Some(level) {
            return None;
        }
        sunrise_sim::datapack_net::received()
    }
}
