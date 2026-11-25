#[allow(unused_imports)]
use super::entity::Entity as _;
use super::{Game, Screen};
use crate::game::server_side::nether_portal::PLAYER_DELAY;
use sunrise_core::random::RandomSource;

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
            self.side
                .audio
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
