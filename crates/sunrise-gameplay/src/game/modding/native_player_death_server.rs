//! Server-side last death location write (`Player.setLastDeathLocation`).
use crate::game::ServerGame;
use mod_api::{GlobalPos, PlayerRef};

impl ServerGame<'_> {
    /// `Player.setLastDeathLocation`: stores where the player died, or
    /// forgets it for None. The client hears of it with the player's next
    /// respawn, as in vanilla. False in the cases a read gives None.
    pub(in crate::game) fn player_set_last_death_location(
        &mut self,
        player: PlayerRef,
        location: Option<GlobalPos>,
    ) -> bool {
        if self.common.player(player.uuid()) != Some(player) {
            return false;
        }
        let stored = location.map(|l| {
            let p = l.pos();
            (l.dimension().to_string(), [p.x, p.y, p.z])
        });
        let mut done = false;
        self.native_as_player(player.uuid(), |game| {
            game.level.last_death = stored;
            done = true;
        }) && done
    }
}
