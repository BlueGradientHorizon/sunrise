#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::substitute_inline_images::substitute;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::substitute_inline_images::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{Answered, HookId, InlineImageReplacement};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::BTreeMap;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::hud::HudAtlas;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::text::{InlineObject, Text};

impl Game<'_> {
    /// Only a subscribed client lays out substituted images. Atlas, dynamic sprite geometry and
    /// mod toggles can each invalidate layout, but updating a texture's pixels does not.
    pub(in crate::game) fn inline_images_revision(&self) -> Option<[u64; 3]> {
        (self.native_window()
            && mod_api::__private::enabled_listened(HookId::SubstituteInlineImages))
        .then(|| {
            [
                self.gui.atlas.revision,
                self.client.dynamic_textures.geometry_revision(),
                mod_api::__private::mods_switched(),
            ]
        })
    }
}
