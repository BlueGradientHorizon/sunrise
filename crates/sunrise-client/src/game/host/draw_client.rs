#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::draw::EngineTooltipFont;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::draw::GameBlockEntityDrawContext;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::draw::GameCameraDrawContext;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::draw::GameDrawContext;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::draw::GameWorldDrawContext;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::draw::LINE_HEIGHT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::draw::MAX_GLYPHS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::draw::MAX_LINES;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::draw::font_id_str;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::draw::label_layout;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::draw::label_layout_with;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::draw::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{
    Game,
    api_values::{TryIntoApi, TryIntoEngine},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{prelude::*, *};

impl BlockEntityDrawContext for GameBlockEntityDrawContext<'_, '_> {
    fn push(&mut self) {
        self.game.native_draw_push(&mut *self.surface)
    }
    fn pop(&mut self) {
        self.game.native_draw_pop(&mut *self.surface)
    }
    fn translate(&mut self, by: [f32; 3]) {
        self.game.native_draw_translate(&mut *self.surface, by)
    }
    fn rotate_about(&mut self, axis: [f32; 3], degrees: f32, pivot: [f32; 3]) {
        self.game
            .native_draw_rotate_about(&mut *self.surface, axis, degrees, pivot)
    }
    fn scale(&mut self, by: [f32; 3]) {
        self.game.native_draw_scale(&mut *self.surface, by)
    }
    fn keep(&mut self) {
        self.game.native_draw_keep(&mut *self.surface)
    }
    fn partial(&self) -> f32 {
        self.game.native_draw_partial(&*self.surface)
    }
    fn game_time(&self) -> i64 {
        self.game.native_draw_game_time(&*self.surface)
    }
    fn animation_time(&self) -> f64 {
        self.game.native_draw_animation_time(&*self.surface)
    }
    fn light(&self) -> u32 {
        self.game.native_draw_light(&*self.surface)
    }
    fn set_light(&mut self, light: u32) {
        self.game.native_draw_set_light(&mut *self.surface, light)
    }
    fn level_light(&mut self) {
        self.game.native_draw_level_light(&mut *self.surface)
    }
    fn unlit(&mut self, on: bool) {
        self.game.native_draw_unlit(&mut *self.surface, on)
    }
    fn quad(&mut self, sprite: &str, corners: [[f32; 3]; 4], uv: [f32; 4], tint: u32) -> bool {
        self.game
            .native_draw_quad(&mut *self.surface, sprite, corners, uv, tint)
    }
    fn cube(
        &mut self,
        from: [f32; 3],
        to: [f32; 3],
        faces: [Option<CubeFace<'_>>; 6],
        tint: u32,
    ) -> bool {
        self.game
            .native_draw_cube(&mut *self.surface, from, to, faces, tint)
    }
    fn block(&mut self, block: BlockKey, properties: &[(&str, &str)]) -> bool {
        let Some(block) = mod_api::__private::block_key_id(block) else {
            return false;
        };
        self.game
            .native_draw_block(&mut *self.surface, block, properties)
    }
    fn item(
        &mut self,
        stack: &ItemStack,
        display: ItemDisplayContext,
        owner_yaw: Option<f32>,
        seed: i32,
    ) -> bool {
        let Ok(stack) = crate::game::api_values::stack_value(stack) else {
            return false;
        };
        let Ok(display) = display.try_into_engine(self.game.items()) else {
            return false;
        };
        self.game
            .native_draw_item(&mut *self.surface, &stack, display, owner_yaw, seed)
    }
    fn spin(&mut self, axis: [f32; 3], pivot: [f32; 3], degrees_per_tick: f32, phase: f32) {
        self.game
            .native_draw_spin(&mut *self.surface, axis, pivot, degrees_per_tick, phase)
    }
    fn still(&mut self) {
        self.game.native_draw_still(&mut *self.surface)
    }
    fn model(&mut self, model: ModelRef, tint: u32) -> Result<(), ModelError> {
        self.game.native_draw_model(&mut *self.surface, model, tint)
    }
    fn text(
        &mut self,
        text: &Text,
        at: [f32; 2],
        color: u32,
        glowing: bool,
        outline: Option<u32>,
    ) -> Result<f32, mod_api::text::TextFormError> {
        let text = sunrise_client_lib::api_text::to_engine(text)?;
        Ok(self
            .game
            .native_draw_text(&mut *self.surface, &text, at, color, glowing, outline))
    }
    fn terrain_model(
        &mut self,
        model: ModelRef,
        options: TerrainModelOptions<'_>,
    ) -> Result<(), TerrainModelError> {
        self.game
            .native_draw_terrain_model(&mut *self.surface, model, options)
    }
    fn text_width(&self, text: &Text) -> f32 {
        let text = sunrise_client_lib::api_text::to_engine_inprocess(text);
        self.game.native_draw_text_width(&*self.surface, &text)
    }
    fn model_part(&mut self, part: &ModelPart, sprite: &str, tint: u32) -> bool {
        self.game
            .native_draw_model_part(&mut *self.surface, part, sprite, tint)
    }
    fn fluid_box(
        &mut self,
        stack: &FluidStack,
        bounds: [f32; 6],
        options: FluidBoxOptions,
    ) -> Result<(), FluidBoxError> {
        self.game
            .native_draw_fluid_box(&mut *self.surface, stack, bounds, options)
    }
}

impl CameraDrawContext for GameCameraDrawContext<'_, '_> {
    fn quad(&mut self, corners: [[f32; 3]; 4], argb: u32) -> bool {
        self.game
            .native_camera_quad(&mut *self.surface, corners, argb)
    }
    fn sprite(&mut self, name: &str, corners: [[f32; 3]; 4], uv: [[f32; 2]; 4], argb: u32) -> bool {
        self.game
            .native_camera_sprite(&mut *self.surface, name, corners, uv, argb)
    }
}

impl Drop for GameDrawContext<'_, '_> {
    fn drop(&mut self) {
        self.clips.close_all(self.surface);
    }
}

impl HandDrawContext for GameBlockEntityDrawContext<'_, '_> {
    fn render_arm(&mut self, arm: MainHand) {
        if let Some(normals) = self.surface.normal_pose() {
            self.game.submit_first_person_arm(
                arm == MainHand::Left,
                (self.surface.pose(), normals),
                self.surface.light(),
                self.surface.vertices_mut(),
            );
        }
    }
}

impl TooltipFont for EngineTooltipFont<'_> {
    fn id(&self) -> &Identifier {
        self.id
    }
    fn line_height(&self) -> i32 {
        9
    }
    fn width(&self, text: &Text) -> Result<i32, TextFormError> {
        let text: crate::text::Text = text.try_into_engine(&())?;
        Ok(self.font.width_text_with_font(&text, self.name()) as i32)
    }
    fn split(&self, text: &Text, max_width: i32) -> Result<Vec<Text>, TextFormError> {
        let text: crate::text::Text = text.try_into_engine(&())?;
        self.font
            .split_formatted_with_font(&text, max_width as f32, self.name())
            .into_iter()
            .map(|line| line.try_into_api())
            .collect()
    }
}

impl WorldDrawContext for GameWorldDrawContext<'_, '_> {
    fn draw_instance_group(&mut self, group: InstanceGroupRef) -> Result<(), GroupError> {
        self.game.draw_instance_group(self.surface, group)
    }
    fn lines(&mut self, lines: &[WorldLine]) -> Result<(), DrawError> {
        self.game.native_world_draw_lines(&mut *self.surface, lines)
    }
    fn quads(&mut self, quads: &[WorldQuad]) -> Result<(), DrawError> {
        self.game.native_world_draw_quads(&mut *self.surface, quads)
    }
    fn pipeline_quads(
        &mut self,
        pipeline: RenderPipelineRef,
        quads: &[PipelineQuad],
    ) -> Result<(), PipelineDrawError> {
        self.game
            .native_world_draw_pipeline_quads(&mut *self.surface, pipeline, quads)
    }
    fn name_tag(
        &mut self,
        at: [f64; 3],
        text: &Text,
        options: NameTagOptions,
    ) -> Result<(), DrawError> {
        self.game
            .native_world_name_tag(&mut *self.surface, at, text, options)
    }
    fn usage(&self) -> WorldDrawUsage {
        self.game.native_world_draw_usage(&*self.surface)
    }
    fn draw_model_instances(
        &mut self,
        model: ModelRef,
        instances: &[ModelInstance],
    ) -> Result<(), InstanceError> {
        self.game
            .native_world_draw_model_instances(&mut *self.surface, model, instances)
    }
}

impl<'a, 'g> GameBlockEntityDrawContext<'a, 'g> {
    pub(crate) fn new(
        game: &'a Game<'g>,
        surface: &'a mut crate::game::native_block_entity_draw::BlockEntityDraw,
    ) -> Self {
        Self { game, surface }
    }
}

impl<'a, 'g> GameCameraDrawContext<'a, 'g> {
    pub(crate) fn new(
        game: &'a Game<'g>,
        surface: &'a mut crate::screen_effects::CameraQuads,
    ) -> Self {
        Self { game, surface }
    }
}

impl<'a, 'g> GameDrawContext<'a, 'g> {
    pub(crate) fn new(game: &'a Game<'g>, surface: &'a mut crate::hud::HudBatch) -> Self {
        Self {
            game,
            surface,
            clips: Default::default(),
            tooltip_font: None,
        }
    }
    /// Lend a fresh clip stack and isolate the drawable tail of one listener.
    pub(crate) fn stage(
        game: &Game<'_>,
        surface: &mut crate::hud::HudBatch,
        listen: &mut dyn FnMut(&mut dyn DrawContext) -> bool,
    ) {
        let counts = (
            surface.vertices.len(),
            surface.pictures.len(),
            surface.blends.len(),
            surface.inline_profiles.len(),
        );
        let last_blend_end = surface.blends.last().map(|(range, _)| range.end);
        let accepted = listen(&mut GameDrawContext::new(game, surface));
        if !accepted {
            surface.vertices.truncate(counts.0);
            surface.pictures.truncate(counts.1);
            surface.blends.truncate(counts.2);
            surface.inline_profiles.truncate(counts.3);
            if let Some((last, end)) = surface.blends.last_mut().zip(last_blend_end) {
                last.0.end = end;
            }
            // Keep glyph uploads: font extraction already cached those atlas locations.
            // They do not draw anything without an accepted vertex referencing them.
        }
    }
    pub(in crate::game) fn with_active_tooltip_font(
        mut self,
        font: &'a EngineTooltipFont<'a>,
    ) -> Self {
        self.tooltip_font = Some(font);
        self
    }
}

impl<'a, 'g> GameWorldDrawContext<'a, 'g> {
    pub(crate) fn new(
        game: &'a Game<'g>,
        surface: &'a mut crate::world_geometry::WorldGeometry,
    ) -> Self {
        Self { game, surface }
    }
}

impl<'a> EngineTooltipFont<'a> {
    pub(in crate::game) fn name(&self) -> &str {
        &self.name
    }
    pub(in crate::game) fn new(font: &'a sunrise_client::font::Font, id: &'a Identifier) -> Self {
        Self {
            font,
            id,
            name: font_id_str(id),
        }
    }
}
