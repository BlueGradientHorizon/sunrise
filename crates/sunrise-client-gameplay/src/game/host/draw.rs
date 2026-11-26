use mod_api::{prelude::*, *};
use sunrise_gameplay_common::game::host::draw::{
    EngineTooltipFont, font_id_str, label_layout_with,
};

/// [`DrawContext`] on the game's `crate::hud::HudBatch`: one per hook call.
#[cfg(feature = "client")]
pub struct GameDrawContext<'a, 'g> {
    pub(crate) game: &'a crate::game::ClientGame<'g>,
    pub(crate) surface: &'a mut crate::hud::HudBatch,
    /// The call's open clips (`push_clip`), closed when the call returns.
    pub(in crate::game) clips: crate::game::gui_clips::GuiClips,
    pub(in crate::game) tooltip_font: Option<&'a EngineTooltipFont<'a>>,
}
#[cfg(feature = "client")]
impl DrawContext for GameDrawContext<'_, '_> {
    fn size(&self) -> [f32; 2] {
        self.game.native_size(&*self.surface)
    }
    fn item_model(&mut self, item: ItemKey, model: &str, at: [f32; 2], size: f32) -> bool {
        let Some(item) = mod_api::__private::item_key_id(item) else {
            return false;
        };
        self.game
            .native_item_model(&mut *self.surface, item, model, at, size)
    }
    fn quad(&mut self, points: [[f32; 2]; 4], argb: u32) {
        self.game.native_quad(&mut *self.surface, points, argb)
    }
    fn fill(&mut self, rect: [f32; 4], argb: u32) {
        self.game.native_fill(&mut *self.surface, rect, argb)
    }
    fn fill_gradient(&mut self, rect: [f32; 4], top: u32, bottom: u32) {
        self.game
            .native_fill_gradient(&mut *self.surface, rect, top, bottom)
    }
    fn sprite(&mut self, name: &str, src: [f32; 4], dst: [f32; 4], tint: Option<u32>) -> bool {
        self.game
            .native_sprite(&mut *self.surface, name, src, dst, tint)
    }
    fn item(&mut self, item: ItemKey, at: [f32; 2], size: f32) -> bool {
        let Some(item) = mod_api::__private::item_key_id(item) else {
            return false;
        };
        self.game.native_item(&mut *self.surface, item, at, size)
    }
    fn text(&mut self, text: &str, at: [f32; 2], argb: u32, shadow: bool) {
        self.game
            .native_text(&mut *self.surface, text, at, argb, shadow)
    }
    fn text_with_font(
        &mut self,
        font: &Identifier,
        text: &Text,
        at: [f32; 2],
        argb: u32,
        shadow: bool,
    ) -> Result<f32, TextFormError> {
        let text: sunrise_game_data::text::Text = text.try_into_engine(&())?;
        let [a, r, g, b] = argb.to_be_bytes();
        if a == 0 {
            return Ok(0.0);
        }
        let formatted;
        let name: &str = match self.tooltip_font.filter(|active| active.id == font) {
            Some(active) => active.name(),
            None => {
                formatted = font_id_str(font);
                &formatted
            }
        };
        Ok(self.surface.text_styled_with_font(
            &self.game.client.gui.atlas,
            &text,
            at,
            crate::hud::rgba(r, g, b, a),
            shadow,
            name,
        ))
    }
    fn with_tooltip_font_raw(&self, font: &Identifier, f: &mut dyn FnMut(&dyn TooltipFont)) {
        if let Some(active) = self.tooltip_font.filter(|active| active.id == font) {
            f(active);
        } else {
            f(&EngineTooltipFont::new(
                &self.game.client.gui.atlas.font,
                font,
            ));
        }
    }
    fn text_scaled(&mut self, text: &str, at: [f32; 2], scale: f32, argb: u32, shadow: bool) {
        self.game
            .native_text_scaled(&mut *self.surface, text, at, scale, argb, shadow)
    }
    fn item_stack(&mut self, stack: &ItemStack, at: [f32; 2], size: f32) -> bool {
        let Ok(stack) = sunrise_gameplay_common::game::api_values::stack_value(stack) else {
            return false;
        };
        self.game
            .native_item_stack(&mut *self.surface, &stack, at, size)
    }
    fn gui_paint(&mut self, draws: &[GuiDraw]) -> Result<(), GuiPaintError> {
        let draws = draws
            .iter()
            .map(crate::game::client_side::api_values_client::gui_draw::gui_draw_to_engine)
            .collect::<Result<Vec<_>, _>>()?;
        crate::game::gui_pose::report_refused_poses(&draws);
        self.game.native_gui_paint(&mut *self.surface, &draws)
    }
    fn gui_nine_slice(&mut self, sprite: &str, rect: [f32; 4], color: u32) {
        self.game
            .native_gui_nine_slice(&mut *self.surface, sprite, rect, color)
    }
    fn gui_tooltip(
        &mut self,
        lines: &[Text],
        at: [f32; 2],
    ) -> Result<(), mod_api::text::TextFormError> {
        let lines: Vec<_> = lines
            .iter()
            .map(sunrise_sim::api_text::to_engine)
            .collect::<Result<_, _>>()?;
        self.game.native_gui_tooltip(&mut *self.surface, &lines, at);
        Ok(())
    }
    fn text_width(&self, text: &str) -> f32 {
        self.game.native_text_width(&*self.surface, text)
    }
    fn text_lines(
        &self,
        text: &Text,
        max_width: f32,
        align: TextAlign,
        scale: f32,
    ) -> Result<TextLines, TextLinesRefusal> {
        let local = sunrise_sim::api_text::to_local_engine(text);
        label_layout_with(
            &self.game.client.gui.atlas.font,
            local.engine(),
            max_width,
            align,
            scale,
            |piece| Ok(local.text_to_api(piece)),
        )
    }
    fn fluid(&mut self, fluid: &str, at: [f32; 2], size: f32) {
        self.game.native_fluid(&mut *self.surface, fluid, at, size)
    }
    fn draw_scene(
        &mut self,
        rect: [f32; 4],
        scene: &mod_api::Scene,
        view: SceneView,
    ) -> Result<(), mod_api::SceneError> {
        let blocks =
            crate::game::client_side::api_values_client::aggregates::scene_blocks(&scene.blocks)?;
        self.game
            .native_draw_scene(&mut *self.surface, rect, &blocks, scene, view)
    }
    fn model_3d(
        &mut self,
        subject: mod_api::Gui3dSubject<'_>,
        view: mod_api::Gui3dView,
    ) -> Result<(), mod_api::Gui3dError> {
        self.game.native_model_3d(&mut *self.surface, subject, view)
    }
    fn push_clip(&mut self, rect: [f32; 4]) -> Result<(), GuiClipRefusal> {
        self.clips.push(self.surface, rect)
    }
    fn pop_clip(&mut self) -> Result<(), GuiClipRefusal> {
        self.clips.pop(self.surface)
    }
    fn entity(&mut self, entity: &GuiEntity, view: GuiEntityView) -> Result<(), GuiEntityError> {
        self.game
            .native_draw_entity(&mut *self.surface, entity, view)
    }
    fn draw_menu_background(&mut self, rect: [f32; 4], in_world: bool) {
        self.game
            .native_draw_menu_background(&mut *self.surface, rect, in_world)
    }
    fn tiled_sprite(&mut self, sprite: &str, rect: [f32; 4], size: f32, argb: u32) -> bool {
        self.game
            .native_tiled_sprite(&mut *self.surface, sprite, rect, size, argb)
    }
    fn gui_draw_item_with(&mut self, stack: &ItemStack, at: [f32; 2], text: Option<&str>) {
        let Ok(stack) = sunrise_gameplay_common::game::api_values::stack_value(stack) else {
            return;
        };
        self.game
            .native_gui_draw_item_with(&mut *self.surface, &stack, at, text)
    }
}
/// [`BlockEntityDrawContext`] on the game's `crate::game::native_block_entity_draw::BlockEntityDraw`.
#[cfg(feature = "client")]
pub(crate) struct GameBlockEntityDrawContext<'a, 'g> {
    pub(crate) game: &'a crate::game::ClientGame<'g>,
    pub(crate) surface: &'a mut crate::game::native_block_entity_draw::BlockEntityDraw,
}

/// [`CameraDrawContext`] on the game's `crate::screen_effects::CameraQuads`.
#[cfg(feature = "client")]
pub(crate) struct GameCameraDrawContext<'a, 'g> {
    pub(crate) game: &'a crate::game::ClientGame<'g>,
    pub(crate) surface: &'a mut crate::screen_effects::CameraQuads,
}
/// [`WorldDrawContext`] on the game's `crate::world_geometry::WorldGeometry`.
#[cfg(feature = "client")]
pub(crate) struct GameWorldDrawContext<'a, 'g> {
    pub(crate) game: &'a crate::game::ClientGame<'g>,
    pub(crate) surface: &'a mut crate::world_geometry::WorldGeometry,
}
