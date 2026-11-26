use super::*;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// Change the active menu without losing window 0's cursor or state counter.
    pub(in crate::game) fn remote_set_menu(
        &mut self,
        menu: Option<(i32, String)>,
        title: Option<sunrise_client_core::text::Text>,
    ) {
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        match (r.menu.as_ref().is_some(), menu.is_some()) {
            (false, true) => {
                r.inventory_carried = self.carried.take();
                r.inventory_state_id = std::mem::take(&mut r.state_id);
                for (saved, active) in r.inventory_craft_grid.iter_mut().zip(&mut self.craft_grid) {
                    *saved = active.take();
                }
                r.inventory_craft_result = r.craft_result.take();
            }
            (true, false) => {
                self.carried = r.inventory_carried.take();
                r.state_id = std::mem::take(&mut r.inventory_state_id);
                self.craft_grid.fill(None);
                for (saved, active) in r.inventory_craft_grid.iter_mut().zip(&mut self.craft_grid) {
                    *active = saved.take();
                }
                r.craft_result = r.inventory_craft_result.take();
            }
            (true, true) => {
                self.carried = None;
                r.state_id = 0;
                self.craft_grid.fill(None);
                r.craft_result = None;
            }
            (false, false) => {}
        }
        r.menu.set(menu);
        r.set_menu_title(title);
    }

    /// The type of the menu a server opened (`MenuType`'s id).
    pub(in crate::game) fn remote_menu_type(&self) -> Option<&str> {
        Some(self.remote.as_ref()?.menu.as_ref()?.1.as_str())
    }
    /// A server's chest-like menu (`MenuType.GENERIC_9xN`): its rows and title.
    pub(in crate::game) fn remote_chest_menu(
        &self,
    ) -> Option<(usize, sunrise_client_core::text::Text)> {
        let r = self.remote.as_ref()?;
        let rows = r
            .menu
            .as_ref()?
            .1
            .strip_prefix("minecraft:generic_9x")?
            .parse()
            .ok()?;
        Some((
            rows,
            r.menu_title
                .clone()
                .unwrap_or_else(sunrise_client_core::text::Text::empty),
        ))
    }
}
