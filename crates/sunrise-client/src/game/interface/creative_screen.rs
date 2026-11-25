//! The creative inventory screen (`CreativeModeInventoryScreen`): its tabs, pages, tooltips,
//! trash slot, search box and clicks.
use super::*;
impl<__Side: crate::game::Side> Game<'_, __Side> {
    pub(in crate::game) fn creative_origin(batch: &HudBatch) -> (f32, f32) {
        batch.centered_origin(Creative::WIDTH, Creative::HEIGHT)
    }
}
