//! Render pipelines mods register (`Hooks::register_render_pipelines`, NeoForge's
//! `RegisterRenderPipelinesEvent`): the registrations, kept once at start, and what each
//! resource load makes of them. A load reads every registered pipeline's fragment file
//! (`assets/<namespace>/shaders/pipeline/<path>.wgsl`), puts Sunrise's prelude before it and its
//! entry points after it, and checks the whole shader, so the renderer (`gfx::mod_pipelines`) is
//! only ever handed WGSL that compiles. A required pipeline the load cannot build fails the load;
//! an optional one is logged and left out. A frame reads none of this by name.
//! The shader's composition and checks live in `sunrise_shader_api::mod_pipeline`, shared with
//! `cargo sunrise-mod check`.
use mod_api::{
    Identifier, PipelineBlend, PipelineKind, PipelineTexture, PipelineTextureError, RenderPipeline,
    RenderPipelineError, RenderPipelineRef, WorldDepth,
};
use std::collections::HashMap;
use std::sync::OnceLock;
use sunrise_shader_api::mod_pipeline::{FOLDER, compose_with};

static REGISTERED: OnceLock<Vec<RenderPipeline>> = OnceLock::new();

/// Authoritative vanilla 26.3 required render pipeline locations (`PIPELINES_BY_LOCATION`),
/// sorted lexicographically for binary search.
const VANILLA_REQUIRED_PIPELINES: &[&str] = &[
    "minecraft:pipeline/animate_sprite_blit",
    "minecraft:pipeline/animate_sprite_interpolate",
    "minecraft:pipeline/armor_cutout_no_cull",
    "minecraft:pipeline/armor_cutout_no_cull_glint",
    "minecraft:pipeline/armor_decal_cutout_no_cull",
    "minecraft:pipeline/armor_translucent",
    "minecraft:pipeline/banner_pattern",
    "minecraft:pipeline/beacon_beam_opaque",
    "minecraft:pipeline/beacon_beam_translucent",
    "minecraft:pipeline/blit_depth",
    "minecraft:pipeline/blit_depth_bounds",
    "minecraft:pipeline/blit_depth_during_depth_bounds",
    "minecraft:pipeline/block_screen_effect",
    "minecraft:pipeline/breeze_wind",
    "minecraft:pipeline/celestial",
    "minecraft:pipeline/clouds",
    "minecraft:pipeline/crosshair",
    "minecraft:pipeline/crumbling",
    "minecraft:pipeline/cutout_block",
    "minecraft:pipeline/cutout_terrain",
    "minecraft:pipeline/cutout_terrain_multidraw",
    "minecraft:pipeline/debug_filled_box",
    "minecraft:pipeline/debug_points",
    "minecraft:pipeline/debug_quads",
    "minecraft:pipeline/debug_triangle_fan",
    "minecraft:pipeline/dragon_rays",
    "minecraft:pipeline/end_crystal_beam",
    "minecraft:pipeline/end_gateway",
    "minecraft:pipeline/end_portal",
    "minecraft:pipeline/end_sky",
    "minecraft:pipeline/energy_swirl",
    "minecraft:pipeline/entity_cutout",
    "minecraft:pipeline/entity_cutout_cull",
    "minecraft:pipeline/entity_cutout_dissolve",
    "minecraft:pipeline/entity_cutout_z_offset",
    "minecraft:pipeline/entity_outline_blit",
    "minecraft:pipeline/entity_shadow",
    "minecraft:pipeline/entity_solid",
    "minecraft:pipeline/entity_solid_glint",
    "minecraft:pipeline/entity_solid_offset_forward",
    "minecraft:pipeline/entity_translucent",
    "minecraft:pipeline/entity_translucent_cull",
    "minecraft:pipeline/entity_translucent_emissive",
    "minecraft:pipeline/eyes",
    "minecraft:pipeline/fire_screen_effect",
    "minecraft:pipeline/flat_clouds",
    "minecraft:pipeline/glint",
    "minecraft:pipeline/gui",
    "minecraft:pipeline/gui_invert",
    "minecraft:pipeline/gui_nausea_overlay",
    "minecraft:pipeline/gui_opaque_textured_background",
    "minecraft:pipeline/gui_text",
    "minecraft:pipeline/gui_text_grayscale",
    "minecraft:pipeline/gui_text_highlight",
    "minecraft:pipeline/gui_textured",
    "minecraft:pipeline/gui_textured_premultiplied_alpha",
    "minecraft:pipeline/integrate_depth",
    "minecraft:pipeline/item_cutout",
    "minecraft:pipeline/item_translucent",
    "minecraft:pipeline/item_translucent_glint",
    "minecraft:pipeline/leash",
    "minecraft:pipeline/lightmap",
    "minecraft:pipeline/lightning",
    "minecraft:pipeline/lines",
    "minecraft:pipeline/lines_depth_bias",
    "minecraft:pipeline/lines_translucent",
    "minecraft:pipeline/lines_translucent_no_depth_write",
    "minecraft:pipeline/mojang_logo",
    "minecraft:pipeline/oit_accumulate_beacon_beam",
    "minecraft:pipeline/oit_accumulate_breeze_wind",
    "minecraft:pipeline/oit_accumulate_clouds",
    "minecraft:pipeline/oit_accumulate_crumbling",
    "minecraft:pipeline/oit_accumulate_debug_filled_box",
    "minecraft:pipeline/oit_accumulate_debug_points",
    "minecraft:pipeline/oit_accumulate_debug_quads",
    "minecraft:pipeline/oit_accumulate_debug_triangle_fan",
    "minecraft:pipeline/oit_accumulate_dragon_rays",
    "minecraft:pipeline/oit_accumulate_energy_swirl",
    "minecraft:pipeline/oit_accumulate_entity",
    "minecraft:pipeline/oit_accumulate_entity_cull",
    "minecraft:pipeline/oit_accumulate_entity_emissive",
    "minecraft:pipeline/oit_accumulate_entity_shadow",
    "minecraft:pipeline/oit_accumulate_eyes",
    "minecraft:pipeline/oit_accumulate_flat_clouds",
    "minecraft:pipeline/oit_accumulate_item",
    "minecraft:pipeline/oit_accumulate_item_glint",
    "minecraft:pipeline/oit_accumulate_item_glint_special",
    "minecraft:pipeline/oit_accumulate_lightning",
    "minecraft:pipeline/oit_accumulate_lines_translucent",
    "minecraft:pipeline/oit_accumulate_particle",
    "minecraft:pipeline/oit_accumulate_terrain",
    "minecraft:pipeline/oit_accumulate_terrain_multidraw",
    "minecraft:pipeline/oit_accumulate_text",
    "minecraft:pipeline/oit_accumulate_text_grayscale",
    "minecraft:pipeline/oit_accumulate_text_grayscale_polygon_offset",
    "minecraft:pipeline/oit_accumulate_text_polygon_offset",
    "minecraft:pipeline/oit_accumulate_translucent_block",
    "minecraft:pipeline/oit_accumulate_weather",
    "minecraft:pipeline/oit_accumulate_world_border",
    "minecraft:pipeline/oit_composite",
    "minecraft:pipeline/oit_depth_bounds_beacon_beam",
    "minecraft:pipeline/oit_depth_bounds_breeze_wind",
    "minecraft:pipeline/oit_depth_bounds_clouds",
    "minecraft:pipeline/oit_depth_bounds_crumbling",
    "minecraft:pipeline/oit_depth_bounds_cull",
    "minecraft:pipeline/oit_depth_bounds_debug_filled_box",
    "minecraft:pipeline/oit_depth_bounds_debug_points",
    "minecraft:pipeline/oit_depth_bounds_debug_quads",
    "minecraft:pipeline/oit_depth_bounds_debug_triangle_fan",
    "minecraft:pipeline/oit_depth_bounds_dragon_rays",
    "minecraft:pipeline/oit_depth_bounds_energy_swirl",
    "minecraft:pipeline/oit_depth_bounds_entity",
    "minecraft:pipeline/oit_depth_bounds_entity_cull",
    "minecraft:pipeline/oit_depth_bounds_entity_emissive",
    "minecraft:pipeline/oit_depth_bounds_entity_shadow",
    "minecraft:pipeline/oit_depth_bounds_eyes",
    "minecraft:pipeline/oit_depth_bounds_flat_clouds",
    "minecraft:pipeline/oit_depth_bounds_item",
    "minecraft:pipeline/oit_depth_bounds_item_glint",
    "minecraft:pipeline/oit_depth_bounds_item_glint_special",
    "minecraft:pipeline/oit_depth_bounds_lightning",
    "minecraft:pipeline/oit_depth_bounds_lines_translucent",
    "minecraft:pipeline/oit_depth_bounds_particle",
    "minecraft:pipeline/oit_depth_bounds_terrain",
    "minecraft:pipeline/oit_depth_bounds_terrain_multidraw",
    "minecraft:pipeline/oit_depth_bounds_text",
    "minecraft:pipeline/oit_depth_bounds_text_grayscale",
    "minecraft:pipeline/oit_depth_bounds_text_grayscale_polygon_offset",
    "minecraft:pipeline/oit_depth_bounds_text_polygon_offset",
    "minecraft:pipeline/oit_depth_bounds_translucent_block",
    "minecraft:pipeline/oit_depth_bounds_weather",
    "minecraft:pipeline/oit_depth_bounds_world_border",
    "minecraft:pipeline/oit_transmittance_beacon_beam",
    "minecraft:pipeline/oit_transmittance_breeze_wind",
    "minecraft:pipeline/oit_transmittance_clouds",
    "minecraft:pipeline/oit_transmittance_crumbling",
    "minecraft:pipeline/oit_transmittance_debug_filled_box",
    "minecraft:pipeline/oit_transmittance_debug_points",
    "minecraft:pipeline/oit_transmittance_debug_quads",
    "minecraft:pipeline/oit_transmittance_debug_triangle_fan",
    "minecraft:pipeline/oit_transmittance_dragon_rays",
    "minecraft:pipeline/oit_transmittance_energy_swirl",
    "minecraft:pipeline/oit_transmittance_entity",
    "minecraft:pipeline/oit_transmittance_entity_cull",
    "minecraft:pipeline/oit_transmittance_entity_emissive",
    "minecraft:pipeline/oit_transmittance_entity_shadow",
    "minecraft:pipeline/oit_transmittance_eyes",
    "minecraft:pipeline/oit_transmittance_flat_clouds",
    "minecraft:pipeline/oit_transmittance_item",
    "minecraft:pipeline/oit_transmittance_item_glint",
    "minecraft:pipeline/oit_transmittance_item_glint_special",
    "minecraft:pipeline/oit_transmittance_lightning",
    "minecraft:pipeline/oit_transmittance_lines_translucent",
    "minecraft:pipeline/oit_transmittance_particle",
    "minecraft:pipeline/oit_transmittance_terrain",
    "minecraft:pipeline/oit_transmittance_terrain_multidraw",
    "minecraft:pipeline/oit_transmittance_text",
    "minecraft:pipeline/oit_transmittance_text_grayscale",
    "minecraft:pipeline/oit_transmittance_text_grayscale_polygon_offset",
    "minecraft:pipeline/oit_transmittance_text_polygon_offset",
    "minecraft:pipeline/oit_transmittance_translucent_block",
    "minecraft:pipeline/oit_transmittance_weather",
    "minecraft:pipeline/oit_transmittance_world_border",
    "minecraft:pipeline/oit_water_mask",
    "minecraft:pipeline/opaque_particle",
    "minecraft:pipeline/outline_cull",
    "minecraft:pipeline/outline_no_cull",
    "minecraft:pipeline/panorama",
    "minecraft:pipeline/secondary_block_outline",
    "minecraft:pipeline/sky",
    "minecraft:pipeline/solid_block",
    "minecraft:pipeline/solid_terrain",
    "minecraft:pipeline/solid_terrain_multidraw",
    "minecraft:pipeline/stars",
    "minecraft:pipeline/sunrise_sunset",
    "minecraft:pipeline/text",
    "minecraft:pipeline/text_grayscale",
    "minecraft:pipeline/text_grayscale_polygon_offset",
    "minecraft:pipeline/text_grayscale_see_through",
    "minecraft:pipeline/text_polygon_offset",
    "minecraft:pipeline/text_see_through",
    "minecraft:pipeline/tracy_blit",
    "minecraft:pipeline/translucent_block",
    "minecraft:pipeline/translucent_particle",
    "minecraft:pipeline/translucent_terrain",
    "minecraft:pipeline/translucent_terrain_multidraw",
    "minecraft:pipeline/vignette",
    "minecraft:pipeline/water_mask",
    "minecraft:pipeline/weather",
    "minecraft:pipeline/world_border",
];

/// Authoritative vanilla 26.3 optional render pipeline locations
/// (`OPTIONAL_PIPELINES_BY_LOCATION`), sorted lexicographically for binary search.
const VANILLA_OPTIONAL_PIPELINES: &[&str] = &[
    "minecraft:pipeline/wireframe",
    "minecraft:pipeline/wireframe_multidraw",
];

fn is_vanilla_pipeline(id: &Identifier, kind: PipelineKind) -> bool {
    if id.namespace() != "minecraft" {
        return false;
    }
    let table = match kind {
        PipelineKind::Required => VANILLA_REQUIRED_PIPELINES,
        PipelineKind::Optional => VANILLA_OPTIONAL_PIPELINES,
    };
    let full = id.to_string();
    table.binary_search(&full.as_str()).is_ok()
}

/// Validate `pipelines` against vanilla's built-in required/optional pipeline tables and earlier
/// mod registrations of the same kind. Cross-kind registrations sharing an ID are allowed.
pub fn validate_registrations(pipelines: &[RenderPipeline]) -> Result<(), String> {
    let mut seen: HashMap<(Identifier, PipelineKind), &str> =
        HashMap::with_capacity(pipelines.len());
    for pipeline in pipelines {
        let kind = pipeline.kind();
        let label = match kind {
            PipelineKind::Required => "required",
            PipelineKind::Optional => "optional",
        };
        if is_vanilla_pipeline(&pipeline.id, kind) {
            return Err(format!(
                "register_render_pipelines: mod {}: {label} pipeline {} collides with vanilla {label} pipeline",
                pipeline.owner(),
                pipeline.id
            ));
        }
        if let Some(earlier) = seen.insert((pipeline.id.clone(), kind), pipeline.owner()) {
            return Err(format!(
                "register_render_pipelines: mod {}: {label} pipeline {} collides with earlier {label} registration from mod {earlier}",
                pipeline.owner(),
                pipeline.id
            ));
        }
    }
    Ok(())
}

/// Validate and publish the mods' registrations once at startup, as `register_render_pipelines`
/// left them: each `(id, kind)` at most once and never colliding with vanilla's same-kind table.
/// A pipeline's place in the list is its slot.
pub fn register(pipelines: Vec<RenderPipeline>) -> Result<(), String> {
    validate_registrations(&pipelines)?;
    if REGISTERED.set(pipelines).is_err() {
        log::warn!("Render pipelines already registered; keeping the first list");
    }
    Ok(())
}

/// The registered pipelines; none until registration publishes them.
pub fn registered() -> &'static [RenderPipeline] {
    REGISTERED.get().map_or(&[], Vec::as_slice)
}

/// A registered pipeline one resource load built: a shader that compiles and its fixed state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Built {
    /// The owning mod, not necessarily the pipeline namespace.
    pub owner: String,
    /// The pipeline's id, for the log.
    pub id: Identifier,
    /// The whole shader: prelude, the mod's file, entry points.
    pub shader: String,
    pub blend: PipelineBlend,
    pub depth: WorldDepth,
    /// Whether the load may go on without it ([`RenderPipeline::optional()`]).
    pub optional: bool,
    /// What its shader samples, if anything.
    pub texture: Option<BuiltTexture>,
}

/// A textured pipeline's texture as a load found it ([`PipelineTexture`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuiltTexture {
    /// A sprite of the 2D atlas, by its name there (`block/stone`, `powah:block/energy_charge`),
    /// looked up when the renderer builds the pipeline.
    Sprite(String),
}

/// The registered pipelines as one resource load built them, by slot. An optional pipeline the
/// load refused has no shader here and resolves to [`RenderPipelineError::Invalid`].
#[derive(Clone, Debug, Default)]
pub struct Pipelines {
    keys: Vec<(Identifier, PipelineKind)>,
    built: Vec<Option<Built>>,
    /// The render assets' generation this set was loaded for; 0 for none.
    generation: u64,
    invalid: std::sync::Arc<std::sync::Mutex<Vec<usize>>>,
}

impl Pipelines {
    /// Build every registered pipeline from `source`, for the load of `generation`. An optional
    /// pipeline whose file is missing or refused is logged with its reason and the others build;
    /// a required one fails the load, naming every required pipeline refused and why.
    pub fn load(
        source: &crate::resource_source::ResourceSource,
        generation: u64,
    ) -> Result<Self, String> {
        Self::build(registered(), generation, |file| {
            source.read_to_string(file).map_err(|e| e.to_string())
        })
    }

    /// Build every registered pipeline, reading fragment files with `read`. A pipeline's
    /// texture is named here; its sprite is looked up when the renderer builds it.
    pub(crate) fn build(
        registered: &[RenderPipeline],
        generation: u64,
        read: impl Fn(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        validate_registrations(registered)?;
        let mut required = Vec::new();
        let built = registered
            .iter()
            .map(|pipeline| {
                let file = fragment_file(&pipeline.fragment);
                // A refusal reads `<file>[:<line>:<column>]: <message>`, the shader's own
                // `Problem`s printed as the pack checker prints them. The checks run contained:
                // a panic in naga on a mod's shader refuses the pipeline, with no crash report.
                let loaded = read(&file)
                    .map_err(|cause| format!("{file}: {cause}"))
                    .and_then(|text| {
                        crate::crash::contain(|| {
                            compose_with(&file, &text, pipeline.texture.is_some())
                        })
                        .map_err(|panic| format!("{file}: checking it panicked: {panic}"))?
                        .map_err(|problems| {
                            problems
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join("; ")
                        })
                    })
                    .and_then(|shader| {
                        let texture = match &pipeline.texture {
                            None => None,
                            Some(texture) => Some(load_texture(texture).map_err(|cause| {
                                format!("its texture: {}", texture_refusal(cause))
                            })?),
                        };
                        Ok((shader, texture))
                    });
                match loaded {
                    Ok((shader, texture)) => Some(Built {
                        owner: pipeline.owner().to_owned(),
                        id: pipeline.id.clone(),
                        shader,
                        blend: pipeline.blend,
                        depth: pipeline.depth,
                        optional: pipeline.optional,
                        texture,
                    }),
                    Err(reason) => {
                        let refusal = format!(
                            "render pipeline {} from mod {} refused: {reason}",
                            pipeline.id,
                            pipeline.owner()
                        );
                        if pipeline.optional {
                            log::warn!("Sunrise: {refusal}");
                        } else {
                            required.push(format!("required {refusal}"));
                        }
                        None
                    }
                }
            })
            .collect();
        if !required.is_empty() {
            return Err(required.join("; "));
        }
        Ok(Self {
            keys: registered
                .iter()
                .map(|p| (p.id.clone(), p.kind()))
                .collect(),
            built,
            generation,
            invalid: Default::default(),
        })
    }

    /// Marks `slot` as one the GPU refused; false when it already was, so it is reported once.
    pub(crate) fn invalidate(&self, slot: usize) -> bool {
        let mut invalid = self.invalid.lock().unwrap_or_else(|e| e.into_inner());
        let fresh = !invalid.contains(&slot);
        if fresh {
            invalid.push(slot);
        }
        fresh
    }

    /// Makes `slot` usable again: a renderer that built it took the refusal of another over.
    pub(crate) fn revalidate(&self, slot: usize) {
        self.invalid
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|&invalid| invalid != slot);
    }

    /// The load this set belongs to: a renderer holding another builds its pipelines again.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Every slot in order, with what the load built for it.
    pub fn built(&self) -> &[Option<Built>] {
        &self.built
    }

    /// The handle of the registered pipeline `(id, kind)` in this load.
    pub fn resolve(
        &self,
        id: &Identifier,
        kind: PipelineKind,
    ) -> Result<RenderPipelineRef, RenderPipelineError> {
        let slot = self
            .keys
            .iter()
            .position(|(registered, registered_kind)| registered == id && *registered_kind == kind)
            .ok_or(RenderPipelineError::Unknown)?;
        if self.built[slot].is_none()
            || self
                .invalid
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains(&slot)
        {
            return Err(RenderPipelineError::Invalid);
        }
        Ok(mod_api::__private::built::render_pipeline_ref(
            self.generation,
            slot as u32,
        ))
    }

    /// The slot a handle draws with, checked once when a batch is submitted.
    pub fn slot(&self, pipeline: RenderPipelineRef) -> Result<u32, RenderPipelineError> {
        use mod_api::__private::game::Fields;
        let fields = pipeline.fields();
        if fields.generation != self.generation {
            return Err(RenderPipelineError::Stale);
        }
        if self
            .invalid
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&(fields.slot as usize))
        {
            return Err(RenderPipelineError::Invalid);
        }
        match self.built.get(fields.slot as usize) {
            Some(Some(_)) => Ok(fields.slot),
            Some(None) => Err(RenderPipelineError::Invalid),
            None => Err(RenderPipelineError::Unknown),
        }
    }
}

/// `mymod:beam/glow` is `assets/mymod/shaders/pipeline/beam/glow.wgsl`.
fn fragment_file(fragment: &Identifier) -> String {
    format!(
        "assets/{}/{FOLDER}/{}.wgsl",
        fragment.namespace(),
        fragment.path()
    )
}

/// `texture` as a load finds it, or its named refusal.
fn load_texture(texture: &PipelineTexture) -> Result<BuiltTexture, PipelineTextureError> {
    match texture {
        PipelineTexture::Sprite(id) => {
            // A file's path or name is not a sprite key.
            let path = id.path();
            if path.starts_with("textures/") || path.ends_with(".png") {
                return Err(PipelineTextureError::NotASpriteKey);
            }
            // Named as the 2D atlas names the blocks and items atlases' sprites.
            Ok(BuiltTexture::Sprite(match id.namespace() {
                "minecraft" => path.to_owned(),
                _ => id.to_string(),
            }))
        }
        // A texture kind a later API adds is one this game has no route for yet.
        _ => Err(PipelineTextureError::NoSuchTextureFile),
    }
}

/// The diagnostics line of a refused texture: its named cause, then what it means.
pub fn texture_refusal(cause: PipelineTextureError) -> String {
    format!("{cause:?}: {cause}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLOW: &str = "\
fn pipeline_fragment(input: PipelineInput) -> vec4<f32> {
    return vec4<f32>(input.color.rgb * (1.0 + sin(frame.time)), input.color.a * input.uv.y);
}
";

    fn pipeline(id: &str, fragment: &str) -> RenderPipeline {
        RenderPipeline::new(
            Identifier::parse(id).unwrap(),
            Identifier::parse(fragment).unwrap(),
            PipelineBlend::Additive,
            WorldDepth::ThroughWalls,
        )
    }
    fn id(text: &str) -> Identifier {
        Identifier::parse(text).unwrap()
    }

    #[test]
    fn an_optional_pipeline_that_fails_leaves_the_others_drawing_and_refuses_its_own_use() {
        let registered = [
            pipeline("example:glow", "example:glow"),
            pipeline("example:broken", "example:broken").optional(),
            pipeline("example:missing", "example:nested/missing").optional(),
            pipeline("example:driver", "example:glow").optional(),
        ];
        let read = |file: &str| match file {
            "assets/example/shaders/pipeline/glow.wgsl" => Ok(GLOW.to_owned()),
            "assets/example/shaders/pipeline/broken.wgsl" => Ok("fn nothing() {}".to_owned()),
            "assets/example/shaders/pipeline/nested/missing.wgsl" => Err("not found".to_owned()),
            other => panic!("read {other}"),
        };
        let set = Pipelines::build(&registered, 7, read).unwrap();

        let glow = set
            .resolve(&id("example:glow"), PipelineKind::Required)
            .unwrap();
        assert_eq!(set.slot(glow), Ok(0));
        let built = set.built()[0].as_ref().unwrap();
        assert_eq!(
            (built.blend, built.depth),
            (PipelineBlend::Additive, WorldDepth::ThroughWalls)
        );
        assert_eq!(
            set.resolve(&id("example:glow"), PipelineKind::Optional),
            Err(RenderPipelineError::Unknown)
        );
        assert_eq!(
            set.resolve(&id("example:broken"), PipelineKind::Optional),
            Err(RenderPipelineError::Invalid)
        );
        assert_eq!(
            set.resolve(&id("example:missing"), PipelineKind::Optional),
            Err(RenderPipelineError::Invalid)
        );
        assert_eq!(
            set.resolve(&id("example:never"), PipelineKind::Required),
            Err(RenderPipelineError::Unknown)
        );

        // The GPU turns down a pipeline the load built: it is reported once, a handle taken
        // before is refused, and the rest still draw.
        let driver = set
            .resolve(&id("example:driver"), PipelineKind::Optional)
            .unwrap();
        assert!(set.invalidate(3));
        assert!(!set.invalidate(3));
        assert_eq!(set.slot(driver), Err(RenderPipelineError::Invalid));
        assert_eq!(
            set.resolve(&id("example:driver"), PipelineKind::Optional),
            Err(RenderPipelineError::Invalid)
        );
        assert_eq!(set.slot(glow), Ok(0));
    }

    #[test]
    fn a_required_pipeline_that_fails_fails_the_load_by_name() {
        let registered = [
            pipeline("example:glow", "example:glow"),
            pipeline("example:beam", "example:beam"),
            pipeline("example:extra", "example:extra").optional(),
        ];
        let before = Pipelines::build(&registered, 7, |_| Ok(GLOW.to_owned())).unwrap();
        let handle = before
            .resolve(&id("example:beam"), PipelineKind::Required)
            .unwrap();

        // A pack broke the required beam and the optional extra: the load fails naming the
        // required one only, so nothing of it is published and the previous load still draws.
        let error = Pipelines::build(&registered, 8, |file| match file {
            "assets/example/shaders/pipeline/glow.wgsl" => Ok(GLOW.to_owned()),
            _ => Ok("garbage".to_owned()),
        })
        .unwrap_err();
        assert!(
            error.starts_with(
                "required render pipeline example:beam from mod  refused: assets/example/shaders/pipeline/beam.wgsl:1:1: "
            ),
            "{error}"
        );
        assert!(!error.contains("example:glow"), "{error}");
        assert!(!error.contains("example:extra"), "{error}");
        assert_eq!(before.slot(handle), Ok(1));
    }

    #[test]
    fn initial_required_failure_has_a_literal_message() {
        let registered = [pipeline("example:glow", "example:glow")];
        let error = Pipelines::build(&registered, 7, |_| Err("not found".to_owned())).unwrap_err();
        assert_eq!(
            error,
            "required render pipeline example:glow from mod  refused: assets/example/shaders/pipeline/glow.wgsl: not found"
        );
    }

    #[test]
    fn a_reload_makes_old_handles_stale_and_can_fix_or_break_a_pipeline() {
        let registered = [pipeline("example:glow", "example:glow").optional()];
        let before = Pipelines::build(&registered, 7, |_| Ok(GLOW.to_owned())).unwrap();
        let handle = before
            .resolve(&id("example:glow"), PipelineKind::Optional)
            .unwrap();

        // A pack replaced the file with one that doesn't compile: the pipeline is invalid in
        // the new load, and the old handle is stale there, not silently redirected.
        let broken = Pipelines::build(&registered, 8, |_| Ok("garbage".to_owned())).unwrap();
        assert_eq!(broken.slot(handle), Err(RenderPipelineError::Stale));
        assert_eq!(
            broken.resolve(&id("example:glow"), PipelineKind::Optional),
            Err(RenderPipelineError::Invalid)
        );

        let fixed = Pipelines::build(&registered, 9, |_| Ok(GLOW.to_owned())).unwrap();
        assert_eq!(fixed.slot(handle), Err(RenderPipelineError::Stale));
        let again = fixed
            .resolve(&id("example:glow"), PipelineKind::Optional)
            .unwrap();
        assert_eq!(fixed.slot(again), Ok(0));
    }

    /// Sprites are named as the 2D atlas names them; an id naming a file is refused by name,
    /// and a required pipeline's refusal fails the load with the named cause.
    #[test]
    fn a_textured_pipeline_names_its_sprite_and_refuses_a_file_path() {
        use mod_api::PipelineTexture;
        let pipeline = |id: &str| {
            let name = Identifier::parse("test:glow").unwrap();
            RenderPipeline::new(
                name.clone(),
                name,
                PipelineBlend::Additive,
                WorldDepth::Tested,
            )
            .textured(PipelineTexture::Sprite(Identifier::parse(id).unwrap()))
        };
        let shader = |_: &str| Ok(GLOW.to_owned());
        let built = |id| {
            Pipelines::build(&[pipeline(id)], 1, shader)
                .map(|set| set.built()[0].as_ref().unwrap().texture.clone().unwrap())
        };
        assert_eq!(
            built("minecraft:block/stone"),
            Ok(BuiltTexture::Sprite("block/stone".into()))
        );
        assert_eq!(
            built("powah:block/energy_charge"),
            Ok(BuiltTexture::Sprite("powah:block/energy_charge".into()))
        );
        assert_eq!(
            load_texture(&PipelineTexture::Sprite(
                Identifier::parse("test:textures/glow").unwrap()
            )),
            Err(PipelineTextureError::NotASpriteKey)
        );
        assert_eq!(
            built("test:effect/glow.png"),
            Err("required render pipeline test:glow from mod  refused: its texture: NotASpriteKey: the id names a file, not a sprite key".into())
        );
    }

    #[test]
    fn resolve_distinguishes_required_and_optional_pipelines_in_either_order() {
        let optional_first = [
            pipeline("example:shared", "example:optional_glow").optional(),
            pipeline("example:shared", "example:required_glow"),
        ];
        let set = Pipelines::build(&optional_first, 11, |_| Ok(GLOW.to_owned())).unwrap();
        let optional = set
            .resolve(&id("example:shared"), PipelineKind::Optional)
            .unwrap();
        let required = set
            .resolve(&id("example:shared"), PipelineKind::Required)
            .unwrap();
        assert_eq!(set.slot(optional), Ok(0));
        assert_eq!(set.slot(required), Ok(1));

        let required_first = [
            pipeline("example:shared", "example:required_glow"),
            pipeline("example:shared", "example:optional_glow").optional(),
        ];
        let set = Pipelines::build(&required_first, 12, |_| Ok(GLOW.to_owned())).unwrap();
        let required = set
            .resolve(&id("example:shared"), PipelineKind::Required)
            .unwrap();
        let optional = set
            .resolve(&id("example:shared"), PipelineKind::Optional)
            .unwrap();
        assert_eq!(set.slot(required), Ok(0));
        assert_eq!(set.slot(optional), Ok(1));
    }

    #[test]
    fn registration_rejects_same_kind_vanilla_and_mod_collisions_while_preserving_cross_kind() {
        assert_eq!(VANILLA_REQUIRED_PIPELINES.len(), 190);
        assert_eq!(VANILLA_OPTIONAL_PIPELINES.len(), 2);
        assert!(
            VANILLA_REQUIRED_PIPELINES
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
        assert!(
            VANILLA_OPTIONAL_PIPELINES
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );

        let vanilla_required_err =
            validate_registrations(&[pipeline("minecraft:pipeline/solid_terrain", "example:glow")])
                .unwrap_err();
        assert!(
            vanilla_required_err.contains("collides with vanilla required pipeline"),
            "{vanilla_required_err}"
        );

        let vanilla_optional_err =
            validate_registrations(&[
                pipeline("minecraft:pipeline/wireframe", "example:glow").optional()
            ])
            .unwrap_err();
        assert!(
            vanilla_optional_err.contains("collides with vanilla optional pipeline"),
            "{vanilla_optional_err}"
        );

        // Cross-kind registrations against vanilla's opposite map and between mods are accepted.
        assert!(
            validate_registrations(&[
                pipeline("minecraft:pipeline/solid_terrain", "example:opt").optional(),
                pipeline("minecraft:pipeline/wireframe", "example:req"),
                pipeline("example:shared", "example:req"),
                pipeline("example:shared", "example:opt").optional(),
            ])
            .is_ok()
        );

        let dup_required_err = validate_registrations(&[
            pipeline("example:dup", "example:a"),
            pipeline("example:dup", "example:b"),
        ])
        .unwrap_err();
        assert!(
            dup_required_err.contains("collides with earlier required registration"),
            "{dup_required_err}"
        );

        let dup_optional_err = validate_registrations(&[
            pipeline("example:dup", "example:a").optional(),
            pipeline("example:dup", "example:b").optional(),
        ])
        .unwrap_err();
        assert!(
            dup_optional_err.contains("collides with earlier optional registration"),
            "{dup_optional_err}"
        );
    }
}
