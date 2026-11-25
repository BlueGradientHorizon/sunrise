use super::Game;
use mod_api::HookId;

impl Game<'_, crate::game::client_side::ClientState> {
    /// Only a subscribed client lays out substituted images. Atlas, dynamic sprite geometry and
    /// mod toggles can each invalidate layout, but updating a texture's pixels does not.
    pub(in crate::game::client_side) fn inline_images_revision(&self) -> Option<[u64; 3]> {
        (self.native_window()
            && mod_api::__private::enabled_listened(HookId::SubstituteInlineImages))
        .then(|| {
            [
                self.gui.atlas.revision,
                self.side.dynamic_textures.geometry_revision(),
                mod_api::__private::mods_switched(),
            ]
        })
    }
}
