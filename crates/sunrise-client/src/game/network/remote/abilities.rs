//! Client-side `LocalPlayer.abilities` and `PlayerInfo` game mode sync (`ClientPacketListener`).
use super::*;
use sunrise_sim::lan::abilities::PlayerAbilities;

/// `Abilities.walkingSpeed` before the server sends one.
pub(in crate::game) const WALKING_SPEED: f32 = 0.1;

/// The walking speed `AbstractClientPlayer.getFieldOfViewModifier` divides the movement speed by.
pub(in crate::game) fn walking_speed(abilities: Option<&PlayerAbilities>) -> f32 {
    abilities.map_or(WALKING_SPEED, |a| a.walking_speed)
}

impl Remote {
    /// `AbstractClientPlayer.gameMode()` for a remote player entity in `level_players`.
    pub(in crate::game) fn remote_player_info_game_mode(&self, entity_id: i32) -> Option<i32> {
        if let Some(slot) = self.level_player_infos.get(&entity_id)
            && let Some(cell) = slot.get()
        {
            return Some(cell.load(std::sync::atomic::Ordering::Relaxed));
        }
        let uuid = self.level_players.get(&entity_id)?;
        if let Some(cell) = self.tab_game_modes.get(uuid) {
            if let Some(slot) = self.level_player_infos.get(&entity_id) {
                let _ = slot.set(Arc::clone(cell));
            }
            return Some(cell.load(std::sync::atomic::Ordering::Relaxed));
        }
        self.tab_players.get(uuid).map(|t| t.game_mode)
    }
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// `Abilities.getWalkingSpeed()` of the local player: the server's, on a server.
    pub(in crate::game) fn remote_walking_speed(&self) -> f32 {
        walking_speed(self.remote.as_ref().map(|r| &r.abilities))
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;
    use {sunrise_client::camera::field_of_view_modifier, sunrise_level::camera_math::FovPlayer};

    fn fov(walking: f32, movement: f64) -> f32 {
        let player = FovPlayer {
            flying: false,
            walking_speed: walking,
            movement_speed: movement,
            using: None,
        };
        field_of_view_modifier(&player, true, 1.0)
    }

    #[test]
    fn the_field_of_view_divides_by_the_servers_walking_speed() {
        // A server that sets the walking speed to 0.2 and the movement speed to 0.2: vanilla's
        // getFieldOfViewModifier is (0.2 / 0.2 + 1) / 2 = 1. Kept at 0.1 it would be
        // (0.2 / 0.1 + 1) / 2 = 1.5, a zoomed-out view.
        let abilities = PlayerAbilities {
            walking_speed: 0.2,
            ..PlayerAbilities::default()
        };
        assert_eq!(fov(walking_speed(Some(&abilities)), 0.2), 1.0);
        // Before any packet: vanilla's default 0.1, a still player at the base speed 0.1.
        assert_eq!(walking_speed(None), 0.1);
        assert_eq!(fov(walking_speed(None), 0.1), 1.0);
    }
}

#[cfg(test)]
#[path = "gamemode_split_tests.rs"]
pub(in crate::game) mod gamemode_split_tests;
