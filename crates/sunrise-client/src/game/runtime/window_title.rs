//! `Minecraft.createTitle` and `updateTitle`: the window's title, as vanilla's reads.
use super::{Game, Screen};

/// `Minecraft* 26.3`: `ModCheck` reports a client that isn't Mojang's signed jar as modified.
pub(in crate::game) fn base() -> String {
    format!("Minecraft* {}", sunrise_core::VANILLA_VERSION)
}

/// The full title for a session part (`title.singleplayer`...), none while not connected.
fn title(session: Option<&str>) -> String {
    match session {
        Some(key) => format!("{} - {}", base(), sunrise_game_data::lang::get(key)),
        None => base(),
    }
}

impl Game<'_, crate::game::client_side::ClientState> {
    /// The session part `createTitle` adds while `getConnection` has a player and its connection is open.
    fn title_session(&self) -> Option<&'static str> {
        let live = self
            .side
            .remote
            .as_ref()
            .is_some_and(|r| r.session.connected());
        if !live || self.player.is_none() {
            return None;
        }
        Some(if self.side.integrated.is_some() && !self.is_published() {
            "title.singleplayer"
        } else if self.side.realm_session {
            "title.multiplayer.realms"
        } else if self.side.integrated.is_none() && !self.side.lan_session {
            "title.multiplayer.other"
        } else {
            "title.multiplayer.lan"
        })
    }

    /// `updateTitle`: on a session change (level, publishing), and a new language once back to play.
    pub(in crate::game::client_side) fn update_window_title(&mut self) {
        let language = match self.side.window_title {
            Some((_, shown)) if self.side.screen != Screen::Playing => shown,
            _ => sunrise_game_data::lang::generation(),
        };
        let shown = (self.title_session(), language);
        if self.side.window_title == Some(shown) {
            return;
        }
        if let Some(window) = &self.side.window.0 {
            window.set_title(&title(shown.0));
            self.side.window_title = Some(shown);
        }
    }
}
