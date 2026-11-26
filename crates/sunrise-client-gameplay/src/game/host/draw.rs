use mod_api::{prelude::*, *};
#[cfg(feature = "client")]
use sunrise_gameplay_common::game::api_values::TryIntoEngine;
#[cfg(any(test, feature = "test-support"))]
use sunrise_gameplay_common::game::api_values::TryIntoApi;

/// Borrow `minecraft:default` directly or format another font identifier.
pub fn font_id_str(id: &mod_api::Identifier) -> std::borrow::Cow<'static, str> {
    if id.namespace() == "minecraft" && id.path() == "default" {
        std::borrow::Cow::Borrowed(sunrise_client::font::DEFAULT)
    } else {
        std::borrow::Cow::Owned(id.to_string())
    }
}

/// Engine font view lent to [`ClientTooltipComponent`] and `render_tooltip_pre`.
pub struct EngineTooltipFont<'a> {
    pub(in crate::game) font: &'a sunrise_client::font::Font,
    pub id: &'a Identifier,
    pub(in crate::game) name: std::borrow::Cow<'static, str>,
}
/// `Font.lineHeight`: nine GUI pixels a line, before `scale`.
pub(in crate::game) const LINE_HEIGHT: f32 = 9.0;
/// The most lines one layout may have; [`DrawContext::text_lines`] refuses more
/// ([`TextLinesRefusal::TooManyLines`]) rather than answer a component of any size. The method's
/// doc names the same number: keep them in step.
pub(in crate::game) const MAX_LINES: usize = 512;
/// The most characters one layout's lines may hold together, before
/// [`TextLinesRefusal::TooManyGlyphs`]. The method's doc names the same number: keep them in step.
pub(in crate::game) const MAX_GLYPHS: usize = 8192;
/// Lays `text` out as a label's lines, as [`DrawContext::text_lines`] documents: the font's own
/// split wraps it at `max_width / scale`, the font measures each line, `align` places them inside
/// that wrapping width, and every dimension and position answered is scaled by `scale`.
#[cfg(any(test, feature = "test-support"))]
pub fn label_layout(
    font: &sunrise_client::font::Font,
    text: &sunrise_game_data::text::Text,
    max_width: f32,
    align: TextAlign,
    scale: f32,
) -> Result<TextLines, TextLinesRefusal> {
    label_layout_with(font, text, max_width, align, scale, |piece| {
        Ok(piece.try_into_api()?)
    })
}
pub fn label_layout_with(
    font: &sunrise_client::font::Font,
    text: &sunrise_game_data::text::Text,
    max_width: f32,
    align: TextAlign,
    scale: f32,
    mut to_api: impl FnMut(&sunrise_game_data::text::Text) -> Result<Text, TextLinesRefusal>,
) -> Result<TextLines, TextLinesRefusal> {
    // No position and no size to answer: a scale of zero or less, or one that is not a number,
    // refuses rather than answering every line at an undefined place.
    if !scale.is_finite() || scale <= 0.0 {
        return Err(TextLinesRefusal::ScaleInvalid);
    }
    // A width the game cannot measure in refuses too: it is not "no wrapping", which would answer
    // lines of any length in a box of no width.
    if !max_width.is_finite() {
        return Err(TextLinesRefusal::WidthNotFinite);
    }
    // Vanilla's one-pixel minimum (`StringSplitter.splitLines`): a finite box narrower than one
    // pixel wraps at one — and so does one `scale` makes narrower — rather than not wrapping.
    let wrap_width = max_width / scale;
    if !wrap_width.is_finite() {
        return Err(TextLinesRefusal::WidthNotFinite);
    }
    let wrap_width = wrap_width.max(1.0);
    let wrapped = font.split_formatted(text, wrap_width);
    if wrapped.len() > MAX_LINES {
        return Err(TextLinesRefusal::TooManyLines);
    }
    let mut lines = Vec::with_capacity(wrapped.len());
    let mut glyphs = 0usize;
    for (i, piece) in wrapped.into_iter().enumerate() {
        // The split keeps the legacy codes; applying them to each run's own style (a `§r` restores
        // the style the run carried out of the break) lets a line measure and draw the same way.
        let piece = piece.resolve_legacy();
        glyphs += piece.chars().count();
        if glyphs > MAX_GLYPHS {
            return Err(TextLinesRefusal::TooManyGlyphs);
        }
        let width = font.width_text(&piece);
        let x = match align {
            TextAlign::Left => 0.0,
            // `TextAlignment.CENTER.calculateLeft`: the half line width is rounded down.
            TextAlign::Centre => wrap_width / 2.0 - (width / 2.0).floor(),
            TextAlign::Right => wrap_width - width,
        };
        let at = [x * scale, i as f32 * LINE_HEIGHT * scale];
        lines.push(TextLine::new(to_api(&piece)?, at, width * scale));
    }
    let width = lines.iter().map(|line| line.width).fold(0.0, f32::max);
    let height = lines
        .last()
        .map_or(0.0, |line| line.at[1] + LINE_HEIGHT * scale);
    Ok(TextLines::new(lines, width, height))
}

/// [`DrawContext`] on the game's `crate::hud::HudBatch`: one per hook call.
#[cfg(feature = "client")]
pub struct GameDrawContext<'a, 'g> {
    pub(crate) game: &'a crate::game::ClientGame<'g>,
    pub surface: &'a mut sunrise_client::hud::HudBatch,
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
            sunrise_client::hud::rgba(r, g, b, a),
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
    pub(crate) surface: &'a mut sunrise_client::screen_effects::CameraQuads,
}
/// [`WorldDrawContext`] on the game's `crate::world_geometry::WorldGeometry`.
#[cfg(feature = "client")]
pub(crate) struct GameWorldDrawContext<'a, 'g> {
    pub(crate) game: &'a crate::game::ClientGame<'g>,
    pub(crate) surface: &'a mut sunrise_client::world_geometry::WorldGeometry,
}
