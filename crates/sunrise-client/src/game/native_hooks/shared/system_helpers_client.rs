use super::*;

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
#[allow(dead_code)]
impl Game<'_, crate::game::client_side::ClientState> {
    /// The frame counter: frames in the last second, the slowest frame and
    /// the 1% low of the last 5 seconds, in frames per second.
    pub(in crate::game) fn native_frame_rate(&self) -> (u32, u32, u32) {
        self.frame_reading()
    }
    /// Attack and use clicks in the last second.
    pub(in crate::game) fn native_clicks(&mut self) -> (u32, u32) {
        self.click_reading()
    }
    /// Every item and recipe the game knows, made once (the same for every
    /// call); none outside a world. They are the window's own (the content
    /// mods' and vanilla's), with the server's recipes of the types a mod had
    /// it send (`native_datapack_sync`) in their place; made again when those
    /// arrive (`native_recipes_received`).
    pub(in crate::game) fn native_recipes(&mut self) -> Option<Arc<NativeRecipes>> {
        self.player.as_ref()?;
        self.made_recipes()
    }
    /// A line in chat from the client.
    pub(in crate::game::client_side) fn native_chat(&mut self, text: &str) {
        let focused = self.side.screen == Screen::Chat;
        let line = Text::literal(text);
        let ticks = self.visual_ticks();
        let atlas = &self.gui.atlas;
        self.side
            .chat_ui
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
        self.side.panel.show_recipes(mod_id, page);
    }
    /// The areas the game itself draws beside the open screen's window: the
    /// container build's, or the frame's own arithmetic with no build yet.
    pub(in crate::game::client_side) fn screen_own_exclusion(
        &self,
        batch: &HudBatch,
    ) -> Vec<[f32; 4]> {
        if !matches!(self.side.screen, Screen::Inventory | Screen::Creative) {
            return Vec::new();
        }
        match self.built_container(batch) {
            Some(layout) => layout.exclusion(),
            None => self.container_layout(batch).exclusion(),
        }
    }
}
