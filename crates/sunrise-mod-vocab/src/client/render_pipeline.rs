//! Render pipelines a client mod registers as plain data, and the quads it draws with them.
use crate::{Identifier, WorldDepth};

/// A pipeline registered by `Hooks::register_render_pipelines`: a fragment shader from the
/// mod's assets and the fixed state it draws with. This is a restricted form: the mod supplies
/// the fragment stage and the host fixes the rest. Vanilla requires a vertex shader and a
/// primitive type; this API fixes both. The engine builds every GPU object.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub struct RenderPipeline {
    owner: String,
    /// The pipeline's own name (`mymod:glow`). Any unused [`Identifier`] is accepted.
    pub id: Identifier,
    /// The fragment shader (`mymod:glow` is `assets/mymod/shaders/pipeline/glow.wgsl`), as the
    /// mod's assets or a resource pack supply it.
    pub fragment: Identifier,
    /// What the pipeline's draws are made of.
    pub vertex_format: PipelineVertexFormat,
    /// How the shader's result joins the frame.
    pub blend: PipelineBlend,
    /// Whether nearer world surfaces hide the draw. No pipeline writes depth.
    pub depth: WorldDepth,
    /// Whether a resource load may go on without this pipeline (NeoForge's
    /// `registerOptionalPipeline`). A required pipeline (NeoForge's `registerPipeline`, the
    /// default) that a load cannot build fails the load: the game does not start, and a reload
    /// fails and keeps the previous resources. An optional one is logged once by id and resolves
    /// to [`RenderPipelineError::Invalid`] while the other pipelines draw.
    pub optional: bool,
    /// What the shader samples with `pipeline_texture(uv)`, if anything ([`Self::textured`]).
    /// `None`, the default, binds no texture: the shader reads only its input and `frame`.
    pub texture: Option<PipelineTexture>,
}
impl RenderPipeline {
    /// Describe a required pipeline that draws [`PipelineVertexFormat::PositionTexColor`]
    /// geometry.
    pub fn new(
        id: Identifier,
        fragment: Identifier,
        blend: PipelineBlend,
        depth: WorldDepth,
    ) -> Self {
        Self {
            owner: String::new(),
            id,
            fragment,
            vertex_format: PipelineVertexFormat::PositionTexColor,
            blend,
            depth,
            optional: false,
            texture: None,
        }
    }
    /// The owning mod, assigned by the host after registration.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Let a resource load that cannot build this pipeline go on without it (NeoForge's
    /// `registerOptionalPipeline`).
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// Let the shader sample `texture` with `pipeline_texture(uv)` (vanilla's `Sampler0`, as
    /// `RenderPipeline.withSampler` declares it). A texture the load refuses
    /// ([`PipelineTextureError`]) refuses the pipeline as a missing shader does: a required one
    /// fails the load, an optional one is logged with the named cause and resolves to
    /// [`RenderPipelineError::Invalid`].
    pub fn textured(mut self, texture: PipelineTexture) -> Self {
        self.texture = Some(texture);
        self
    }
}

/// A texture a pipeline's shader samples ([`RenderPipeline::textured`]): always a rectangle of
/// the atlas the host binds, never a texture of its own. `pipeline_texture(uv)` maps `fract(uv)`
/// into that rectangle and loads one texel by integer coordinates at mip level 0 (nearest, no
/// filtering, so no neighbouring sprite's texel enters): `uv` 0 to 1 spans it once and beyond
/// that it repeats. It returns linear light (the mod render colour rule) with the texel's alpha.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum PipelineTexture {
    /// A sprite of the game's block or item atlas, by its texture id: `powah:block/energy_charge`
    /// is `assets/powah/textures/block/energy_charge.png` as the `minecraft:blocks` atlas holds
    /// it, `minecraft:item/diamond` an item sprite. An animated sprite animates. A sprite in
    /// neither atlas refuses the pipeline ([`PipelineTextureError::NoSuchSprite`]). A texture
    /// file of the mod's own, packed into the runtime atlas's slot pool (#4438) and drawn the
    /// same way, follows once that pool exists.
    Sprite(Identifier),
}

/// Why a pipeline's texture was refused: the named cause its refusal is logged with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum PipelineTextureError {
    /// No sprite of that id in the block or item atlas.
    NoSuchSprite,
    /// No texture file of that id in the mod's assets or a resource pack.
    NoSuchTextureFile,
    /// The id names a file (a `textures/` path or a `.png` name), not a sprite key.
    NotASpriteKey,
    /// A texture file over the pool's cap of 1024 x 1024 pixels.
    OverCap,
    /// A texture file that isn't an image the game can decode.
    Undecodable,
    /// No free atlas slot, or over the mod's image budget.
    NoSlot,
}

impl std::fmt::Display for PipelineTextureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NoSuchSprite => "no such sprite in the block or item atlas",
            Self::NoSuchTextureFile => "no such texture file",
            Self::NotASpriteKey => "the id names a file, not a sprite key",
            Self::OverCap => "the texture file is over 1024 x 1024 pixels",
            Self::Undecodable => "the texture file cannot be decoded",
            Self::NoSlot => "no free atlas slot, or over the mod's image budget",
        })
    }
}

impl std::error::Error for PipelineTextureError {}

/// The vertex data a pipeline's draws carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum PipelineVertexFormat {
    /// A position, a texture coordinate and a colour: what [`PipelineQuad`] gives.
    PositionTexColor,
}

/// How a pipeline's fragments join the frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum PipelineBlend {
    /// Laid over the frame by the fragment's alpha, as [`crate::WorldQuad`] is.
    Translucent,
    /// Added to the frame, scaled by the fragment's alpha.
    Additive,
}

/// A registered pipeline as the current resources built it, resolved through
/// [`crate::RenderAssets::render_pipeline`]. Retain it between frames; resolve it again after a
/// successful resource reload.
// no wire: decoding from_parts rejects generation zero (an unpublished pipeline handle).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: private fields; only the engine resolves pipeline handles
pub struct RenderPipelineRef(pub(crate) u64, pub(crate) u32);

impl RenderPipelineRef {
    pub(crate) fn from_parts(generation: u64, slot: u32) -> Result<Self, &'static str> {
        if generation == 0 {
            return Err("unpublished pipeline generation");
        }
        Ok(Self(generation, slot))
    }
}

/// Why a pipeline could not be resolved or drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum RenderPipelineError {
    /// No mod registered a pipeline of this id.
    Unknown,
    /// The pipeline is registered, but the current resources give it no usable shader. The log
    /// names the pipeline and the reason.
    Invalid,
    /// A successful resource reload superseded the handle's generation.
    Stale,
}

/// A two-sided quad drawn by a mod pipeline. Corners run top left, bottom left, bottom right,
/// top right as seen from the front.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub struct PipelineQuad {
    /// Four world-space corners in blocks, before camera subtraction.
    pub corners: [[f64; 3]; 4],
    /// Each corner's texture coordinate, handed to the shader as given.
    pub uv: [[f32; 2]; 4],
    /// Packed 0xAARRGGBB colour in vanilla's space; the shader reads it as linear light.
    pub color: u32,
}
impl PipelineQuad {
    /// Describe a quad; the draw context validates it when submitting the batch.
    pub fn new(corners: [[f64; 3]; 4], uv: [[f32; 2]; 4], color: u32) -> Self {
        Self { corners, uv, color }
    }
    /// A quad whose texture coordinates span 0 to 1: (0, 0) at the top left corner and (1, 1)
    /// at the bottom right.
    pub fn spanning(corners: [[f64; 3]; 4], color: u32) -> Self {
        Self::new(
            corners,
            [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
            color,
        )
    }
}

/// Why a whole pipeline batch was refused; nothing from that batch was appended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum PipelineDrawError {
    /// The handle is stale, or names no usable pipeline.
    Pipeline(RenderPipelineError),
    /// A coordinate is not finite, or a camera-relative coordinate overflows f32.
    NonFinite,
    /// The batch would exceed this view's quad capacity, which pipeline quads share with
    /// [`crate::WorldDrawContext::quads`].
    CapacityExceeded,
}

/// Built-in required vanilla 26.3 render pipelines (`RenderPipelines.PIPELINES_BY_LOCATION`,
/// `minecraft:` namespace, sorted for binary search).
const BUILTIN_REQUIRED_RENDER_PIPELINES: &[&str] = &[
    "pipeline/animate_sprite_blit",
    "pipeline/animate_sprite_interpolate",
    "pipeline/armor_cutout_no_cull",
    "pipeline/armor_cutout_no_cull_glint",
    "pipeline/armor_decal_cutout_no_cull",
    "pipeline/armor_translucent",
    "pipeline/banner_pattern",
    "pipeline/beacon_beam_opaque",
    "pipeline/beacon_beam_translucent",
    "pipeline/blit_depth",
    "pipeline/blit_depth_bounds",
    "pipeline/blit_depth_during_depth_bounds",
    "pipeline/block_screen_effect",
    "pipeline/breeze_wind",
    "pipeline/celestial",
    "pipeline/clouds",
    "pipeline/crosshair",
    "pipeline/crumbling",
    "pipeline/cutout_block",
    "pipeline/cutout_terrain",
    "pipeline/cutout_terrain_multidraw",
    "pipeline/debug_filled_box",
    "pipeline/debug_points",
    "pipeline/debug_quads",
    "pipeline/debug_triangle_fan",
    "pipeline/dragon_rays",
    "pipeline/end_crystal_beam",
    "pipeline/end_gateway",
    "pipeline/end_portal",
    "pipeline/end_sky",
    "pipeline/energy_swirl",
    "pipeline/entity_cutout",
    "pipeline/entity_cutout_cull",
    "pipeline/entity_cutout_dissolve",
    "pipeline/entity_cutout_z_offset",
    "pipeline/entity_outline_blit",
    "pipeline/entity_shadow",
    "pipeline/entity_solid",
    "pipeline/entity_solid_glint",
    "pipeline/entity_solid_offset_forward",
    "pipeline/entity_translucent",
    "pipeline/entity_translucent_cull",
    "pipeline/entity_translucent_emissive",
    "pipeline/eyes",
    "pipeline/fire_screen_effect",
    "pipeline/flat_clouds",
    "pipeline/glint",
    "pipeline/gui",
    "pipeline/gui_invert",
    "pipeline/gui_nausea_overlay",
    "pipeline/gui_opaque_textured_background",
    "pipeline/gui_text",
    "pipeline/gui_text_grayscale",
    "pipeline/gui_text_highlight",
    "pipeline/gui_textured",
    "pipeline/gui_textured_premultiplied_alpha",
    "pipeline/integrate_depth",
    "pipeline/item_cutout",
    "pipeline/item_translucent",
    "pipeline/item_translucent_glint",
    "pipeline/leash",
    "pipeline/lightmap",
    "pipeline/lightning",
    "pipeline/lines",
    "pipeline/lines_depth_bias",
    "pipeline/lines_translucent",
    "pipeline/lines_translucent_no_depth_write",
    "pipeline/mojang_logo",
    "pipeline/oit_accumulate_beacon_beam",
    "pipeline/oit_accumulate_breeze_wind",
    "pipeline/oit_accumulate_clouds",
    "pipeline/oit_accumulate_crumbling",
    "pipeline/oit_accumulate_debug_filled_box",
    "pipeline/oit_accumulate_debug_points",
    "pipeline/oit_accumulate_debug_quads",
    "pipeline/oit_accumulate_debug_triangle_fan",
    "pipeline/oit_accumulate_dragon_rays",
    "pipeline/oit_accumulate_energy_swirl",
    "pipeline/oit_accumulate_entity",
    "pipeline/oit_accumulate_entity_cull",
    "pipeline/oit_accumulate_entity_emissive",
    "pipeline/oit_accumulate_entity_shadow",
    "pipeline/oit_accumulate_eyes",
    "pipeline/oit_accumulate_flat_clouds",
    "pipeline/oit_accumulate_item",
    "pipeline/oit_accumulate_item_glint",
    "pipeline/oit_accumulate_item_glint_special",
    "pipeline/oit_accumulate_lightning",
    "pipeline/oit_accumulate_lines_translucent",
    "pipeline/oit_accumulate_particle",
    "pipeline/oit_accumulate_terrain",
    "pipeline/oit_accumulate_terrain_multidraw",
    "pipeline/oit_accumulate_text",
    "pipeline/oit_accumulate_text_grayscale",
    "pipeline/oit_accumulate_text_grayscale_polygon_offset",
    "pipeline/oit_accumulate_text_polygon_offset",
    "pipeline/oit_accumulate_translucent_block",
    "pipeline/oit_accumulate_weather",
    "pipeline/oit_accumulate_world_border",
    "pipeline/oit_composite",
    "pipeline/oit_depth_bounds_beacon_beam",
    "pipeline/oit_depth_bounds_breeze_wind",
    "pipeline/oit_depth_bounds_clouds",
    "pipeline/oit_depth_bounds_crumbling",
    "pipeline/oit_depth_bounds_cull",
    "pipeline/oit_depth_bounds_debug_filled_box",
    "pipeline/oit_depth_bounds_debug_points",
    "pipeline/oit_depth_bounds_debug_quads",
    "pipeline/oit_depth_bounds_debug_triangle_fan",
    "pipeline/oit_depth_bounds_dragon_rays",
    "pipeline/oit_depth_bounds_energy_swirl",
    "pipeline/oit_depth_bounds_entity",
    "pipeline/oit_depth_bounds_entity_cull",
    "pipeline/oit_depth_bounds_entity_emissive",
    "pipeline/oit_depth_bounds_entity_shadow",
    "pipeline/oit_depth_bounds_eyes",
    "pipeline/oit_depth_bounds_flat_clouds",
    "pipeline/oit_depth_bounds_item",
    "pipeline/oit_depth_bounds_item_glint",
    "pipeline/oit_depth_bounds_item_glint_special",
    "pipeline/oit_depth_bounds_lightning",
    "pipeline/oit_depth_bounds_lines_translucent",
    "pipeline/oit_depth_bounds_particle",
    "pipeline/oit_depth_bounds_terrain",
    "pipeline/oit_depth_bounds_terrain_multidraw",
    "pipeline/oit_depth_bounds_text",
    "pipeline/oit_depth_bounds_text_grayscale",
    "pipeline/oit_depth_bounds_text_grayscale_polygon_offset",
    "pipeline/oit_depth_bounds_text_polygon_offset",
    "pipeline/oit_depth_bounds_translucent_block",
    "pipeline/oit_depth_bounds_weather",
    "pipeline/oit_depth_bounds_world_border",
    "pipeline/oit_transmittance_beacon_beam",
    "pipeline/oit_transmittance_breeze_wind",
    "pipeline/oit_transmittance_clouds",
    "pipeline/oit_transmittance_crumbling",
    "pipeline/oit_transmittance_debug_filled_box",
    "pipeline/oit_transmittance_debug_points",
    "pipeline/oit_transmittance_debug_quads",
    "pipeline/oit_transmittance_debug_triangle_fan",
    "pipeline/oit_transmittance_dragon_rays",
    "pipeline/oit_transmittance_energy_swirl",
    "pipeline/oit_transmittance_entity",
    "pipeline/oit_transmittance_entity_cull",
    "pipeline/oit_transmittance_entity_emissive",
    "pipeline/oit_transmittance_entity_shadow",
    "pipeline/oit_transmittance_eyes",
    "pipeline/oit_transmittance_flat_clouds",
    "pipeline/oit_transmittance_item",
    "pipeline/oit_transmittance_item_glint",
    "pipeline/oit_transmittance_item_glint_special",
    "pipeline/oit_transmittance_lightning",
    "pipeline/oit_transmittance_lines_translucent",
    "pipeline/oit_transmittance_particle",
    "pipeline/oit_transmittance_terrain",
    "pipeline/oit_transmittance_terrain_multidraw",
    "pipeline/oit_transmittance_text",
    "pipeline/oit_transmittance_text_grayscale",
    "pipeline/oit_transmittance_text_grayscale_polygon_offset",
    "pipeline/oit_transmittance_text_polygon_offset",
    "pipeline/oit_transmittance_translucent_block",
    "pipeline/oit_transmittance_weather",
    "pipeline/oit_transmittance_world_border",
    "pipeline/oit_water_mask",
    "pipeline/opaque_particle",
    "pipeline/outline_cull",
    "pipeline/outline_no_cull",
    "pipeline/panorama",
    "pipeline/secondary_block_outline",
    "pipeline/sky",
    "pipeline/solid_block",
    "pipeline/solid_terrain",
    "pipeline/solid_terrain_multidraw",
    "pipeline/stars",
    "pipeline/sunrise_sunset",
    "pipeline/text",
    "pipeline/text_grayscale",
    "pipeline/text_grayscale_polygon_offset",
    "pipeline/text_grayscale_see_through",
    "pipeline/text_polygon_offset",
    "pipeline/text_see_through",
    "pipeline/tracy_blit",
    "pipeline/translucent_block",
    "pipeline/translucent_particle",
    "pipeline/translucent_terrain",
    "pipeline/translucent_terrain_multidraw",
    "pipeline/vignette",
    "pipeline/water_mask",
    "pipeline/weather",
    "pipeline/world_border",
];

/// Built-in optional vanilla 26.3 render pipelines (`RenderPipelines.OPTIONAL_PIPELINES_BY_LOCATION`,
/// `minecraft:` namespace, sorted for binary search).
const BUILTIN_OPTIONAL_RENDER_PIPELINES: &[&str] =
    &["pipeline/wireframe", "pipeline/wireframe_multidraw"];

fn is_builtin_render_pipeline(pipeline: &RenderPipeline) -> bool {
    if pipeline.id.namespace() != "minecraft" {
        return false;
    }
    let table = if pipeline.optional {
        BUILTIN_OPTIONAL_RENDER_PIPELINES
    } else {
        BUILTIN_REQUIRED_RENDER_PIPELINES
    };
    table.binary_search(&pipeline.id.path()).is_ok()
}

/// What `owner` added to `pipelines` in `register_render_pipelines`, checked as it returns: a
/// pipeline whose id a built-in vanilla pipeline or an earlier registration took is logged with
/// the mod and dropped.
pub(crate) fn validate_render_pipelines(
    owner: &str,
    previous: &[RenderPipeline],
    pipelines: &mut Vec<RenderPipeline>,
) {
    let added = pipelines.split_off(previous.len().min(pipelines.len()));
    for mut pipeline in added {
        if is_builtin_render_pipeline(&pipeline) {
            log::warn!(
                "register_render_pipelines: mod {owner}: pipeline {} refused: collides with a built-in vanilla pipeline",
                pipeline.id
            );
        } else if pipelines.iter().any(|kept| kept.id == pipeline.id) {
            log::warn!(
                "register_render_pipelines: mod {owner}: pipeline {} refused: an earlier registration has that id",
                pipeline.id
            );
        } else {
            pipeline.owner = owner.to_owned();
            pipelines.push(pipeline);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pipeline(id: &str) -> RenderPipeline {
        let id = Identifier::parse(id).unwrap();
        RenderPipeline::new(
            id.clone(),
            id,
            PipelineBlend::Translucent,
            WorldDepth::Tested,
        )
    }
    fn ids(pipelines: &[RenderPipeline]) -> Vec<String> {
        pipelines.iter().map(|p| p.id.to_string()).collect()
    }

    #[test]
    fn a_mod_keeps_new_pipelines_across_namespaces_and_records_its_owner() {
        let previous = vec![pipeline("other:glow")];
        let mut pipelines = previous.clone();
        pipelines.extend([
            pipeline("mine:glow"),
            pipeline("other:beam"),
            pipeline("minecraft:custom_beam"),
            pipeline("mine:glow"),
            pipeline("mine:beam"),
        ]);
        validate_render_pipelines("owning_mod", &previous, &mut pipelines);
        assert_eq!(
            ids(&pipelines),
            [
                "other:glow",
                "mine:glow",
                "other:beam",
                "minecraft:custom_beam",
                "mine:beam",
            ]
        );
        assert_eq!(pipelines[1].owner(), "owning_mod");
        assert_eq!(pipelines[2].owner(), "owning_mod");
        assert_eq!(pipelines[3].owner(), "owning_mod");
    }

    #[test]
    fn a_mod_cannot_take_an_id_registered_before_it() {
        let previous = vec![pipeline("shared:glow")];
        let mut pipelines = previous.clone();
        pipelines.push(pipeline("shared:glow"));
        validate_render_pipelines("late", &previous, &mut pipelines);
        assert_eq!(pipelines, previous);
    }

    #[test]
    fn a_mod_cannot_take_a_builtin_vanilla_pipeline_id() {
        assert!(
            BUILTIN_REQUIRED_RENDER_PIPELINES
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
        assert!(
            BUILTIN_OPTIONAL_RENDER_PIPELINES
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
        let mut pipelines = vec![
            pipeline("minecraft:pipeline/gui"),
            pipeline("minecraft:pipeline/wireframe").optional(),
            pipeline("minecraft:pipeline/custom_beam"),
            pipeline("minecraft:pipeline/custom_beam"),
        ];
        validate_render_pipelines("maker", &[], &mut pipelines);
        assert_eq!(ids(&pipelines), ["minecraft:pipeline/custom_beam"]);
        assert_eq!(pipelines[0].owner(), "maker");
    }

    #[test]
    fn a_mod_can_register_more_than_vanillas_pipeline_count() {
        let mut pipelines: Vec<_> = (0..131).map(|i| pipeline(&format!("mine:p{i}"))).collect();
        validate_render_pipelines("mine", &[], &mut pipelines);
        assert_eq!(pipelines.len(), 131);
        assert_eq!(pipelines.last().unwrap().id.to_string(), "mine:p130");

        let previous = pipelines.clone();
        pipelines.push(pipeline("next:glow"));
        validate_render_pipelines("next", &previous, &mut pipelines);
        assert_eq!(pipelines.len(), 132);
    }
}
