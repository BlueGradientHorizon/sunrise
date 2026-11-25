#[allow(unused_imports)]
use super::*;

#[allow(unused_imports)]
use sunrise_protocol::packets::configuration::{ClientInformation, Transfer};

/// Connection already started; its eventual packets still belong to the next listener.
pub(in crate::game) struct RemoteOpening {
    pub(in crate::game) session: Session,
    pub(in crate::game) builtin: Arc<Builtin>,
    pub(in crate::game) lang: Arc<Language>,
    pub(in crate::game) info: ClientInformation,
    pub(in crate::game) user: (String, u128),
    pub(in crate::game) local: bool,
    pub(in crate::game) list_name: Option<String>,
}

impl Game<'_> {
    pub(in crate::game) fn connect_remote_session(
        &mut self,
        address: &str,
        name: &str,
        local: Option<sunrise_protocol::connection::Pipe>,
        cookies: Option<sunrise_client::net::Cookies>,
        lan: bool,
        realm: bool,
        list_name: Option<&str>,
    ) -> Result<()> {
        let opening =
            self.open_remote_session(address, name, local, cookies, lan, true, list_name)?;
        self.install_remote_session(opening, lan, realm)
    }

    /// `handleTransfer`: close the old connection and start the new one now.
    /// Keep the old listener installed for the remaining handlers of this bundle.
    pub(in crate::game) fn start_remote_transfer(
        &mut self,
        transfer: Transfer,
    ) -> Result<Option<RemoteOpening>> {
        let Some(remote) = self.remote.as_ref() else {
            return Ok(None);
        };
        remote.session.close_after_pending();
        if remote.session.address == "local" {
            return Ok(None);
        }
        let address = format!("{}:{}", transfer.host, transfer.port);
        let name = remote.session.name.clone();
        let cookies = remote.session.cookies.clone();
        self.open_remote_session(&address, &name, None, Some(cookies), false, false, None)
            .map(Some)
    }

    pub(in crate::game) fn install_remote_transfer(
        &mut self,
        opening: RemoteOpening,
    ) -> Result<()> {
        self.connection_left(sunrise_hooks::connection::Left::Transferred);
        self.leave_server(None)?;
        self.connection_starts(&opening.session.address, &opening.session.name, false);
        self.install_remote_session(opening, false, false)
    }

    /// Refuse a bad Play payload with the normal connection-error screen.
    pub(in crate::game) fn refuse_play_payload(
        &mut self,
        error: impl std::fmt::Display,
    ) -> Result<()> {
        log::warn!("Sunrise: {error}");
        self.leave_server(Some(Text::translate(
            "disconnect.genericReason",
            &[Text::literal(error.to_string())],
        )))
    }

    /// Disconnect and return to the multiplayer screen (DisconnectedScreen).
    pub(in crate::game) fn leave_server(&mut self, reason: Option<Text>) -> Result<()> {
        if reason.is_none() && self.integrated.as_ref().is_some_and(|s| !s.stopped()) {
            return self.begin_closing_save(false);
        }
        self.close_server_session(reason, |_| {})
    }

    /// `ClientPacketListener.handleLogin` when `Minecraft.player != null`: retain the live player, run `resetPos`, rebind residency, and reset input.
    pub(in crate::game) fn retain_remote_player_on_login(&mut self) {
        self.side_reset_client_item_use();
        self.hook.discard();
        if let Some(r) = self.remote.as_mut() {
            r.respawn_velocity = None;
        }
        let retained_max_health = self.player.as_mut().map(|player| {
            let _ = player.set_pose(sunrise_client::pose::Pose::Standing);
            player.velocity = Vec3::ZERO;
            player.pitch = 0.0;
            let max_health = player
                .attribute("minecraft:max_health")
                .ok()
                .map(|v| v as f32);
            if let Some(max) = max_health {
                let _ = player.set_health(max, false);
            }
            max_health
        });
        if let Some(max_health) = retained_max_health {
            self.pitch = 0.0;
            if let Some(max) = max_health
                && let Some(r) = self.remote.as_mut()
            {
                r.health.0 = max;
            }
            self.ui.death_time = 0;
            self.input_player_replaced();
        }
        self.bind_local_player_residency();
    }
}
