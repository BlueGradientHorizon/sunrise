use super::*;

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    /// Mirror `Remote::abilities` onto the live `PlayerMotion` body.
    pub(in crate::game::client_side) fn apply_remote_abilities(&mut self) {
        let (Some(player), Some(a)) = (
            self.player.as_mut(),
            self.remote.as_ref().map(|r| &r.abilities),
        ) else {
            return;
        };
        player.invulnerable = a.invulnerable;
        player.flying = a.flying;
        player.may_fly = a.may_fly;
        player.flying_speed = a.flying_speed;
    }
    /// ClientPacketListener.handleRotatePlayer: the server's yaw and pitch, each added to the
    /// current one when its bit says relative, then sent back as a `Rot` move with both
    /// flags false.
    pub(in crate::game) fn remote_rotation(&mut self, p: &play::PlayerRotation) {
        self.yaw = if p.relative_yaw {
            self.yaw + p.yaw
        } else {
            p.yaw
        };
        self.pitch = (if p.relative_pitch {
            self.pitch + p.pitch
        } else {
            p.pitch
        })
        .clamp(-90.0, 90.0);
        self.snap_player_model_pitch();
        self.send(S::MovePlayerRot(play::MovePlayerRot {
            yaw: self.yaw,
            pitch: self.pitch,
            flags: 0,
        }));
    }
    /// Mirror spectator scroll-wheel `flying_speed` back into `Remote::abilities`.
    pub(in crate::game::client_side) fn set_remote_flying_speed(&mut self, speed: f32) {
        if let Some(r) = self.remote.as_mut() {
            r.abilities.flying_speed = speed;
        }
    }
    /// Sync local and remote player spectator modes from `PlayerInfo` and `MultiPlayerGameMode`.
    pub(in crate::game::client_side) fn sync_remote_player_game_mode(&mut self) {
        debug_assert!(self.remote.is_some());
        if self.player.is_some() {
            let info_spectator = self.own_player_is_spectator();
            let local_spectator = self.spectator();
            if let Some(player) = self.player.as_mut() {
                player.spectator = info_spectator;
                player.local_spectator = local_spectator;
            }
        }
        if let Some(r) = self.remote.as_ref() {
            for i in 0..self.mobs.len() {
                let id = self.mobs[i].id as i32;
                if r.level_players.contains_key(&id) {
                    let spectator = r.remote_player_info_game_mode(id) == Some(SPECTATOR);
                    self.mobs.update(i, |m| m.extra.spectator = spectator);
                }
            }
        }
    }
}

impl Remote {
    /// `AbstractClientPlayer.gameMode()`: resolves `PlayerInfo` once per entity lifetime.
    pub(in crate::game::client_side) fn own_player_info_game_mode(&self) -> Option<i32> {
        if let Some(cell) = self.own_player_info.get() {
            return Some(cell.load(std::sync::atomic::Ordering::Relaxed));
        }
        if let Some(cell) = self.tab_game_modes.get(&self.profile) {
            let _ = self.own_player_info.set(Arc::clone(cell));
            return Some(cell.load(std::sync::atomic::Ordering::Relaxed));
        }
        self.tab_players.get(&self.profile).map(|t| t.game_mode)
    }
    /// Copy predicted body flight and scroll speed into `Remote::abilities` before mode updates.
    pub(in crate::game::client_side) fn sync_body_flight(
        &mut self,
        flying: bool,
        flying_speed: f32,
    ) {
        self.abilities.flying = flying;
        self.abilities.flying_speed = flying_speed;
    }
    /// Apply `GameType.updatePlayerAbilities` onto `LocalPlayer.abilities` and mirror `game_mode`.
    pub(in crate::game::client_side) fn update_local_mode_abilities(&mut self, mode: i32) -> bool {
        self.game_mode = mode;
        self.abilities.update_for_mode(mode);
        self.abilities.instabuild
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "gamemode_split_tests_client.rs"]
pub(in crate::game) mod gamemode_split_tests;
