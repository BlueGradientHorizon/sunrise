#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::system_helpers::*;

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
#[allow(dead_code)]
impl Game<'_> {
    /// A line in chat from the client.
    pub(in crate::game::client_side) fn native_chat(&mut self, text: &str) {
        let focused = self.client.screen == Screen::Chat;
        let line = Text::literal(text);
        let ticks = self.visual_ticks();
        let atlas = &self.gui.atlas;
        self.ui
            .chat
            .add(atlas, line, None, ChatSource::Client, ticks, focused);
    }
    /// Shows these recipes in the panel instead of its items, as many as
    /// fit a page, under `title` and a back button, with a row of `tabs`
    /// (each an icon; `tab` is the selected one) between them. At most 256
    /// recipes and 16 tabs; no recipes shows the items again. The same title
    /// and tab again keep the page the player turned to.
    pub(in crate::game) fn native_panel_recipes(
        &mut self,
        mod_id: &str,
        title: &str,
        tabs: Vec<mod_api::Stack>,
        tab: u32,
        views: Vec<NativeRecipeView>,
    ) {
        let page = RecipePage::new(title, tabs, tab, views);
        self.client.panel.show_recipes(mod_id, page);
    }
    /// The areas the game itself draws beside the open screen's window: the
    /// container build's, or the frame's own arithmetic with no build yet.
    pub(in crate::game::client_side) fn screen_own_exclusion(
        &self,
        batch: &HudBatch,
    ) -> Vec<[f32; 4]> {
        if !matches!(self.client.screen, Screen::Inventory | Screen::Creative) {
            return Vec::new();
        }
        match self.built_container(batch) {
            Some(layout) => layout.exclusion(),
            None => self.container_layout(batch).exclusion(),
        }
    }
}
