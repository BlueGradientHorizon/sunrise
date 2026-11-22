//! The hotbar's `GuiGraphicsExtractor.extractSlot`: the item model followed by decorations.
use super::Game;
use sunrise_client::hud::HudBatch;
use sunrise_client::hud_layout::GuiItems;
use sunrise_client::items::ItemStack;

impl Game<'_> {
    pub(super) fn draw_item_hotbar(
        &self,
        batch: &mut HudBatch,
        [w, h]: [i32; 2],
        attack: Option<f32>,
    ) {
        use sunrise_client::hud_layout::{HotbarInputs, hotbar, paint_items};
        let partial = self.clock.fraction() as f32;
        let inputs = HotbarInputs {
            selected: self.selected as i32,
            right_handed: !self.settings.main_hand_left,
            offhand: self.offhand.is_some(),
            slots: std::array::from_fn(|i| {
                self.inventory[i]
                    .as_ref()
                    .map(|_| (f32::from(self.hotbar_pop[i]) - partial).max(0.0))
            }),
            offhand_pop: (f32::from(self.hotbar_pop[9]) - partial).max(0.0),
            attack,
        };
        let draws = hotbar(w, h, &inputs);
        paint_items(
            batch,
            &self.gui.atlas,
            &draws,
            &mut HotbarItems { game: self },
        );
    }
}

/// The local player's hotbar slots (0-8) and off hand (40), drawn for `GuiDraw::Item`.
struct HotbarItems<'a, 'g> {
    game: &'a Game<'g>,
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
