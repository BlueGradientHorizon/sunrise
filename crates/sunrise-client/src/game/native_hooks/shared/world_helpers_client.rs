#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::draw::GameDrawContext;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::world_helpers::GuiLayerWrapFrame;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::world_helpers::VanillaGuiDraw;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::world_helpers::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    __private::game::CallbackOwner, ClientHost, DrawContext, GuiLayerWrapContext,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    gui_layers::{self, GuiLayerBody, VanillaGuiLayer},
    hud::HudPaintCheckpoint,
};

#[allow(dead_code)]
impl Game<'_> {
    /// Applies a client mod's `clear_particles` if one is waiting. Called at the start of
    /// each particle tick, after `RenderFrameEvent.Pre`, and at particle extraction, after
    /// HUD, screen and entity callbacks. Extraction also discards its geometry if a model
    /// callback asks to clear while it is being built, even while paused or tick-frozen.
    pub(in crate::game::client_side) fn apply_mod_particle_clear(&mut self) {
        if self.client.mod_clear_particles.take() {
            self.clear_client_particles();
        }
    }
    /// `ParticleEngine.clearParticles` over every group Sunrise keeps apart: the engine's
    /// particles, starters, queue and emitters, the item pickups and the elder guardian
    /// apparition. Leaving a level and switching levels clear through it too.
    pub(in crate::game) fn clear_client_particles(&mut self) {
        self.client.mod_clear_particles.set(false);
        self.particles.0.clear();
        self.pickups.0.clear();
    }
    /// `ParticleEngine.countParticles`: every live group. Sunrise keeps vanilla's groups
    /// apart: ordinary particles, firework starters and elder guardians in the engine,
    /// and ITEM_PICKUP in `pickups`.
    pub(in crate::game) fn client_particle_count(&self) -> usize {
        self.particles.0.group_count() + self.pickups.0.live.len()
    }
    /// A mod's own screen closes (Escape, or `mod_screen_close`).
    pub(in crate::game) fn close_mod_screen(&mut self) {
        if self.client.screen == Screen::Mod {
            if self.title.0.is_some() {
                self.close_mod_screen_to_title();
            } else {
                self.set_screen(Screen::Playing);
                self.capture(true);
            }
            if self.client.screen == Screen::Mod {
                // `screen_opening` kept it open, or opened a mod's own.
                return;
            }
        }
        self.client.native_mod_screen = None;
    }
    /// `Screen.onClose`'s `setScreen(null)` with no world: the title screen
    /// opens, asked for once as `mod` to `title`, and a mod's answer applies
    /// as on any title screen. Told to the frame's observer, so it doesn't
    /// announce it again.
    pub(in crate::game::client_side) fn close_mod_screen_to_title(&mut self) {
        let decision = if self.screen_hooks_heard() {
            let current = self.screen_change_view(Screen::Mod);
            let none = self.screen_change_view(Screen::Playing);
            self.decide_screen(&current, &screen_view_with_id(&none, "title"), true)
        } else {
            ScreenDecision::Open
        };
        match decision {
            ScreenDecision::Keep => (),
            ScreenDecision::ModScreen(mod_id) => self.apply_screen(Screen::Mod, Some(mod_id)),
            ScreenDecision::Vanilla(target) => self.open_screen_target(target),
            ScreenDecision::Open => {
                if let Some(title) = self.title.0.as_mut() {
                    title.open_by_id("title");
                }
                self.apply_screen(Screen::Playing, None);
            }
        }
    }
    /// Whether a box in `corner` shows: F3's debug screen takes both top
    /// corners (its left and right columns), and a bottom corner while one
    /// of its charts is drawn there.
    pub(in crate::game::client_side) fn native_corner_shown(&self, corner: Corner) -> bool {
        let charts = self
            .client
            .debug_charts
            .corners(!self.has_singleplayer_server());
        !debug_takes(corner, self.client.debug, charts)
    }
}

impl<'g> Game<'g> {
    /// Draws inserted layers not drawn yet this frame, below `until` or to the end with `None`.
    /// A skipped vanilla layer keeps its place; its neighbours draw at the next paint site.
    /// Inserted layers don't draw while the HUD is hidden.
    pub(in crate::game) fn draw_mod_gui_layers(
        &mut self,
        until: Option<VanillaGuiLayer>,
        batch: &mut HudBatch,
    ) {
        let Some(next) = self.client.hud_layer_next else {
            return;
        };
        let Some(order) = gui_layers::order() else {
            return;
        };
        let slots = order.slots();
        let end = until.map_or(slots.len(), |layer| order.position(layer));
        // An earlier anchor can still paint later (e.g. deferred subtitles). Don't redraw slots.
        for slot in &slots[next.min(end)..end] {
            if slot.vanilla_anchor().is_none() {
                self.run_gui_layer(slot.id(), batch, &slot.body, None, false);
            }
        }
        // Past `until` itself: its paint site draws it, not the cursor.
        self.client.hud_layer_next = Some(next.max(end + usize::from(until.is_some())));
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
