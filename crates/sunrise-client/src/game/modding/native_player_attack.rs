//! A player's attack strength for native server mods, through a `PlayerRef`:
//! `Player.getAttackStrengthScale(partial)`, the charge `Player.attack` reads
//! at its start (`attack_entity` fires before that, and before the swing
//! resets the ticker). Reads return None for a stale handle or a player who
//! isn't online here; a resting guest reads its saved body.
use super::Game;
use super::native_server::PlayerRead;
use mod_api::PlayerRef;
use sunrise_client::attack_strength::{BASE_ATTACK_SPEED, delay};

impl Game<'_> {
    /// `Player.getAttackStrengthScale(partial)` for `player`: a server's player, or on a client
    /// the local player alone (#5691).
    /// Returns `None` for a non-finite partial tick; finite partials are not restricted.
    pub(in crate::game) fn player_attack_strength_scale(
        &self,
        player: PlayerRef,
        partial: f32,
    ) -> Option<f32> {
        if !partial.is_finite() {
            return None;
        }
        Some(match self.player_read_both(player)? {
            PlayerRead::Acting { .. } => return self.attack_strength_scale_at(partial),
            PlayerRead::Guest(guest) => {
                let (ticks, speed) = self.guest_attack_charge(guest)?;
                ((ticks as f32 + partial) / delay(speed)).clamp(0.0, 1.0)
            }
        })
    }

    /// `Player.getCurrentItemAttackStrengthDelay()` for `player`: a server's player, or on a
    /// client the local player alone (#5691).
    pub(in crate::game) fn player_current_item_attack_strength_delay(
        &self,
        player: PlayerRef,
    ) -> Option<f32> {
        Some(delay(match self.player_read_both(player)? {
            PlayerRead::Acting { .. } => self.main_hand_attack_speed(),
            PlayerRead::Guest(guest) => self.guest_attack_charge(guest)?.1,
        }))
    }

    /// A guest between turns: its attack ticker and its attack speed with the main-hand item of
    /// its last tick.
    fn guest_attack_charge(&self, guest: &sunrise_client::lan::guest::Guest) -> Option<(u32, f64)> {
        let (ticks, item) = self
            .lan
            .as_ref()?
            .play
            .get(&guest.conn)
            .map_or((0, None), |play| (play.attack_ticks(), play.attack_item()));
        Some((ticks, guest_attack_speed(self, guest, item)))
    }
}

/// `Game.main_hand_attack_speed` for a guest between turns: its saved body's
/// attack speed base, the main-hand item of its last tick, and its effects'
/// modifiers.
fn guest_attack_speed(
    game: &Game<'_>,
    guest: &sunrise_client::lan::guest::Guest,
    item: Option<&str>,
) -> f64 {
    let base = guest
        .motion
        .as_ref()
        .and_then(|player| {
            let key = player.attributes.key("minecraft:attack_speed")?;
            let handle = player.attributes.lookup(Some(&key))?;
            player
                .attributes
                .instance(&handle)
                .ok()
                .map(|value| value.base_value())
        })
        .unwrap_or(BASE_ATTACK_SPEED);
    let value = base
        + item
            .and_then(|id| game.items().get(id))
            .map_or(0.0, |info| f64::from(info.attack_speed));
    game.sanitize_attack_speed(
        guest
            .effects
            .modifiers()
            .into_iter()
            .filter(|modifier| modifier.0 == "minecraft:attack_speed")
            .fold(value, |value, modifier| value * (1.0 + modifier.2)),
    )
}

#[cfg(test)]
mod tests {
    /// A client reads its own player's attack charge and full-charge delay from its own ticker
    /// and main-hand attack speed, as the crosshair's attack indicator does; any other player
    /// reads None (#5691).
    #[test]
    #[ignore = "daily"]
    fn a_client_reads_its_own_players_attack_charge_and_delay() {
        use super::super::native_client_session::tests::logged_client;
        use super::super::native_entity_refs::tests::with_client_game;
        use sunrise_client::motion::PlayerMotion;
        use sunrise_core::geometry::Vec3;
        mod_platform::testing::with_mods(Default::default(), || {
            with_client_game(|game| {
                let _server = logged_client(game);
                game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
                let local = game.local_player().unwrap();
                let other = mod_api::__private::game::player_ref(
                    mod_api::EntityRef::new(local.entity().epoch(), 99),
                    99,
                );
                let delay = sunrise_client::attack_strength::delay(game.main_hand_attack_speed());
                assert!(delay.is_finite() && delay > 0.0, "{delay}");
                for ticks in [0, 2, 40] {
                    game.attack_ticks = ticks;
                    let host = &*game as &dyn mod_api::ClientHost;
                    assert_eq!(local.current_item_attack_strength_delay(host), Some(delay));
                    assert_eq!(
                        local.attack_strength_scale(host, 0.0),
                        Some((ticks as f32 / delay).clamp(0.0, 1.0))
                    );
                    assert_eq!(other.current_item_attack_strength_delay(host), None);
                    assert_eq!(other.attack_strength_scale(host, 0.0), None);
                }
                let host = &*game as &dyn mod_api::ClientHost;
                assert_eq!(local.attack_strength_scale(host, 0.0), Some(1.0));
                assert_eq!(local.attack_strength_scale(host, f32::NAN), None);
            });
        });
    }
}
