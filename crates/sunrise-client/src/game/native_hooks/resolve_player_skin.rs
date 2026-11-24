#[allow(unused_imports)]
use super::Game;
#[allow(unused_imports)]
use mod_api::{HookId, PlayerModel, PlayerSkinOverride};

impl Game<'_> {
    /// Collect the local player's skin and arm width together, before render extraction.
    pub(in crate::game) fn native_resolve_player_skin(
        &self,
        selection: &mut Option<mod_api::PlayerSkinOverride>,
    ) {
        self.fire_resolve_player_skin(selection);
    }
}

#[cfg(test)]
#[path = "resolve_player_skin/tests.rs"]
pub(in crate::game) mod tests;
