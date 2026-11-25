#[allow(unused_imports)]
pub(in crate::game::client_side) use super::entity::{
    Entity as _, entity_lists_of, for_each_entity,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Dim, Game, Result, Screen};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::nether_portal::Arrival;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::nether_portal::PLAYER_DELAY;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::nether_portal::PROJECTILE_DELAY;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::nether_portal::Plan;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::nether_portal::PortalSide;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::nether_portal::Portals;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::nether_portal::TransferredArrivalState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::nether_portal::VEHICLE_DELAY;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::nether_portal::end_portal_rotation;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::nether_portal::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::HashMap;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::fast_hash::FxHashMap;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::{Species, spawn_rules};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::nether_portal::{ENTITY_DELAY, PortalState};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::world::PendingEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Axis, Vec3},
    position::BlockPos,
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::level::flags as level_flags;
#[allow(unused_imports)]
pub(in crate::game::client_side) use {
    sunrise_core::nbt::Tag,
    sunrise_world::level::portal_travel::{
        BorderBounds, FoundRectangle, closest_portal, collision_free_position, exit_position,
        largest_rectangle_around, relative_position,
    },
};

#[path = "nether_portal/respawn_attributes_client.rs"]
pub(in crate::game) mod respawn_attributes;

impl Game<'_, crate::game::client_side::ClientState> {
    /// Whether the inventory key opens nothing while standing in a nether portal.
    pub(in crate::game) fn in_portal_transition(&self) -> bool {
        self.portals.client_was_inside
    }

    /// Client-side `setAsInsidePortal`, driving `client_portal_effect`.
    pub(in crate::game) fn client_inside_nether_portal(&mut self) {
        if !self.player_can_use_portal() {
            return;
        }
        if self.portals.player.cooldown > 0 {
            self.portals.player.cooldown = PLAYER_DELAY;
        } else {
            self.portals.client_inside = true;
        }
    }

    /// `LocalPlayer.handlePortalTransitionEffect`: whether the confusion
    /// effect grows this tick. Entering a portal closes the screen only when
    /// it does not allow it (`Screen.isAllowedInPortal`), and plays
    /// `block.portal.trigger` when the effect starts from nothing.
    pub(in crate::game::client_side) fn client_portal_effect(&mut self) -> bool {
        let inside = std::mem::take(&mut self.portals.client_inside);
        self.portals.client_was_inside = inside;
        if !inside {
            return false;
        }
        // No screen at all (`screen() == null`) is nothing to close.
        if self.side.screen != Screen::Playing && !self.side.screen.allowed_in_portal() {
            if matches!(self.side.screen, Screen::Creative | Screen::Inventory) {
                self.close_container_client();
                self.carried = None;
            }
            self.set_screen(Screen::Playing);
            self.capture(true);
        }
        if self.side.screen_fx.portal == 0.0 {
            let pitch = self.world.random.next_float() * 0.4 + 0.8;
            self.side.audio
                .0
                .play("minecraft:block.portal.trigger", None, 0.25, pitch);
        }
        true
    }
}

#[cfg(test)]
#[path = "tests/portal_spawn_tests_client.rs"]
pub(in crate::game) mod portal_spawn_tests;

#[cfg(test)]
#[path = "tests/nether_portal_stand_tests_client.rs"]
pub(in crate::game) mod stand_tests;
