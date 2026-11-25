use super::Game;
use mod_api::{HookId, PlayerModel, PlayerSkinOverride};

impl Game<'_> {
    /// Collect the local player's skin and arm width together, before render extraction.
    pub(in crate::game) fn native_resolve_player_skin(
        &self,
        selection: &mut Option<mod_api::PlayerSkinOverride>,
    ) {
        self.fire_resolve_player_skin(selection);
    }

    /// Install the resolved appearance on the render-only avatar and return the original pair.
    pub(in crate::game) fn apply_player_skin_override(
        &mut self,
        panorama: bool,
    ) -> Option<(Option<String>, bool)> {
        if !mod_api::__private::listened(HookId::ResolvePlayerSkin)
            || panorama
            || self.player.is_none()
            || self.player_model.is_none()
        {
            return None;
        }
        let mut selection = None;
        self.native_resolve_player_skin(&mut selection);
        let PlayerSkinOverride { texture, model, .. } = selection?;
        // The body, the hand and the preview sample one packed atlas, so the 64 x 64 dynamic
        // texture is copied on the GPU into a skin slot of it, every frame it is selected
        // (as `skin_slots` hands slots out for downloaded skins). Anything else keeps vanilla.
        if self.dynamic_textures.size(&texture) != Some([64, 64]) {
            return None;
        }
        let (slot, _) = self.skin_slots.place(format!("dynamic:{texture}"))?;
        let name = format!("sunrise/skin{slot}");
        let rect = self.gui.atlas.sprites.get(&name)?;
        let dst = [
            (rect.u0 * self.gui.atlas.width as f32).round() as u32,
            (rect.v0 * self.gui.atlas.height as f32).round() as u32,
        ];
        self.dynamic_textures.copy_to_atlas(&texture, dst);
        let avatar = self.player_model.as_mut()?;
        Some((
            avatar.skin.replace(name),
            std::mem::replace(&mut avatar.slim, model == PlayerModel::Slim),
        ))
    }
}

#[cfg(test)]
#[path = "resolve_player_skin/tests.rs"]
mod tests;
