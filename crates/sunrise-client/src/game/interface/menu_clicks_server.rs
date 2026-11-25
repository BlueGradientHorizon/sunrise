#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::menu_clicks::*;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn creative_display_click_server(
        &mut self,
        shown: Option<&ItemStack>,
        button: i32,
        input: sunrise_client::menu::Input,
    ) -> sunrise_client::creative::DisplayClick {
        let catalog = world_items!(self);
        let max = |s: &ItemStack| catalog.get(&s.item).map_or(64, |i| i.max_stack);
        sunrise_client::creative::click_display(&mut self.carried, shown, max, button, input)
    }
    #[inline(always)]
    pub(in crate::game) fn menu_session_with_game_server(&mut self, before: &[Option<ItemStack>]) {
        if let Some(BlockEntity::Furnace(f)) = self
            .container
            .and_then(|k| self.world.block_entities.get_mut(&k))
        {
            // AbstractFurnaceBlockEntity.setItem: only a new input restarts cooking.
            f.input_clicked(&self.loop_context, world_recipes!(self), before[0].clone());
        }
    }
}
