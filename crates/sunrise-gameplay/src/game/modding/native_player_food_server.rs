//! Server-side player food and experience writes for native mods.
use crate::game::ServerGame;
use mod_api::PlayerRef;
use sunrise_sim::entities::Experience;

impl ServerGame<'_> {
    /// Whether `player` still names the same player on this server.
    fn native_player_valid(&self, player: PlayerRef) -> bool {
        self.player_handle_read(player).is_some()
    }

    /// Runs `f` with `player`'s state in place, when the handle is current and they are alive.
    fn native_player_write(&mut self, player: PlayerRef, f: impl FnOnce(&mut Self)) -> bool {
        self.native_player_valid(player) && self.native_as_player_handle(player, f)
    }

    /// The acting player's food data, inside `native_player_write`.
    fn native_food_mut(&mut self) -> Option<&mut sunrise_world::food::FoodData> {
        self.common.player.as_mut().map(|p| &mut p.health.food)
    }

    /// `FoodData.eat(nutrition, saturationModifier)`.
    pub(in crate::game) fn player_feed(
        &mut self,
        player: PlayerRef,
        nutrition: i32,
        saturation_modifier: f32,
    ) -> bool {
        self.native_player_write(player, |game| {
            if let Some(food) = game.native_food_mut() {
                food.eat(nutrition, saturation_modifier);
            }
        })
    }

    /// `Player.causeFoodExhaustion`.
    pub(in crate::game) fn player_add_exhaustion(
        &mut self,
        player: PlayerRef,
        amount: f32,
    ) -> bool {
        self.native_player_write(player, |game| {
            if let Some(p) = game.player.as_mut() {
                p.cause_food_exhaustion(amount);
            }
        })
    }

    /// `FoodData.setFoodLevel`.
    pub(in crate::game) fn player_set_food_level(&mut self, player: PlayerRef, level: i32) -> bool {
        self.native_player_write(player, |game| {
            if let Some(food) = game.native_food_mut() {
                food.food = level;
            }
        })
    }

    /// `FoodData.setSaturation`.
    pub(in crate::game) fn player_set_saturation(
        &mut self,
        player: PlayerRef,
        saturation: f32,
    ) -> bool {
        self.native_player_write(player, |game| {
            if let Some(food) = game.native_food_mut() {
                food.saturation = saturation;
            }
        })
    }

    /// Sets the exhaustion as it is (Paper's `HumanEntity#setExhaustion`).
    pub(in crate::game) fn player_set_exhaustion(
        &mut self,
        player: PlayerRef,
        exhaustion: f32,
    ) -> bool {
        self.native_player_write(player, |game| {
            if let Some(food) = game.native_food_mut() {
                food.exhaustion = exhaustion;
            }
        })
    }

    /// `ServerPlayer.setExperiencePoints`.
    pub(in crate::game) fn player_set_experience_points(
        &mut self,
        player: PlayerRef,
        points: i32,
    ) -> bool {
        self.native_player_write(player, |game| {
            let limit = Experience::needed(game.experience.level) as f32;
            let max = (limit - 1.0) / limit;
            // Mth.clamp; `limit` is at least 7, so nothing here is NaN.
            let progress = points as f32 / limit;
            game.experience.progress = if progress < 0.0 {
                0.0
            } else {
                progress.min(max)
            };
        })
    }

    /// `ServerPlayer.setExperienceLevels`.
    pub(in crate::game) fn player_set_experience_levels(
        &mut self,
        player: PlayerRef,
        levels: i32,
    ) -> bool {
        self.native_player_write(player, |game| game.experience.level = levels)
    }

    /// `Player.totalExperience`.
    pub(in crate::game) fn player_set_total_experience(
        &mut self,
        player: PlayerRef,
        total: i32,
    ) -> bool {
        total >= 0 && self.native_player_write(player, |game| game.experience.total = total)
    }

    /// `ExperienceOrb.playerTouch`: Mending first, then `Player.giveExperiencePoints`.
    pub(in crate::game) fn player_give_experience_points(
        &mut self,
        player: PlayerRef,
        points: u32,
    ) -> bool {
        let Ok(points) = i32::try_from(points) else {
            return false;
        };
        self.native_player_write(player, |game| {
            let rest = if points > 0 {
                game.mending(points as u32)
            } else {
                0
            };
            if rest > 0 {
                game.give_experience_points(rest as i32);
            }
        })
    }

    /// `Player.giveExperienceLevels`.
    pub(in crate::game) fn player_give_levels(&mut self, player: PlayerRef, levels: i32) -> bool {
        self.native_player_write(player, |game| game.give_levels(levels))
    }
}
