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
impl Game<'_> {
    /// Sets a block in the current authoritative level through `Level.setBlock`.
    /// Returns false for another dimension, an invalid state, or no change.
    /// `flags` selects vanilla's neighbour, shape and client updates. A block
    /// entity block gets its block entity.
    pub(in crate::game) fn native_set_block(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        state: BlockStateId,
        flags: i32,
    ) -> bool {
        if (self.serving.is_none() && self.options.dedicated.is_none())
            || dimension != self.dim.id()
            || self.pack.assets.registry.state(state).is_none()
        {
            return false;
        }
        let changed = self.set_block_live(pos, state, flags, true);
        let _ = self.process_level_events();
        changed
    }
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
    /// Runs `change` on the mod block entity at `pos` (a generator charging
    /// the machine beside it) and returns what it returns; none for any
    /// other block entity. Afterwards, as its `setChanged` would, its chunk is saved next
    /// time and, when its items changed, comparators read it again.
    pub(in crate::game) fn native_with_mod_block_entity<R>(
        &mut self,
        pos: [i32; 3],
        change: impl FnOnce(&mut EngineBlockEntity) -> R,
    ) -> Option<R> {
        let key = (pos[0], pos[1], pos[2]);
        let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&key) else {
            return None;
        };
        let items = m.items.clone();
        let out = change(m);
        let moved = m.items != items;
        self.world.modified.insert((key.0 >> 4, key.2 >> 4));
        if moved {
            self.container_changed(key);
        }
        Some(out)
    }
    /// This `Game` is the window's, not a server's.
    pub(in crate::game) fn native_window(&self) -> bool {
        self.options.execution_role == ExecutionRole::Client
    }
}

// The vanilla paint procedure belongs to the caller's frame, not the frozen layer order.
pub(in crate::game) type VanillaGuiDraw<'p, 'g> = dyn FnMut(&mut Game<'g>, &mut HudBatch) + 'p;

pub(in crate::game) struct GuiLayerWrapFrame<'a, 'p, 'g> {
    pub(in crate::game) game: &'a mut Game<'g>,
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

impl<'g> Game<'g> {
    /// Dispatches a vanilla slot after flushing inserted layers below it. Replacing or wrapping
    /// a body leaves the slot's ID, order and hooks unchanged. The original body's eligibility
    /// applies only to the original paint, including when a wrapper delegates to it.
    pub(in crate::game) fn gui_layer(
        &mut self,
        layer: VanillaGuiLayer,
        batch: &mut HudBatch,
        draw: &mut dyn FnMut(&mut Game<'g>, &mut HudBatch),
    ) {
        self.draw_mod_gui_layers(Some(layer), batch);
        self.run_gui_layer(
            layer.id(),
            batch,
            gui_layers::order().map_or(&GuiLayerBody::Vanilla, |order| {
                &order.slots()[order.position(layer)].body
            }),
            Some(draw),
            matches!(
                layer,
                VanillaGuiLayer::PlayerHealth
                    | VanillaGuiLayer::ArmorLevel
                    | VanillaGuiLayer::FoodLevel
                    | VanillaGuiLayer::VehicleHealth
                    | VanillaGuiLayer::AirLevel
            ),
        );
    }

    pub(in crate::game) fn run_gui_layer(
        &mut self,
        id: &str,
        batch: &mut HudBatch,
        body: &GuiLayerBody,
        vanilla: Option<&mut VanillaGuiDraw<'_, 'g>>,
        status_row: bool,
    ) {
        let mut canceled = false;
        self.native_gui_layer_pre(id, batch, &mut canceled);
        if canceled {
            return;
        }
        let start = batch.vertices.len();
        self.draw_gui_layer_body(batch, body, vanilla);
        if status_row {
            self.keep_status_rows(&batch.vertices[start..], batch.width, batch.height);
        }
        self.native_gui_layer_post(id, batch);
    }

    pub(in crate::game) fn draw_gui_layer_body(
        &mut self,
        batch: &mut HudBatch,
        body: &GuiLayerBody,
        vanilla: Option<&mut VanillaGuiDraw<'_, 'g>>,
    ) {
        match body {
            GuiLayerBody::Vanilla => {
                // Only a vanilla slot can contain this node; its paint site lends the closure.
                if let Some(draw) = vanilla {
                    CallbackOwner::call_engine(|| draw(self, batch));
                }
            }
            GuiLayerBody::Native { owner, draw } => {
                if let Some(owner) = owner.as_ref().filter(|owner| owner.is_enabled()) {
                    let paint = batch.paint_checkpoint();
                    if owner
                        .call("HUD layer", "gui_layer_draw", || {
                            draw(self, &mut GameDrawContext::new(self, batch));
                        })
                        .is_err()
                    {
                        paint.discard(batch);
                    }
                }
            }
            GuiLayerBody::Wrapped {
                owner,
                wrap,
                previous,
            } => {
                let Some(owner) = owner.as_ref().filter(|owner| owner.is_enabled()) else {
                    self.draw_gui_layer_body(batch, previous, vanilla);
                    return;
                };
                let mut paint = batch.paint_checkpoint();
                if owner
                    .call("HUD layer", "gui_layer_wrapper", || {
                        wrap(&mut GuiLayerWrapFrame {
                            game: self,
                            batch,
                            previous,
                            vanilla,
                            paint: &mut paint,
                        });
                    })
                    .is_err()
                {
                    paint.discard(batch);
                }
            }
        }
    }
}
