//! Developer single-player startup and network-entry policy.
#[allow(unused_imports)]
use super::lan::LanSettings;
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use sunrise_client::lan::{NetConfig, hub::HubConfig};

impl Options {
    pub(in crate::game) fn validate_developer_single_player(&self) -> Result<()> {
        if self.developer_single_player
            && (self.server.is_some()
                || self.session_stdin
                || self.session_file.is_some()
                || !self.execution_role.is_client())
        {
            return Err(
                sunrise_client::lang::get("sunrise.developer_offline.single_player").into(),
            );
        }
        Ok(())
    }
}
