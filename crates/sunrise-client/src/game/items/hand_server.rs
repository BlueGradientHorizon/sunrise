#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Mining, minecarts::CartKind};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::BreakSpeedDispatch;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::GameHands;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::HarvestDispatch;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::OFFHAND;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::interact_location;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::matches_for_swap;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    Result,
    attack_strength::HeldStack,
    first_person::{Hand, HandWorld, Hit, SwingAnimation, UseResult, swing_duration},
    items::ItemStack,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::{Aabb, Vec3};

impl Game<'_> {
    /// The item whose attack damage and speed modifiers the player has (`attack_item`).
    pub(in crate::game::server_side) fn attack_item_info(
        &self,
    ) -> Option<&sunrise_client::items::ItemInfo> {
        self.items().get(self.server.attack_item.as_deref()?)
    }
    /// `Attributes.ATTACK_SPEED` with the main-hand item's modifier and the
    /// Haste and Mining Fatigue modifiers (`ADD_MULTIPLIED_TOTAL`), clamped to
    /// the attribute's range.
    pub(in crate::game) fn main_hand_attack_speed(&self) -> f64 {
        if let Some(player) = &self.player
            && let Ok(speed) = player
                .attributes
                .computed_value_of("minecraft:attack_speed")
            && (self.remote.is_some()
                || !self.server.applied_modifiers.is_empty()
                || self.potions.effects.get("haste").is_some()
                || self.potions.effects.get("mining_fatigue").is_some())
        {
            // The computed value is already inside the attribute's bounds.
            return speed;
        }
        let base = self
            .player
            .as_ref()
            .and_then(|player| {
                player
                    .attributes
                    .computed_value_of("minecraft:attack_speed")
                    .ok()
            })
            .unwrap_or(sunrise_client::attack_strength::BASE_ATTACK_SPEED);
        let value = base
            + self
                .attack_item_info()
                .map_or(0.0, |i| f64::from(i.attack_speed));
        let value = self
            .potions
            .effects
            .modifiers()
            .into_iter()
            .filter(|m| m.0 == "minecraft:attack_speed")
            .fold(value, |v, m| v * (1.0 + m.2));
        self.sanitize_attack_speed(value)
    }
    /// The window tick already picked before key handling. Simulation keeps its own ray.
    pub(in crate::game) fn pick_tick_targets(&mut self) {
        if self.native_window() {
            return;
        }
        self.server.tick_hit.set(None);
        self.server.tick_mob = None;
        match self.compute_crosshair_decision() {
            super::pick_entity::CrosshairDecision::Block(hit) => {
                self.server.tick_hit.set(Some(Some(hit)));
                self.server.tick_mob = Some(None);
            }
            super::pick_entity::CrosshairDecision::Entity(hit) => {
                self.server.tick_hit.set(Some(None));
                let mob = self.mob_index.find(&self.mobs, hit.id).map(|i| {
                    let feet = self.mobs[i].position;
                    (
                        i,
                        hit.distance_sq.sqrt(),
                        [
                            hit.point[0] - feet.x,
                            hit.point[1] - feet.y,
                            hit.point[2] - feet.z,
                        ],
                    )
                });
                self.server.tick_mob = Some(mob);
            }
            super::pick_entity::CrosshairDecision::Miss => {
                self.server.tick_hit.set(Some(None));
                self.server.tick_mob = Some(None);
            }
        }
    }
    /// Release only the non-window simulation's tick-local caches.
    pub(in crate::game) fn release_tick_targets(&mut self) {
        self.server.tick_hit.set(None);
        self.server.tick_mob = None;
    }
    /// `Player.resetAttackStrengthTicker`.
    pub(in crate::game) fn reset_attack_strength(&mut self) {
        self.server.attack_ticks = 0;
        self.reset_attack_strength_client();
    }
    /// `swing_arm` for a use whose result is `SwingSource.PREDICTED`: the
    /// player's client swings by itself, so a server keeps it to the others.
    pub(in crate::game::server_side) fn swing_arm_predicted(&mut self) {
        let before = self.swings_started;
        self.swing_arm();
        if self.swings_started != before {
            self.server.predicted_swings += 1;
        }
    }
    #[inline(always)]
    pub(in crate::game) fn tick_hand_server(
        &mut self,
        reset: bool,
        changed: bool,
        main: Option<&ItemStack>,
    ) {
        if reset {
            self.server.attack_ticks = 0;
        }
        if changed {
            self.apply_equipment();
        }
        self.server.attack_item = main.map(|s| s.item.clone());
    }
}
