#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::autosave::*;

impl Game<'_> {
    /// The autosave doesn't `wait` for the chunks to be on disk: every
    /// generated chunk is saved, hundreds at a time, encoded and written on
    /// the region writer meanwhile.
    pub(in crate::game) fn save_game_with(&mut self, wait: bool) -> Result<()> {
        if self.remote.is_some() {
            return Ok(());
        }
        self.admit_level_save()?;
        // `DedicatedServer.saveAllChunks` tells the management protocol's clients when a save starts and when
        // it ends without failing, for every save: the command, the autosave, the pause.
        let dedicated = self.is_dedicated_server();
        if dedicated {
            dedicated::events::record_server("saving", serde_json::json!({}));
        }
        self.save_level(wait)?;
        self.save_parked_levels(wait)?;
        self.save_game_metadata()?;
        let _ = self.finish_player_autosaves(wait);
        if wait {
            self.client.message = Some((
                sunrise_client::lang::get("commands.save.success"),
                Instant::now(),
            ));
        }
        self.native_world_saved(wait);
        if dedicated {
            dedicated::events::record_server("saved", serde_json::json!({}));
        }
        Ok(())
    }
}
