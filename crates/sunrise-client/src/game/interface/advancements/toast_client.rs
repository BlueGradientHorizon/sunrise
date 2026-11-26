use sunrise_client::{hud::rgba, toasts::Slide};
use sunrise_game_data::text::Text;
use sunrise_items::items::ItemStack;
use sunrise_sim::advancements::Frame;

pub(in crate::game) struct Toast {
    pub(in crate::game) icon: ItemStack,
    pub(in crate::game) frame: Frame,
    pub(in crate::game) title: Text,
    pub(in crate::game) description: Option<Text>,
    pub(in crate::game) slide: Slide,
    /// A `RecipeToast`: the crafting station and result of each display
    /// unlocked while it lasts (`recipeItems`); empty for an advancement.
    pub(in crate::game) recipes: Vec<(String, ItemStack)>,
    /// `RecipeToast.changed` and `lastChanged`.
    pub(in crate::game) changed: bool,
    pub(in crate::game) last_changed: i64,
}
impl Toast {
    /// `RecipeToast.addOrUpdate` on a new toast.
    pub(in crate::game) fn recipe(items: Vec<(String, ItemStack)>) -> Self {
        Self {
            icon: ItemStack::new("minecraft:air", 0),
            frame: Frame::Task,
            title: Text::translate("recipe.toast.title", &[]),
            description: Some(Text::translate("recipe.toast.description", &[])),
            slide: Slide::default(),
            recipes: items,
            changed: true,
            last_changed: 0,
        }
    }
    pub(in crate::game) fn is_recipe(&self) -> bool {
        !self.recipes.is_empty()
    }
    /// `RecipeToast.addItem`: more displays on a toast still queued or shown.
    pub(in crate::game) fn add_recipes(&mut self, items: Vec<(String, ItemStack)>) {
        self.recipes.extend(items);
        self.changed = true;
    }
    pub(in crate::game) fn is_challenge(&self) -> bool {
        !self.is_recipe() && self.frame == Frame::Challenge
    }
    pub(in crate::game) fn update(&mut self, multiplier: f64) -> (Option<bool>, bool) {
        let mut slide = std::mem::take(&mut self.slide);
        let result = slide.update_now(|fully| self.wants_show(fully, multiplier));
        self.slide = slide;
        result
    }
    /// `Toast.update` with `fullyVisibleForMs`: whether it wants to be shown.
    pub(in crate::game) fn wants_show(&mut self, fully_for: i64, multiplier: f64) -> bool {
        let time = 5000.0 * multiplier;
        if self.recipes.is_empty() {
            // AdvancementToast.
            return (fully_for as f64) < time;
        }
        if self.changed {
            self.last_changed = fully_for;
            self.changed = false;
        }
        ((fully_for - self.last_changed) as f64) < time
    }
    /// `RecipeToast.displayedRecipeIndex`: the displays take turns over the
    /// toast's display time.
    pub(in crate::game) fn displayed_recipe(&self, fully_for: i64, multiplier: f64) -> usize {
        let n = self.recipes.len() as f64;
        (fully_for as f64 / (5000.0 * multiplier / n).max(1.0) % n) as usize
    }
}

/// The toast's frame label and its colour (`AdvancementToast`: -30465, pink,
/// for a challenge, else -256, yellow), packed as `hud::rgba` for the batch.
pub(in crate::game) fn toast_label(frame: Frame) -> (&'static str, u32) {
    match frame {
        Frame::Task => ("advancements.toast.task", rgba(255, 255, 0, 255)),
        Frame::Goal => ("advancements.toast.goal", rgba(255, 255, 0, 255)),
        Frame::Challenge => ("advancements.toast.challenge", rgba(255, 136, 255, 255)),
    }
}
