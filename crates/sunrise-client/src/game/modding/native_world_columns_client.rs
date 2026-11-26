//! Client-side column height reads for native mods.
use super::Game;
use crate::game::block_edits_common::MAX_COLUMNS;

impl Game<'_, crate::game::client_side::ClientState> {
    /// For a client mod: each column's height by `heightmap` in the world this window shows,
    /// none for a column whose chunk this client does not hold.
    pub(in crate::game) fn client_surface_heights(
        &self,
        columns: &[(i32, i32)],
        heightmap: mod_api::Heightmap,
    ) -> Result<Vec<Option<i32>>, String> {
        if !self.side.remote.as_ref().is_some_and(|r| r.logged_in) {
            return Err("no client world is open".to_string());
        }
        if columns.len() > MAX_COLUMNS {
            return Err(format!(
                "{} columns, more than {MAX_COLUMNS} a call",
                columns.len()
            ));
        }
        Ok(self.loaded_column_heights(columns, heightmap))
    }
}
