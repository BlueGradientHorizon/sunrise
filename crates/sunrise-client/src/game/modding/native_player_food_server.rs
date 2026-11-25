//! Server-side player food and experience writes for native mods.
use super::Game;
use mod_api::PlayerRef;
use sunrise_client::entities::Experience;

impl Game<'_, crate::game::server_side::ServerState> {
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
        self.player.as_mut().map(|p| &mut p.health.food)
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

#[cfg(test)]
mod tests {
    use super::super::native_entity_refs::tests::with_game;
    use super::*;
    use mod_api::{EntityRef, WorldEpoch};
    use sunrise_client::{items::ItemStack, motion::PlayerMotion};
    use sunrise_core::geometry::Vec3;

    /// The acting player with a body, as a hook's turn finds them.
    fn acting_player(game: &mut super::Game<'_>) -> PlayerRef {
        game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
        crate::game::native_entity_refs::tests::bind_local_player(game, 20, 123);
        game.player(123).unwrap()
    }

    #[test]
    #[ignore = "daily"]
    fn points_repair_a_damaged_mending_item_before_experience() {
        with_game(|game| {
            let player = acting_player(game);
            let mut sword = ItemStack::new("minecraft:diamond_sword", 1);
            sword.damage = 1000;
            sword.enchant("minecraft:mending", 1);
            game.inventory[game.selected] = Some(sword);
            assert!(game.player_give_experience_points(player, 10));
            let sword = game.inventory[game.selected].as_ref().unwrap();
            assert!(
                sword.damage < 1000,
                "Mending repaired the item before any point became experience"
            );
            assert_eq!(game.experience.total, 0);
            game.inventory[game.selected] = None;
            assert!(game.player_give_experience_points(player, 10));
            assert_eq!(game.experience.total, 10);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn levels_are_added_taken_and_clamped_at_zero() {
        with_game(|game| {
            let player = acting_player(game);
            game.experience.progress = 0.25;
            assert!(game.player_give_levels(player, 3));
            assert_eq!(game.experience.level, 3);
            assert_eq!(
                game.experience.progress, 0.25,
                "a level add keeps the progress"
            );
            game.experience.total = 40;
            assert!(game.player_give_levels(player, -5));
            assert_eq!(game.experience.level, 0, "below zero clamps to zero");
            assert_eq!(game.experience.total, 0, "the clamp clears the total");
            assert_eq!(
                game.experience.progress, 0.0,
                "the clamp clears the progress"
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn the_total_is_set_alone_and_a_negative_one_is_refused() {
        with_game(|game| {
            let player = acting_player(game);
            game.experience.level = 5;
            game.experience.progress = 0.5;
            assert!(game.player_set_total_experience(player, 1234));
            assert_eq!(game.player_experience(player).unwrap().total, 1234);
            assert_eq!(game.experience.level, 5, "the level is kept");
            assert_eq!(game.experience.progress, 0.5, "the progress is kept");
            assert!(!game.player_set_total_experience(player, -1));
            assert_eq!(game.experience.total, 1234, "a refusal writes nothing");
        });
    }

    #[test]
    #[ignore = "daily"]
    fn more_points_than_an_int_are_refused_untouched() {
        with_game(|game| {
            let player = acting_player(game);
            assert!(game.player_give_experience_points(player, i32::MAX as u32));
            assert_eq!(game.experience.total, i32::MAX);
            let mut sword = ItemStack::new("minecraft:diamond_sword", 1);
            sword.damage = 1000;
            sword.enchant("minecraft:mending", 1);
            game.inventory[game.selected] = Some(sword);
            assert!(!game.player_give_experience_points(player, i32::MAX as u32 + 1));
            let sword = game.inventory[game.selected].as_ref().unwrap();
            assert_eq!(sword.damage, 1000, "a refused give repairs nothing");
            assert_eq!(
                game.experience.total,
                i32::MAX,
                "a refused give gives nothing"
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn a_stale_handle_gives_nothing() {
        with_game(|game| {
            let player = acting_player(game);
            let stale = mod_api::__private::game::player_ref(
                EntityRef::new(WorldEpoch::new(u64::MAX), 7),
                player.uuid(),
            );
            assert!(!game.player_give_experience_points(stale, 10));
            assert!(!game.player_give_levels(stale, 1));
            assert!(!game.player_set_total_experience(stale, 10));
            assert_eq!(game.experience.total, 0);
            assert_eq!(game.experience.level, 0);
        });
    }
}
