#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::host::draw::GameDrawContext;
#[allow(unused_imports)]
use mod_api::{__private::game::CallbackOwner, ClientHost, DrawContext, GuiLayerWrapContext};
#[allow(unused_imports)]
use sunrise_client::{
    gui_layers::{self, GuiLayerBody, VanillaGuiLayer},
    hud::HudPaintCheckpoint,
};

#[allow(dead_code)]
impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// The item entities of the server level `dimension` whose box meets
    /// the box `min`..`max`, in the level's order: vanilla's
    /// `Level.getEntitiesOfClass(ItemEntity.class, box, ENTITY_STILL_ALIVE)`, as a
    /// hopper's `getItemsAtAndAbove` asks for its block and the one above
    /// (`[x, y, z]`..`[x + 1, y + 2, z + 1]`). Empty for a level not in place
    /// (as `native_set_block`) and on a client of a server.
    pub(in crate::game) fn native_item_entities(
        &self,
        dimension: &str,
        min: [f64; 3],
        max: [f64; 3],
    ) -> Vec<NativeItemEntity> {
        if (self.serving.is_none() && self.options.dedicated.is_none())
            || dimension != self.dim.id()
        {
            return Vec::new();
        }
        self.dropped
            .iter()
            .filter(|e| e.stack.count > 0 && box_meets(&e.bounds(), min, max))
            .map(|e| NativeItemEntity {
                id: e.id,
                position: [e.position.x, e.position.y, e.position.z],
                stack: e.stack.clone(),
                // The helper exposes an unsigned delay; keep its projection.
                pickup_delay: e.pickup_delay.clamp(0, i64::from(u32::MAX)) as u32,
            })
            .collect()
    }
    /// This `Game` is the window's, not a server's.
    pub(in crate::game) fn native_window(&self) -> bool {
        self.options.execution_role == ExecutionRole::Client
    }
}

// The vanilla paint procedure belongs to the caller's frame, not the frozen layer order.
pub(in crate::game) type VanillaGuiDraw<'p, 'g> =
    dyn FnMut(&mut crate::game::ClientGame<'g>, &mut HudBatch) + 'p;

pub(in crate::game) struct GuiLayerWrapFrame<'a, 'p, 'g> {
    pub(in crate::game) game: &'a mut crate::game::ClientGame<'g>,
    pub(in crate::game) batch: &'a mut HudBatch,
    pub(in crate::game) previous: &'a GuiLayerBody,
    pub(in crate::game) vanilla: Option<&'a mut VanillaGuiDraw<'p, 'g>>,
    pub(in crate::game) paint: &'a mut HudPaintCheckpoint,
}

impl GuiLayerWrapContext for GuiLayerWrapFrame<'_, '_, '_> {
    fn host(&self) -> &dyn ClientHost {
        &*self.game
    }

    fn with_draw(&mut self, draw: &mut dyn FnMut(&mut dyn DrawContext)) {
        draw(&mut GameDrawContext::new(self.game, self.batch));
    }

    fn draw_previous(&mut self) {
        let start = self.batch.vertices.len();
        self.game
            .draw_gui_layer_body(self.batch, self.previous, self.vanilla.as_deref_mut());
        self.paint
            .preserve_delegated(start..self.batch.vertices.len());
    }
}
