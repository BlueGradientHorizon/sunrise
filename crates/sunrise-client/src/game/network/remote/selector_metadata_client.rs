use super::*;

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn receive_selector_tokens(&mut self, data: &[u8]) -> Result<()> {
        let Some(remote) = self.remote.as_mut() else {
            return Ok(());
        };
        if !remote.selector_metadata_supported {
            return Ok(());
        }
        match sunrise_client::lan::commands::selector_sync::decode(data)? {
            sunrise_client::lan::commands::selector_sync::Snapshot::Tokens(tokens) => {
                self.ui.registries.selector_tokens = tokens;
            }
            sunrise_client::lan::commands::selector_sync::Snapshot::Unsupported(revision) => {
                remote.selector_metadata_supported = false;
                self.ui.registries.selector_tokens.clear();
                log::warn!(
                    "Sunrise: selector metadata revision {revision} is unsupported; tokens disabled for this connection"
                );
            }
        }
        Ok(())
    }
}
