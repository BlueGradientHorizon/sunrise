#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::resolve_player_skin::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{HookId, PlayerModel, PlayerSkinOverride};

impl Game<'_, crate::game::client_side::ClientState> {
    /// Install the resolved appearance on the render-only avatar and return the original pair.
    pub(in crate::game::client_side) fn apply_player_skin_override(
        &mut self,
        panorama: bool,
    ) -> Option<(Option<String>, bool)> {
        if !mod_api::__private::listened(HookId::ResolvePlayerSkin)
            || panorama
            || self.player.is_none()
            || self.side.player_model.is_none()
        {
            return None;
        }
        let mut selection = None;
        self.native_resolve_player_skin(&mut selection);
        let PlayerSkinOverride { texture, model, .. } = selection?;
        // The same packed atlas is sampled by the body, the hand, and the preview.
        // The pool is absent if the current atlas has no slots for live images.
        if self.gui.atlas.dynamic_texture_pool.is_none()
            || self.side.dynamic_textures.size(&texture) != Some([64, 64])
        {
            return None;
        }
        let avatar = self.side.player_model.as_mut()?;
        Some((
            avatar.skin.replace(texture.to_string()),
            std::mem::replace(&mut avatar.slim, model == PlayerModel::Slim),
        ))
    }
}

#[cfg(test)]
#[path = "resolve_player_skin/tests_client.rs"]
pub(in crate::game) mod tests;
