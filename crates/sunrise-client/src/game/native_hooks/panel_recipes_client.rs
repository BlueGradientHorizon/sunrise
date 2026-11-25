use super::*;
use crate::game::native_hooks::panel_recipes::page_views;

impl Game<'_, crate::game::client_side::ClientState> {
    /// Shows `page` in mod `mod_id`'s item panel instead of its items, cut to the renderer's
    /// limits; `None` shows the items again.
    pub(in crate::game) fn panel_recipes(
        &mut self,
        mod_id: &str,
        page: Option<mod_api::RecipePage>,
    ) {
        let (title, tabs, tab, views) = page_views(page);
        self.native_panel_recipes(mod_id, &title, tabs, tab, views);
    }
}
