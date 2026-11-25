#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, chat::PLAYER_NAME, commands::Target};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::GameEnv;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::Recorded;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::ScoreState;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::local_tab_players;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::scoreboard::tab_players_needed;
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
    /// `Hud.extractScoreboardSidebar` / `displayScoreboardSidebar`.
    pub(in crate::game) fn draw_scoreboard(&self, batch: &mut HudBatch) {
        let board = &self.score.board;
        let Some(objective) = self.sidebar_objective() else {
            return;
        };
        let draws = hud_overlays::sidebar(
            batch.width as i32,
            batch.height as i32,
            board,
            objective,
            &self.gui.atlas.font,
            self.settings.text_background(0.3),
            self.settings.text_background(0.4),
        );
        hud_layout::paint(batch, &self.gui.atlas, &draws);
    }
    /// `Hud.extractTabList`: the player list while the player-list key is
    /// held. A local world shows it with a joined LAN guest or a `list`
    /// objective.
    pub(in crate::game::client_side) fn draw_tab_list(&mut self, batch: &mut HudBatch) {
        use sunrise_client::tab_list::{self, TabInputs};
        // Player heads and the player list both draw from the same skin
        // slots: a new frame, with what the last one drew.
        self.flush_skin_slots();
        let key =
            self.client.screen == super::Screen::Playing && self.held_mapping(Action::Playerlist);
        let Some(count) = self.remote.as_ref().map(|r| r.listed_player_count()) else {
            self.tab_list.0.set_visible(false);
            return;
        };
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
        let Some((mut players, online_mode, upside_down)) = self.remote_tab_players() else {
            return;
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
    pub(in crate::game::client_side) fn flush_skin_slots(&mut self) {
        let (ready, drawn) = {
            let mut downloads = self.skin_downloads.0.borrow_mut();
            (downloads.take_ready(), downloads.take_drawn())
        };
        self.client.skin_slots.next_frame(drawn);
        for (location, rgba) in ready {
            self.store_skin(location, rgba);
        }
    }
    /// The objective in the player's team's sidebar slot, else the sidebar's.
    pub(in crate::game) fn sidebar_objective(&self) -> Option<&sb::Objective> {
        let board = &self.score.board;
        if board.display.is_empty() {
            return None;
        }
        // A server's client is in its team by its profile name.
        let name = self
            .remote
            .as_ref()
            .map_or(PLAYER_NAME, |r| r.profile_name.as_str());
        let team_slot = board
            .players_team(name)
            .and_then(|t| t.color.as_ref())
            .map(|c| format!("sidebar.team.{c}"))
            .and_then(|slot| board.display_objective(&slot));
        team_slot.or_else(|| board.display_objective(sb::SLOT_SIDEBAR))
    }
    /// The sidebar the HUD shows: its objective and its lines, for `status`.
    pub(in crate::game::client_side) fn sidebar_status(&self) -> serde_json::Value {
        let board = &self.score.board;
        let Some(objective) = self.sidebar_objective() else {
            return serde_json::Value::Null;
        };
        let scores: serde_json::Map<String, serde_json::Value> = board
            .scores
            .iter()
            .filter_map(|(holder, s)| Some((holder.clone(), s.get(&objective.name)?.value.into())))
            .collect();
        serde_json::json!({ "objective": objective.name, "title": objective.display_text().plain(), "scores": scores })
    }
    /// Client-side viewer team from `Remote::profile_name` (never falls back to server `PLAYER_NAME`).
    pub(in crate::game) fn client_player_team(&self) -> Option<&sb::Team> {
        self.own_team()
    }
    /// Client-side `TamableAnimal.getTeam` of an entity id (other players by profile name, mobs by UUID).
    pub(in crate::game) fn client_mob_team(&self, id: u32) -> Option<&sb::Team> {
        if self.score.board.teams.is_empty() {
            return None;
        }
        self.client_team_with_owner(id, || {
            self.mobs.iter().find(|m| m.id == id).and_then(tame_owner)
        })
    }
    /// Client-side [`client_mob_team`](Self::client_mob_team) for a mob already in hand.
    pub(in crate::game) fn client_mob_team_of(
        &self,
        m: &sunrise_client::mobs::Mob,
    ) -> Option<&sb::Team> {
        if self.score.board.teams.is_empty() {
            return None;
        }
        self.client_team_with_owner(m.id, || tame_owner(m))
    }
    /// Client-side team snapshot for `MobWorld` (viewer, remote players, and client-tracked mobs).
    pub(in crate::game) fn client_mob_teams(&self) -> sunrise_client::mobs::Teams<'_> {
        if self.score.board.teams.is_empty() {
            return Default::default();
        }
        let tame_owners: HashMap<u32, [i32; 4]> = self
            .mobs
            .iter()
            .filter_map(|m| Some((m.id, tame_owner(m)?)))
            .collect();
        let own = self
            .remote
            .as_ref()
            .and_then(|r| Some((u32::try_from(r.entity_id).ok()?, self.client_player_team()?)));
        let others = self
            .remote_player_names()
            .into_iter()
            .filter_map(|(id, name)| Some((id, self.score.board.players_team(&name)?)));
        let players: HashMap<u32, &sb::Team> = own.into_iter().chain(others).collect();
        sunrise_client::mobs::Teams {
            player: self.client_player_team(),
            players,
            mobs: self
                .persist
                .uuids
                .keys()
                .filter_map(|id| {
                    let owner = || tame_owners.get(id).copied();
                    Some((*id, self.client_team_with_owner(*id, owner)?))
                })
                .collect(),
        }
    }
    fn client_team_with_owner(
        &self,
        id: u32,
        owner: impl FnOnce() -> Option<[i32; 4]>,
    ) -> Option<&sb::Team> {
        if let Some(name) = self.remote_player_name(id) {
            return self.score.board.players_team(name);
        }
        let uuid = *self.persist.uuids.get(&id)?;
        let own = uuid_text(uuid);
        if let Some(team) = self.score.board.players_team(&own) {
            return Some(team);
        }
        let owner = owner().and_then(|o| self.client_root_owner_name(o));
        pet_team(&self.score.board, &own, owner.as_deref())
    }
    fn client_root_owner_name(&self, owner: [i32; 4]) -> Option<String> {
        let pet_owner = |uuid: [i32; 4]| {
            let pet = self
                .remote
                .as_ref()
                .and_then(|r| r.level_uuids.get(&ints_uuid(uuid)).copied())
                .and_then(|id| u32::try_from(id).ok())
                .or_else(|| {
                    self.persist
                        .uuids
                        .iter()
                        .find_map(|(&id, &u)| (u == uuid).then_some(id))
                })?;
            self.mobs
                .iter()
                .find(|m| m.id == pet && m.extra.tame)?
                .extra
                .owner
        };
        let root = root_owner(owner, pet_owner);
        let root_u128 = ints_uuid(root);
        if let Some(r) = self.remote.as_ref() {
            if r.profile == root_u128 {
                return Some(r.profile_name.clone());
            }
            if let Some((&id, _)) = r.level_players.iter().find(|(_, u)| **u == root_u128) {
                return Some(
                    r.level_player_name(id, &root_u128)
                        .map_or_else(|| uuid_text(root), str::to_owned),
                );
            }
            if r.level_uuids.contains_key(&root_u128) {
                return Some(uuid_text(root));
            }
        }
        self.persist
            .uuids
            .values()
            .any(|&u| u == root)
            .then(|| uuid_text(root))
    }
}
