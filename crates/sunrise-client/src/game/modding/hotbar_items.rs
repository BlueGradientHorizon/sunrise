//! `GuiDraw::Item` for the hotbar: the local player's hotbar slots drawn as `extractSlot`
//! draws them (`item`, then `itemDecorations`).
use super::Game;
use sunrise_client::hud::HudBatch;
use sunrise_client::hud_layout::GuiItems;
use sunrise_client::items::ItemStack;

/// The local player's hotbar slots (0-8) and off hand (40), drawn for `GuiDraw::Item`.
pub(crate) struct HotbarItems<'a, 'g> {
    pub(crate) game: &'a Game<'g>,
}

impl HotbarItems<'_, '_> {
    fn stack(&self, slot: u32) -> Option<&ItemStack> {
        match slot {
            40 => self.game.offhand.as_ref(),
            0..=8 => self.game.inventory.get(slot as usize)?.as_ref(),
            _ => None,
        }
    }
}

impl GuiItems for HotbarItems<'_, '_> {
    fn item(&mut self, batch: &mut HudBatch, slot: u32, x: i32, y: i32, seed: i32) {
        if let Some(stack) = self.stack(slot) {
            self.game
                .draw_item_model(batch, stack, x as f32, y as f32, seed);
        }
    }

    fn decorations(&mut self, batch: &mut HudBatch, slot: u32, x: i32, y: i32) {
        if let Some(stack) = self.stack(slot) {
            self.game
                .draw_item_decorations(batch, stack, x as f32, y as f32, None);
        }
    }
}
