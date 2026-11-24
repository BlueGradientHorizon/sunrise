#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, chat::PLAYER_NAME, commands::Target};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::GameEnv;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::Recorded;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::ScoreState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::feedback_name;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::gives_uuid;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::in_range;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::local_tab_players;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::pet_team;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::root_owner;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::tab_players_needed;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::tame_owner;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::trigger_objectives_for;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::EntityTags;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::{HashMap, HashSet};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    command::{
        Context,
        args::{IntRange, Selector},
    },
    hud::HudBatch,
    hud_layout, hud_overlays,
    lan::{guest::Guest, playerdata::ints_uuid},
    save::{self, uuid_from_text, uuid_text},
    scoreboard::{self as sb, Comp, Scoreboard, literal},
    scoreboard_cmd::{self, Env, Holder},
    tab_list::TabPlayer,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::stats::{StatScoreChange, StatScoreOp};

impl Game<'_> {
    /// `Hud.extractTabList`: the player list while the player-list key is
    /// held. A local world shows it with a joined LAN guest or a `list`
    /// objective.
    pub(in crate::game) fn draw_tab_list(&mut self, batch: &mut HudBatch) {
        use sunrise_client::tab_list::{self, TabInputs};
        // Player heads and the player list both draw from the same skin
        // slots: a new frame, with what the last one drew.
        self.flush_skin_slots();
        let key =
            self.client.screen == super::Screen::Playing && self.held_mapping(Action::Playerlist);
        let count = self.remote.as_ref().map_or_else(
            || {
                1 + self
                    .lan
                    .as_ref()
                    .map_or(0, |h| h.hub.guests.iter().filter(|g| g.placed).count())
            },
            |r| r.listed_player_count(),
        );
        let has_objective = self.score.board.display_objective(sb::SLOT_LIST).is_some();
        let shown = tab_list::visible(
            // `Hud.extractRenderState` draws the list inside `if (!isHidden)`.
            !self.ui.hide_gui,
            key,
            self.has_singleplayer_server(),
            count,
            has_objective,
        );
        let changed = self.tab_list.0.set_visible(shown);
        if !tab_players_needed(shown, changed) {
            // World skins still need download polling and atlas residency checks.
            self.update_remote_skins();
            self.publish_tab_object_skins();
            return;
        }
        let (mut players, online_mode, upside_down) = match self.remote_tab_players() {
            Some(remote) => remote,
            None => {
                // The integrated server: the local player, online mode, no ping.
                let uuid = self.level.player_uuid.map_or(0, |u| {
                    u.iter()
                        .fold(0u128, |a, &w| (a << 32) | u128::from(w as u32))
                });
                let mut me = TabPlayer::new(uuid, PLAYER_NAME);
                me.game_mode = self.mode_id();
                me.skin = "entity/player/wide/steve".into();
                let guests = self.lan.as_ref().map_or(&[][..], |h| &h.hub.guests);
                let players = local_tab_players(me, guests);
                let upside_down = players
                    .iter()
                    .filter(|p| p.name == "Dinnerbone" || p.name == "Grumm")
                    .map(|p| p.uuid)
                    .collect();
                (players, true, upside_down)
            }
        };
        if changed {
            // PlayerTabOverlay.setVisible: the names, said as the list comes up.
            self.say_system_now(&tab_list::narration(&players, &self.score.board));
        }
        if !shown {
            return;
        }
        // The faces of the players the list shows.
        let faces: std::collections::HashSet<u128> = self
            .tab_list
            .0
            .displayed_ids(&players, &self.score.board)
            .iter()
            .copied()
            .collect();
        for p in players.iter_mut().filter(|p| faces.contains(&p.uuid)) {
            p.skin = self.face_skin(p.uuid, &p.skin);
        }
        let board = &self.score.board;
        let objective = board.display_objective(sb::SLOT_LIST);
        let up = |id: u128| upside_down.contains(&id);
        let inputs = TabInputs {
            players: &players,
            board,
            objective,
            screen_width: batch.width as i32,
            online_mode,
            gui_ticks: self.visual_ticks() as i64,
            // Sunrise has no "chat background only" option: vanilla's default (on).
            background: tab_list::row_background(true, self.settings.text_background_opacity),
            upside_down: &up,
        };
        let draws = self.tab_list.0.layout(&inputs, &self.gui.atlas.font);
        sunrise_client::hud_layout::paint(batch, &self.gui.atlas, &draws);
    }
    /// Advance the skin slots to a new frame with what the last one drew, and
    /// store any finished skin downloads into the atlas.
    pub(in crate::game) fn flush_skin_slots(&mut self) {
        let (ready, drawn) = {
            let mut downloads = self.skin_downloads.0.borrow_mut();
            (downloads.take_ready(), downloads.take_drawn())
        };
        self.client.skin_slots.next_frame(drawn);
        for (location, rgba) in ready {
            self.store_skin(location, rgba);
        }
    }
}
