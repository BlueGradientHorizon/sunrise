//! Player food and experience reads for native mods (`PlayerRef`).
use super::Game;
use mod_api::{PlayerExperience, PlayerRef};
use sunrise_sim::entities::Experience;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// `FoodData.needsFood`: the food level is below 20.
    pub(in crate::game) fn player_needs_food(&self, player: PlayerRef) -> Option<bool> {
        Some(self.player_food(player)?.level < 20)
    }

    /// `FoodData.hasEnoughFood`: the food level is above 6, enough to sprint.
    pub(in crate::game) fn player_has_enough_food(&self, player: PlayerRef) -> Option<bool> {
        Some(self.player_food(player)?.level as f32 > 6.0)
    }

    /// `Player.canEat(always)`: an invulnerable player (creative), `always`
    /// (food that can always be eaten) or a food level below 20.
    pub(in crate::game) fn player_can_eat(&self, player: PlayerRef, always: bool) -> Option<bool> {
        let invulnerable = self.player_abilities(player)?.invulnerable;
        Some(invulnerable || always || self.player_needs_food(player)?)
    }

    /// `Player.experienceLevel`, `experienceProgress` and `totalExperience`.
    pub(in crate::game) fn player_experience(&self, player: PlayerRef) -> Option<PlayerExperience> {
        Some(match self.player_read_both(player)? {
            super::native_server::PlayerRead::Guest(g) => {
                PlayerExperience::from(mod_api::__private::built::PlayerExperience {
                    level: g.xp_level,
                    progress: g.xp_progress,
                    total: g.xp_total,
                })
            }
            super::native_server::PlayerRead::Acting { .. } => {
                PlayerExperience::from(mod_api::__private::built::PlayerExperience {
                    level: self.experience.level,
                    progress: self.experience.progress,
                    total: self.experience.total,
                })
            }
        })
    }

    /// `Player.getXpNeededForNextLevel`: the points the current level needs to fill.
    pub(in crate::game) fn player_xp_needed_for_next_level(
        &self,
        player: PlayerRef,
    ) -> Option<i32> {
        Some(Experience::needed(self.player_experience(player)?.level))
    }
}
