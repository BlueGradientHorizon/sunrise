//! Untextured, unlit geometry submitted for one world view.

/// Whether world geometry is hidden by nearer world surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum WorldDepth {
    /// Test against the world's depth, without writing depth.
    Tested,
    /// Draw over world surfaces, without testing or writing depth.
    ThroughWalls,
}

/// A world-space line, colored with 0xAARRGGBB and measured in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub struct WorldLine {
    /// Starting position in blocks, before camera subtraction.
    pub from: [f64; 3],
    /// Ending position in blocks, before camera subtraction.
    pub to: [f64; 3],
    /// Packed ARGB color, including alpha.
    pub color: u32,
    /// Finite, positive width in pixels.
    pub width: f32,
    /// Occlusion behavior.
    pub depth: WorldDepth,
}
impl WorldLine {
    /// Describe a line; the draw context validates it when submitting the batch.
    pub fn new(from: [f64; 3], to: [f64; 3], color: u32, width: f32, depth: WorldDepth) -> Self {
        Self {
            from,
            to,
            color,
            width,
            depth,
        }
    }
}

/// A two-sided, unlit quad. Corners run top left, bottom left, bottom right,
/// top right as seen from the front; alpha blends without writing depth.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub struct WorldQuad {
    /// Four world-space corners in blocks, before camera subtraction.
    pub corners: [[f64; 3]; 4],
    /// Packed 0xAARRGGBB color, including alpha.
    pub color: u32,
    /// Occlusion behavior.
    pub depth: WorldDepth,
}
impl WorldQuad {
    /// Describe a quad; the draw context validates it when submitting the batch.
    pub fn new(corners: [[f64; 3]; 4], color: u32, depth: WorldDepth) -> Self {
        Self {
            corners,
            color,
            depth,
        }
    }
}

/// Why a whole geometry batch or label was refused; nothing from that call was appended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum DrawError {
    /// A coordinate or width is not finite, or a camera-relative coordinate overflows f32.
    NonFinite,
    /// A finite line width is zero or negative.
    InvalidWidth,
    /// A label's text was refused at the boundary (`TextFormError`).
    TextRefused,
}

/// How [`WorldDrawContext::name_tag`](crate::WorldDrawContext::name_tag) draws a label.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub struct NameTagOptions {
    /// Draw it as vanilla draws a name that is not sneaking: opaque text in the world and a
    /// faded copy over everything, walls included. Off, it is drawn once as a sneaking name:
    /// the faded text, depth tested and translucent. The faded text's alpha follows the
    /// text background opacity unless `faded_alpha` is set: 255 times the larger of
    /// (opacity + 0.75) / 2 and 0.5, 0x7F at the default.
    pub see_through: bool,
    /// Put the translucent black backdrop behind the faded text, at the player's text
    /// background opacity (`Options.getBackgroundOpacity(0.25)`: 0x3F at the default).
    pub background: bool,
    /// The light at the label, as a name tag reads its entity's: sky light in the high four
    /// bits, block light in the low four (`0xF0` is full sky light and no block light; `0x0F`
    /// full block light under no sky). Without `see_through` the label is drawn in exactly
    /// this light; with it, the copy in the world raises each of the two levels to at least 2
    /// (`lightCoordsWithEmission(light, 2)`) and the copy over everything ignores it.
    pub light: u8,
    /// The alpha byte of the faded text, whichever copy it is: the see-through copy, or the
    /// single copy when `see_through` is off (`Font.drawInBatch` takes the colour's alpha from
    /// its caller). `None` keeps the value derived from the text background opacity above.
    /// Fragments under alpha 0.1 are discarded as for every name tag copy, so an alpha under 26
    /// draws no text. The backdrop is unchanged.
    pub faded_alpha: Option<u8>,
    /// With `see_through`, draw the opaque copy in the world in `light` exactly, without
    /// raising each level to at least 2. No effect without `see_through`, which already draws
    /// in `light` as given.
    pub exact_light: bool,
}
impl NameTagOptions {
    /// Describe a label's drawing; the faded text's alpha follows the text background opacity
    /// and the light of the copy in the world is raised to at least 2.
    pub fn new(see_through: bool, background: bool, light: u8) -> Self {
        Self {
            see_through,
            background,
            light,
            faded_alpha: None,
            exact_light: false,
        }
    }

    /// Draw the faded text at alpha `alpha` ([`NameTagOptions::faded_alpha`]).
    pub fn with_faded_alpha(mut self, alpha: u8) -> Self {
        self.faded_alpha = Some(alpha);
        self
    }

    /// Draw the opaque copy in `light` exactly ([`NameTagOptions::exact_light`]).
    pub fn with_exact_light(mut self) -> Self {
        self.exact_light = true;
        self
    }
}

/// Where in the world's frame a stage's geometry is drawn: NeoForge 26.3's
/// `RenderLevelStageEvent` stages Sunrise draws, in the order the frame reaches them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum RenderLevelStage {
    /// After the sky, sun, moon and stars, before any terrain (`AfterSky`): behind
    /// everything else the world draws, for backdrops and atmosphere.
    AfterSky,
    /// After solid and cutout terrain, before entities and block entities
    /// (`AfterOpaqueBlocks`).
    AfterOpaqueBlocks,
    /// After translucent terrain such as water and stained glass, before translucent
    /// particles (`AfterTranslucentBlocks`).
    AfterTranslucentBlocks,
    /// After rain and snow, before the world border (`AfterWeather`).
    AfterWeather,
    /// After the rest of the world, before the first-person hand (`AfterLevel`): where
    /// `submit_custom_geometry` and the debug renderers draw.
    AfterLevel,
}
impl RenderLevelStage {
    /// Every stage, in the order the frame reaches them.
    pub const ALL: [Self; 5] = [
        Self::AfterSky,
        Self::AfterOpaqueBlocks,
        Self::AfterTranslucentBlocks,
        Self::AfterWeather,
        Self::AfterLevel,
    ];
}

// no wire: in-process frame section view lent during world rendering.
/// One visible chunk section in the current world view (NeoForge 26.3's `IRenderableSection`).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct RenderableSection {
    /// World-space block coordinates of the section's minimum corner.
    pub origin: [i32; 3],
    /// World-space axis-aligned bounding box `[min, max]` of the section in blocks.
    pub bounds: [[f64; 3]; 2],
    /// Whether the compiled section currently holds no rendered geometry.
    pub empty: bool,
}
impl RenderableSection {
    /// Describe one visible chunk section in the current frame.
    pub const fn new(origin: [i32; 3], bounds: [[f64; 3]; 2], empty: bool) -> Self {
        Self {
            origin,
            bounds,
            empty,
        }
    }
}

// no wire: borrows frame camera state and visible sections for the duration of a render callback.
/// Read-only frame state and visible chunk sections lent for one world view.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct WorldRenderView<'a> {
    /// Frame camera state (position, eye height, rotation, projection and dimension).
    pub camera: &'a crate::Camera,
    /// World partial tick (`alpha`), distinct from `camera.partial_tick` while paused or frozen.
    pub partial_tick: f32,
    /// Level game time in ticks.
    pub game_time: i64,
    /// Whether vanilla 26.3 order-independent transparency (improved transparency) is active this frame.
    pub improved_transparency: bool,
    /// Visible chunk sections in the current view (NeoForge 26.3 `IRenderableSection` list).
    pub sections: &'a [RenderableSection],
}
impl<'a> WorldRenderView<'a> {
    /// Borrow the frame camera and visible sections for one world view.
    pub fn new(
        camera: &'a crate::Camera,
        partial_tick: f32,
        game_time: i64,
        improved_transparency: bool,
        sections: &'a [RenderableSection],
    ) -> Self {
        Self {
            camera,
            partial_tick,
            game_time,
            improved_transparency,
            sections,
        }
    }

    /// Camera position in world-space blocks.
    pub fn camera_position(&self) -> [f64; 3] {
        self.camera.position
    }

    /// Camera-relative model-view matrix in column-major order.
    pub fn model_view_matrix(&self) -> [f32; 16] {
        self.camera.view
    }

    /// Reversed-Z projection matrix in column-major order.
    pub fn projection_matrix(&self) -> [f32; 16] {
        self.camera.projection
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    static PREPARE_CALLS: AtomicUsize = AtomicUsize::new(0);
    struct Listener;
    #[mod_api::__subscriptions]
    impl Hooks for Listener {
        fn prepare_render_buffers(
            &self,
            _: &dyn ClientHost,
            event: &mut PrepareRenderBuffersEvent<'_>,
        ) {
            assert_eq!(event.view.game_time, 42);
            assert_eq!(event.view.partial_tick, 0.5);
            assert!(event.view.improved_transparency);
            assert_eq!(event.view.camera_position(), [1.0, 2.0, 3.0]);
            assert_eq!(event.view.sections.len(), 1);
            assert_eq!(event.view.sections[0].origin, [16, 64, -32]);
            PREPARE_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        fn submit_custom_geometry(
            &self,
            _: &dyn ClientHost,
            event: &mut SubmitCustomGeometryEvent<'_>,
        ) {
            assert_eq!(event.view.game_time, 42);
            assert_eq!(event.view.camera_position(), [1.0, 2.0, 3.0]);
            assert_eq!(event.view.sections.len(), 1);
            CALLS.fetch_add(1, Ordering::Relaxed);
        }
    }
    struct Empty;
    #[mod_api::__subscriptions]
    impl Hooks for Empty {}
    impl Host for Empty {}
    mod_api::impl_host_areas!(Empty);
    impl ClientHost for Empty {}
    mod_api::impl_client_host_areas!(Empty);
    impl WorldDrawContext for Empty {}
    #[test]
    fn world_geometry_dispatch_skips_unsubscribed_and_disabled_mods() {
        install(
            vec![
                ModEntry::new("geometry", Box::new(Listener)),
                ModEntry::new("empty", Box::new(Empty)),
            ]
            .into(),
        );
        let camera = Camera {
            position: [1.0, 2.0, 3.0],
            eye_height: 1.62,
            partial_tick: 0.25,
            yaw: 15.0,
            pitch: -10.0,
            fov_degrees: 70.0,
            near: 0.05,
            far: 128.0,
            view: [0.0; 16],
            projection: [0.0; 16],
            dimension: "minecraft:overworld".into(),
        };
        let sections = [RenderableSection::new(
            [16, 64, -32],
            [[16.0, 64.0, -32.0], [32.0, 80.0, -16.0]],
            false,
        )];
        let view = WorldRenderView::new(&camera, 0.5, 42, true, &sections);
        dispatch_prepare_render_buffers(&Empty, &view);
        dispatch_submit_custom_geometry(&Empty, &view, &mut Empty);
        assert_eq!(PREPARE_CALLS.load(Ordering::Relaxed), 1);
        assert_eq!(CALLS.load(Ordering::Relaxed), 1);
        assert!(set_mod_enabled("geometry", false));
        dispatch_prepare_render_buffers(&Empty, &view);
        dispatch_submit_custom_geometry(&Empty, &view, &mut Empty);
        assert_eq!(PREPARE_CALLS.load(Ordering::Relaxed), 1);
        assert_eq!(CALLS.load(Ordering::Relaxed), 1);
    }
}
