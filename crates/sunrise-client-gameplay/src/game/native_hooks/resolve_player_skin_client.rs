use crate::game::ClientGame;
use mod_api::{HookId, PlayerModel, PlayerSkinOverride};

impl ClientGame<'_> {
    /// Install the resolved appearance on the render-only avatar and return the original pair.
    pub(crate) fn apply_player_skin_override(
        &mut self,
        panorama: bool,
    ) -> Option<(Option<String>, bool)> {
        if !mod_api::__private::listened(HookId::ResolvePlayerSkin)
            || panorama
            || self.common.player.is_none()
            || self.client.player_model.is_none()
        {
            return None;
        }
        let mut selection = None;
        self.native_resolve_player_skin(&mut selection);
        let PlayerSkinOverride { texture, model, .. } = selection?;
        // The same packed atlas is sampled by the body, the hand, and the preview.
        // The pool is absent if the current atlas has no slots for live images.
        if self.client.gui.atlas.dynamic_texture_pool.is_none()
            || self.client.dynamic_textures.size(&texture) != Some([64, 64])
        {
            return None;
        }
        let avatar = self.client.player_model.as_mut()?;
        Some((
            avatar.skin.replace(texture.to_string()),
            std::mem::replace(&mut avatar.slim, model == PlayerModel::Slim),
        ))
    }
}
