#[allow(unused_imports)]
pub(in crate::game::server_side) use super::lan::LanSettings;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::developer_launch::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::lan::{NetConfig, hub::HubConfig};

impl Game<'_, crate::game::server_side::ServerState> {
    /// Start the network side and the play-state hub: for the LAN (with the
    /// host's own profile reserved) or for a dedicated server (no host).
    pub(in crate::game) fn lan_host(
        &mut self,
        settings: LanSettings,
        net: NetConfig,
        config: HubConfig,
    ) -> Result<()> {
        if self.options.developer_single_player && net.listen {
            return Err(
                sunrise_client::lang::get("sunrise.developer_offline.single_player").into(),
            );
        }
        self.start_lan_hub(settings, net, config)
    }
}
