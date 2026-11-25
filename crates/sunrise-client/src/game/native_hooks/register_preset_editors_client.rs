#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_preset_editors::native_register_preset_editors;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_preset_editors::*;

impl Game<'_> {
    pub(in crate::game) fn preset_editor_dimensions(&self) -> Option<mod_api::Compound> {
        let sunrise_core::nbt::Tag::Compound(dimensions) =
            self.preset_editor_title()?.preset_editor_dimensions()?
        else {
            return None;
        };
        Some(dimensions)
    }
    pub(in crate::game) fn set_preset_editor_dimensions(
        &self,
        dimensions: mod_api::Compound,
    ) -> bool {
        self.preset_editor_title().is_some_and(|title| {
            title.set_preset_editor_dimensions(sunrise_core::nbt::Tag::Compound(dimensions))
        })
    }
    pub(in crate::game) fn preset_editor_title(&self) -> Option<&sunrise_client::title::Title> {
        if self.client.screen != Screen::Playing
            || self.screen_over_playing()
            || self.remote_loading()
        {
            return None;
        }
        self.title
            .0
            .as_ref()
            .filter(|title| title.preset_editor_session().is_some())
    }
}
