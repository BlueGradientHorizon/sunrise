//! Initial-load publication boundary shared by startup and its resource fixture.
use super::{Game, client_reload};
impl Game<'_> {
    pub(in crate::game) fn publish_initial_client_models(
        &mut self,
        client_applies: Vec<client_reload::PreparedClientReload>,
    ) -> sunrise_client::Result<()> {
        if self.options.is_client_start() {
            // The renderers and client setup read the world-rest tables. Join
            // them and validate extensions before publishing any listener state.
            self.join_world_rest()?;
            let item_extensions = Self::prepare_client_item_extensions(&self.gui)?;
            let phase = self.mod_phase.clone();
            client_reload::apply_client_reload(&phase, client_applies, || {
                self.armor_stand_assets.extensions = item_extensions;
                self.resolve_entity_renderers();
                self.texture_atlases_stitched();
                self.native_model_baking_completed();
            });
            self.native_client_setup();
        }
        Ok(())
    }
}
