//! A shader pack's post-processing over Sunrise's own world (spec 2.1 rows 4, 6, 8, 9; 6.1; 6.2;
//! 6.4): the world renders into its own target as with a resolution scale, its colour becomes
//! colortex0 and its depth `depthtex0`, then the pack's `prepare`, `deferred`, `composite` and
//! `final` programs run in order, and `final` draws onto the screen under the hand and HUD.
//! Gbuffers programs come later; until then the pack's composites read Sunrise's picture.
//!
//! Every colortex buffer a program reads or writes has two textures, A and B. Programs read
//! "main" and write "alt", and each buffer a program wrote flips afterwards (6.1.6). Every
//! frame starts with A as "main". A cleared buffer has both textures cleared then (Iris 1.11.7
//! reads and blends onto the clear value where a program leaves pixels, probed with a pack of our
//! own). A buffer that is not cleared reads back what was written last (OQ-8): when the last
//! frame ended on B, B is copied to A, and B keeps what was last written into it, which is what
//! a write leaves alone or blends onto there (a model that fits Iris 1.11.7's values in probe
//! packs of our own).
//! Pack buffers store rows as GL does (row 0 at the bottom: the translator negates clip y), so
//! the world is copied in upside down, and its reverse-Z depth becomes GL window depth.

// Everything below the module docs used to be one file; it is split by what each part does.
// `mod.rs` still holds `PackPost`'s fields and the module's test modules, and every name the
// parts share is visible here, so the tests reach the same names they always did.
//
// - `types`: the pack's entries and exits (what it is handed, what it makes, what it refuses)
//   and the puzzle of passes, buffers and bindings they are built from.
// - `construction`: `PackPost::new` and the pieces it builds them with, up to `set_paged_maps`.
// - `execution`: from `take_center_depth` on: the frame's passes, its flips and resizes.
mod construction;
mod execution;
mod types;

// The names other modules reach through `gfx::pack_post` keep the paths and visibility they had.
pub use construction::Refused;
pub(crate) use construction::{Geometry, ShadowView, TerrainList, TerrainPage};
// The tests' fixtures reach these where they used to (one file had one namespace).
#[cfg(test)]
pub(crate) use construction::{TerrainDraw, block_ids, renderable_float, terrain_tables};
use types::*;
pub(crate) use types::{Clouds, Lines, World};

use std::collections::HashMap;

// `PackPost`'s fields' types, and what the tests reach through `use super::*` (as they did
// when this was one file).
#[allow(unused_imports)]
use glam::{DMat4, DVec3};
#[allow(unused_imports)]
use smallvec::SmallVec;
#[allow(unused_imports)]
use sunrise_shaders::{BUFFERS, Format, Group, Post, SHADOW_COLORS};

#[allow(unused_imports)]
use super::pack_draw::DrawUniforms;
#[allow(unused_imports)]
use super::pack_entities::{PROGRAMS, PackEntities, PackEntitiesExt};
#[allow(unused_imports)]
use super::pack_images;
#[allow(unused_imports)]
use super::pack_sky::PackSky;
#[allow(unused_imports)]
use super::pack_textures;
#[allow(unused_imports)]
use super::pack_uniforms::{self, FrameUniforms, FrameUniformsExt, PackInputs};

mod packed;

pub struct PackPost {
    setup_pending: bool,
    pub notes: Vec<String>,
    passes: Vec<Pass>,
    /// Shadow cache for temporal reprojection.
    shadow_cache: ShadowCache,
    /// What draws terrain (`gbuffers_terrain` or its fallback), between `prepare` and `deferred`;
    /// when the pack draws terrain in two parts, solid terrain only (`gbuffers_terrain_solid` or
    /// its fallback).
    terrain: Option<Pass>,
    /// What draws cutout terrain (`gbuffers_terrain_cutout` or its fallback), after `terrain`,
    /// when the pack draws terrain in two parts. A pack with no program for one part has only
    /// the other.
    terrain_cutout: Option<Pass>,
    /// What draws translucent terrain (`gbuffers_water` or its fallback), after `deferred`.
    water: Option<Pass>,
    /// The pack has no program for solid and cutout terrain, or for translucent terrain:
    /// nothing draws it (`sunrise_shaders::Post::terrain_undrawn`).
    terrain_undrawn: bool,
    water_undrawn: bool,
    /// What draws the sky (`gbuffers_skybasic` or its fallback), before terrain, and its vertices.
    sky: Option<Sky>,
    /// What draws entities and block entities, after terrain, and their vertices.
    entities: Option<Entities>,
    /// What draws the clouds, after entities, and this frame's clouds.
    clouds: Option<Pass>,
    /// What draws the block outline (`gbuffers_line`, else `gbuffers_basic`), and this frame's.
    lines: Option<Pass>,
    line_draw: Option<Lines>,
    cloud_draw: Option<Clouds>,
    /// The pack draws the clouds (`gbuffers_clouds` or its fallback), even with a program that
    /// writes nothing (a pack drawing its own clouds discards vanilla's), or `clouds=off`: the
    /// game's clouds are not drawn.
    hides_clouds: bool,
    /// The pack has a sky (`gbuffers_skybasic`) or sun and moon (`gbuffers_skytextured`)
    /// program: vanilla's stays hidden even when the program writes nothing.
    hides_sky: bool,
    hides_celestial: bool,
    /// `colortex0ClearColor`, when the pack sets it.
    sky_clear: Option<[f32; 4]>,
    /// What draws rain and snow (the same program, each with its kind's sprite as `texture`),
    /// after translucent terrain, this frame's weather and the sprites' generation bound.
    weather: Option<[Pass; 2]>,
    weather_draw: Option<Weather>,
    weather_generation: u64,
    /// What draws the first-person hand (`gbuffers_hand` or its fallback), after the opaque
    /// world, and its vertices.
    hand: Option<(Pass, super::pack_hand::PackHand)>,
    /// `gbuffers_hand_water` (or its fallback): the hand's translucent quads, after deferred.
    hand_water: Option<Pass>,
    /// The pre-translucent copy: `depthtex1`, or `depthtex2` without a shader hand.
    depth1: bool,
    /// A program after the hand reads `depthtex2`: it is copied before the hand.
    depth2: bool,
    /// Translucent terrain reads the boats' water patches (`Targets::water_mask`).
    water_mask: bool,
    /// The water patches' depth holds patches (or was never cleared): cleared before the next
    /// frame's are drawn.
    water_mask_dirty: bool,
    /// The shadow buffers, when the shadow pass runs (2.5), and the `shadowcomp` programs.
    shadow_targets: Option<super::pack_shadow::ShadowTargets>,
    /// What draws terrain from the light (`shadow`), and translucent terrain (`shadow_water`).
    shadow_terrain: Option<Pass>,
    /// `end_portal.png`'s atlas rect this frame, and a texture of its own copied from it that
    /// End portal draws bind as their atlas.
    portal_rect: [f32; 4],
    portal_texture: Option<([f32; 4], wgpu::TextureView)>,
    shadow_water: Option<Pass>,
    shadow_comp: Vec<Pass>,
    /// Buffers with mipmaps (filterable ones a program asks them for).
    mipped: u16,
    /// Buffers whose mip chain matches their first level, this frame.
    mips_valid: u16,
    /// The downsampling pipeline of each format with mipmaps.
    down: HashMap<wgpu::TextureFormat, (wgpu::RenderPipeline, wgpu::BindGroupLayout)>,
    /// The per-draw uniforms of gbuffers programs.
    draws: DrawUniforms,
    /// Set 3 of flat terrain: an unknown-source fallback and immutable state/sprite tables.
    /// Each page's bind group tracks both its vertex buffer and optional source sidecar.
    block_ids: Option<(wgpu::BindGroupLayout, wgpu::Buffer, wgpu::Buffer)>,
    page_groups: HashMap<u32, (wgpu::Buffer, Option<wgpu::Buffer>, wgpu::BindGroup)>,
    gui_size: [u32; 2],
    /// GPU time of each pass, while the renderer asks for it (`pack_timing.rs`).
    pub timing: Option<super::pack_timing::PassTiming>,
    /// `depthtex0` at the screen's centre, read back for `centerDepthSmooth`.
    center_depth: Option<super::pack_center_depth::CenterDepth>,
    /// `shaders.properties`' custom uniforms, after the frame's in `uniforms.values`.
    custom: sunrise_shaders::Custom,
    atlas: Option<wgpu::TextureView>,
    material_maps: Option<[wgpu::TextureView; 2]>,
    /// The shared atlas's normal and specular paged maps (`set_paged_maps`).
    paged_maps: [Option<wgpu::TextureView>; 2],
    lightmap: wgpu::TextureView,
    deferred_pre: u16,
    composite_pre: u16,
    formats: [wgpu::TextureFormat; BUFFERS],
    clears: [Option<[f32; 4]>; BUFFERS],
    /// Buffers of a three-channel format, and whether their textures are new: then they get
    /// alpha 1 before the next frame.
    rgb: u16,
    rgb_unset: bool,
    used: u16,
    /// Buffers some pass draws into the "alt" side of or flips; the others' B is their A.
    doubled: u16,
    /// `size.buffer.*`.
    sizes: [Option<sunrise_shaders::BufferSize>; BUFFERS],
    /// Buffers bound as images, and those also copied for a pass that samples them.
    storage: u16,
    copied: u16,
    /// Bit n set: B is buffer n's "main".
    main: u16,
    targets: Option<Targets>,
    noise: wgpu::TextureView,
    dummy: wgpu::TextureView,
    dummy_black: wgpu::TextureView,
    dummy_normal: wgpu::TextureView,
    dummy_3d: wgpu::TextureView,
    dummy_1d: wgpu::TextureView,
    /// `image.*` and `bufferObject.*`.
    images: Vec<pack_images::PackImage>,
    /// Set 5 of programs with a geometry stage.
    indices: Option<wgpu::BindGroupLayout>,
    shadow_culling: sunrise_shaders::ShadowCulling,
    gpu: Gpu,
    /// Zeros to clear images by copying, where the device has no `clear_texture`.
    zeros: Option<wgpu::Buffer>,
    /// What the images no `image.*` gives bind.
    stand_ins: Vec<pack_images::StandIn>,
    dump: Option<pack_images::Dump>,
    buffers: pack_images::Buffers,
    textures: Vec<pack_textures::PackTexture>,
    /// The stand-in shadow map and its comparison sampler; cleared on the first frame.
    shadow: (wgpu::TextureView, wgpu::Sampler),
    shadow_cleared: bool,
    linear: wgpu::Sampler,
    nearest: wgpu::Sampler,
    repeat: wgpu::Sampler,
    nearest_repeat: wgpu::Sampler,
    atlas_sampler: wgpu::Sampler,
    /// colortex buffers' (`SamplerKind::Buffer`).
    buffer_sampler: wgpu::Sampler,
    /// None when it would write nothing: colortex0 holds integers and there is no depth to copy
    /// nor buffer to clear.
    blit: Option<(wgpu::RenderPipeline, wgpu::BindGroupLayout)>,
    /// The blit also writes depthtex0 (some program reads it) and clears these buffers.
    blit_depth: bool,
    blit_clears: Vec<usize>,
    /// Buffers cleared in passes of their own (more than the blit's eight attachments).
    other_clears: Vec<usize>,
    /// Source views identify the world target, including replacements at the same size.
    blit_group: Option<([wgpu::TextureView; 2], wgpu::BindGroup)>,
    /// Shows colortex0 when the pack has no `final` (OQ-5).
    copy: (wgpu::RenderPipeline, wgpu::BindGroupLayout),
    /// Copies the picture `final` drew at the world's size onto the window: no flip, it is
    /// already in the screen's rows.
    upscale: (wgpu::RenderPipeline, wgpu::BindGroupLayout),
    /// The picture `final` draws while the world's size is not the window's.
    final_target: Option<FinalTarget>,
    screen: wgpu::TextureFormat,
    uniforms: FrameUniforms,
}

#[cfg(test)]
mod history_reset_tests;

#[cfg(test)]
mod world_target_tests;

#[cfg(test)]
mod celestial_blend_tests;

#[cfg(test)]
mod higher_shadowcolor_tests;

#[cfg(test)]
mod single_side_tests;

#[cfg(test)]
mod mixed_size_tests;

#[cfg(test)]
mod sky_disc_blend_tests;

#[cfg(test)]
mod undrawn_terrain_tests;

mod image_formats;
use image_formats::{image_format, storage_formats_of};
#[cfg(test)]
mod full_outputs_tests;
#[cfg(test)]
mod shadow_faces_tests;

#[cfg(test)]
mod r11f_image_tests;

#[cfg(test)]
mod work_group_fold_tests;

#[cfg(test)]
mod forced_flip_tests;

#[cfg(test)]
mod weather_depth_tests;

#[cfg(test)]
mod float_into_integer_tests;

#[cfg(test)]
mod custom_type_tests;

#[cfg(test)]
mod sky_depth_tests;

#[cfg(test)]
mod depth_copy_tests;

#[cfg(test)]
mod end_fan_tests;

#[cfg(test)]
mod runtime_length_tests;

#[cfg(test)]
mod translucent_color_tests;

#[cfg(test)]
mod noise_resolution_tests;

#[cfg(test)]
mod snorm_tests;

#[cfg(test)]
mod depth_test;

#[cfg(test)]
mod terrain_parts_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::TextureFormat as T;

    #[test]
    fn shadow_cache_key_tracks_world_and_time_changes() {
        let key = ShadowCacheKey {
            sun_angle: 0.0,
            moon_angle: 180.0,
            snapped_camera: Some(DVec3::ZERO),
            shadow_settings_hash: 1,
            shadow_scene_revision: (2, 3),
            ticks: 4,
            day_time: 5,
            shadow_uniforms_hash: 6,
        };

        assert_ne!(
            key,
            ShadowCacheKey {
                shadow_scene_revision: (8, 3),
                ..key
            }
        );
        assert_ne!(key, ShadowCacheKey { day_time: 9, ..key });
        assert_ne!(
            key,
            ShadowCacheKey {
                shadow_uniforms_hash: 10,
                ..key
            }
        );
    }

    /// Two textures with a sampler each become two textures sharing one sampler variable.
    #[test]
    fn samplers_of_a_kind_share_one_variable() {
        let mut module = naga::front::wgsl::parse_str(
            "@group(0) @binding(1) var a: texture_2d<f32>;\n\
             @group(0) @binding(2) var sa: sampler;\n\
             @group(0) @binding(3) var b: texture_2d<f32>;\n\
             @group(0) @binding(4) var sb: sampler;\n\
             @fragment fn main() -> @location(0) vec4<f32> {\n\
             return textureSample(a, sa, vec2(0.5)) + textureSample(b, sb, vec2(0.5));\n}\n",
        )
        .unwrap();
        share_samplers(&mut module, &HashMap::from([(2, 2), (4, 2)]));
        let info = naga::valid::Validator::new(Default::default(), Default::default())
            .validate(&module)
            .unwrap();
        let used: Vec<u32> = module
            .global_variables
            .iter()
            .filter(|(h, _)| !info.get_entry_point(0)[*h].is_empty())
            .filter_map(|(_, g)| g.binding.as_ref().map(|b| b.binding))
            .collect();
        assert_eq!(used, [1, 2, 3]);
    }

    /// A pack's pipelines are made together on several threads: each lands in its pass's slot,
    /// and one wgpu refuses comes back by name (the pack is then built a program at a time).
    #[test]
    fn pipelines_compile_together_and_a_refused_one_is_named() {
        let _gpu = crate::gfx::test_gpu::lock();
        let Some((_adapter, device, _queue)) = crate::gfx::test_gpu::optional_device() else {
            return;
        };
        let layout = device.create_pipeline_layout(&Default::default());
        let module = |source: &str| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(source.into()),
            })
        };
        let job = |label: &str, module: wgpu::ShaderModule| {
            let slot = Slot::default();
            let job = ComputeJob {
                label: label.into(),
                layout: layout.clone(),
                module,
            };
            (slot.clone(), Job::Compute(slot, job))
        };
        let plain = "@compute @workgroup_size(1) fn main() {}";
        let go = std::sync::atomic::AtomicBool::new(false);
        let (slots, jobs): (Vec<_>, Vec<_>) = (0..12)
            .map(|i| job(&format!("composite{i}"), module(plain)))
            .unzip();
        // Cancelled before it starts: nothing is made (`new` then drops the pack).
        assert_eq!(compile(&device, &jobs, &true.into()), None);
        assert!(slots.iter().all(|s| s.get().is_none()));
        assert_eq!(compile(&device, &jobs, &go), None);
        assert!(slots.iter().all(|s| s.get().is_some()));
        // A binding the layout doesn't have: wgpu refuses the pipeline.
        let bound = "@group(0) @binding(0) var<storage, read_write> b: array<u32>;\n\
                     @compute @workgroup_size(1) fn main() { b[0] = 1u; }";
        let (_, bad) = job("deferred3", module(bound));
        let (slots, mut jobs): (Vec<_>, Vec<_>) =
            (0..4).map(|i| job(&format!("c{i}"), module(plain))).unzip();
        jobs.insert(2, bad);
        let (program, _why) = compile(&device, &jobs, &go).expect("the pipeline is refused");
        assert_eq!(program, "deferred3");
        // The refusal stops none of the others: they are all made.
        assert!(slots.iter().all(|s| s.get().is_some()));
    }

    /// #3525: naga's SPIR-V writer panicked on Photon's gbuffers programs as the pipeline was
    /// made. Such a panic skips the pass, as a validation error does, and the device goes on.
    #[test]
    fn a_shader_compiler_panic_skips_the_pass() {
        let _gpu = crate::gfx::test_gpu::lock();
        let Some((_adapter, device, _queue)) = crate::gfx::test_gpu::optional_device() else {
            return;
        };
        let failed = checked(&device, || -> wgpu::Buffer {
            panic!("internal error: entered unreachable code: Expression [89] is not cached!")
        });
        let error = failed.expect_err("the panic comes back as an error");
        assert!(error.contains("Expression [89] is not cached!"), "{error}");
        let buffer = |usage| {
            checked(&device, || {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: 16,
                    usage,
                    mapped_at_creation: false,
                })
            })
        };
        // The device still makes things, and its errors still come back.
        assert!(buffer(wgpu::BufferUsages::UNIFORM).is_ok());
        assert!(buffer(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::MAP_WRITE).is_err());
    }

    /// `texture.composite.colortex0` stands in for buffer 0 in composite programs until one of
    /// them flips that buffer; later ones, and `final`, read the buffer (Super Duper Vanilla,
    /// Bliss, SEUS PTGI HRR). Another stage's flips don't release it.
    #[test]
    fn a_custom_texture_over_a_buffer_ends_when_its_stage_flips_it() {
        let _gpu = crate::gfx::test_gpu::lock();
        let Some((adapter, device, _queue)) = crate::gfx::test_gpu::optional_device() else {
            return;
        };
        let textures = [pack_textures::PackTexture {
            stage: "composite".into(),
            name: "colortex0".into(),
            dimension: wgpu::TextureViewDimension::D2,
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            blur: false,
            clamp: false,
            png: false,
            image: pack_textures::Image::Atlas,
        }];
        let interface = sunrise_shaders::Interface {
            uniform_size: 48,
            initial_values: vec![0; 48],
            uniforms: Vec::new(),
            samplers: vec![sunrise_shaders::Sampler {
                name: "colortex0".into(),
                ty: "sampler2D".into(),
                binding: 1,
            }],
            outputs: vec![0],
            draw: false,
            images: Vec::new(),
            buffers: Vec::new(),
        };
        let buffers = pack_images::Buffers::new(
            &device,
            Default::default(),
            Default::default(),
            wgpu::ShaderStages::FRAGMENT,
        );
        let stand_ins = std::cell::RefCell::new(Vec::new());
        let pack = Resources {
            textures: &textures,
            images: &[],
            left_out: &[],
            buffers: &buffers,
            indices: None,
            gpu: Gpu::of(&adapter, device.features()),
            storable: 0,
            rgb: 0,
            packed: [None; BUFFERS],
            released: std::cell::Cell::new(("", 0)),
            stand_ins: &stand_ins,
            shadow_linear: 0,
            jobs: None,
        };
        let reads = |group: Group| {
            let (_, samplers, ..) = bindings(
                device.features(),
                &interface,
                "composite",
                group,
                &[1],
                None,
                &[T::Rgba8Unorm; BUFFERS],
                None,
                &mut Vec::new(),
                &pack,
            )
            .expect("the pass is kept");
            samplers[0].source
        };
        assert!(reads(Group::Composite) == Source::Atlas);
        pack.released.set(("deferred", 1));
        assert!(reads(Group::Composite) == Source::Atlas);
        pack.released.set(("composite", 1));
        assert!(reads(Group::Composite) == Source::Color(0));
        assert!(reads(Group::Final) == Source::Color(0));
    }

    /// Iris 1.11.7 (a probe pack of ours, #9064) didn't bind a PNG under
    /// `texture.<stage>.<name>` when `name` was a name of the pack's own, no sampler of the
    /// game's; a raw texture there, a PNG over `gaux2` or `noisetex`, and a `customTexture` PNG
    /// were bound.
    #[test]
    fn a_png_under_a_name_of_its_own_is_not_bound() {
        let _gpu = crate::gfx::test_gpu::lock();
        let Some((adapter, device, _queue)) = crate::gfx::test_gpu::optional_device() else {
            return;
        };
        let texture = |stage: &str, name: &str, png: bool| pack_textures::PackTexture {
            stage: stage.into(),
            name: name.into(),
            dimension: wgpu::TextureViewDimension::D2,
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            blur: !png,
            clamp: !png,
            png,
            image: pack_textures::Image::Atlas,
        };
        let textures = [
            texture("composite", "tp", true),
            texture("composite", "tr", false),
            texture("composite", "gaux2", true),
            texture("composite", "noisetex", true),
            texture(sunrise_shaders::EVERY_STAGE, "cp", true),
        ];
        let names = ["tp", "tr", "gaux2", "noisetex", "cp"];
        let interface = sunrise_shaders::Interface {
            uniform_size: 48,
            uniforms: Vec::new(),
            samplers: names
                .iter()
                .zip(1..)
                .map(|(name, k)| sunrise_shaders::Sampler {
                    name: (*name).into(),
                    ty: "sampler2D".into(),
                    binding: 2 * k - 1,
                })
                .collect(),
            outputs: vec![0],
            draw: false,
            images: Vec::new(),
            buffers: Vec::new(),
            initial_values: vec![0; 48],
        };
        let buffers = pack_images::Buffers::new(
            &device,
            Default::default(),
            Default::default(),
            wgpu::ShaderStages::FRAGMENT,
        );
        let stand_ins = std::cell::RefCell::new(Vec::new());
        let pack = Resources {
            textures: &textures,
            images: &[],
            left_out: &[],
            buffers: &buffers,
            indices: None,
            gpu: Gpu::of(&adapter, device.features()),
            storable: 0,
            rgb: 0,
            released: std::cell::Cell::new(("", 0)),
            stand_ins: &stand_ins,
            shadow_linear: 0,
            jobs: None,
            packed: [None; BUFFERS],
        };
        let (_, samplers, ..) = bindings(
            device.features(),
            &interface,
            "final",
            Group::Final,
            &[0],
            None,
            &[T::Rgba8Unorm; BUFFERS],
            None,
            &mut Vec::new(),
            &pack,
        )
        .expect("the pass is kept");
        let read: Vec<Source> = samplers.iter().map(|b| b.source).collect();
        assert!(
            read == [
                Source::Dummy,
                Source::Atlas,
                Source::Atlas,
                Source::Atlas,
                Source::Atlas
            ]
        );
    }

    /// Iris 1.11.7 runs a program whose image no `image.*` gives, and its loads read zero
    /// (observed with a probe pack). Its pass is kept, the image bound to a stand-in of its
    /// declared format, zero again before each pass even after one stored into it.
    #[test]
    fn an_image_without_an_entry_reads_zero() {
        let _gpu = crate::gfx::test_gpu::lock();
        use wgpu::TextureViewDimension as D;
        let Some((adapter, device, queue)) = crate::gfx::test_gpu::optional_device() else {
            return;
        };
        let image = |name: &str, ty: &str, format: &str, binding| sunrise_shaders::Image {
            name: name.into(),
            ty: ty.into(),
            format: format.into(),
            access: String::new(),
            binding,
            before: None,
        };
        let interface = sunrise_shaders::Interface {
            uniform_size: 48,
            initial_values: vec![0; 48],
            uniforms: Vec::new(),
            samplers: Vec::new(),
            outputs: vec![0],
            draw: false,
            images: vec![
                image("mask", "uimage3D", "r32ui", 1),
                image("other", "uimage3D", "r32ui", 2),
                image("flat", "image2D", "rgba8", 3),
            ],
            buffers: Vec::new(),
        };
        let buffers = pack_images::Buffers::new(
            &device,
            Default::default(),
            Default::default(),
            wgpu::ShaderStages::FRAGMENT,
        );
        let stand_ins = std::cell::RefCell::new(Vec::new());
        let pack = Resources {
            textures: &[],
            images: &[],
            left_out: &[],
            buffers: &buffers,
            indices: None,
            gpu: Gpu::of(&adapter, device.features()),
            storable: 0,
            rgb: 0,
            packed: [None; BUFFERS],
            released: std::cell::Cell::new(("", 0)),
            stand_ins: &stand_ins,
            shadow_linear: 0,
            jobs: None,
        };
        let mut notes = Vec::new();
        let (.., images) = bindings(
            device.features(),
            &interface,
            "composite",
            Group::Composite,
            &[0],
            None,
            &[T::Rgba8Unorm; BUFFERS],
            None,
            &mut notes,
            &pack,
        )
        .expect("the pass is kept");
        assert!(
            images
                == [
                    (1, Image::StandIn(T::R32Uint, D::D3)),
                    (2, Image::StandIn(T::R32Uint, D::D3)),
                    (3, Image::StandIn(T::Rgba8Unorm, D::D2)),
                ]
        );
        assert_eq!(notes.len(), 3, "{notes:?}");
        // One stand-in for each format and dimension.
        let made = pack_images::stand_ins(&device, stand_ins.take());
        assert_eq!(made.len(), 2);
        let mask = pack_images::stand_in(&made, T::R32Uint, D::D3).expect("made");
        let texture = mask.view.texture();
        queue.write_texture(
            texture.as_image_copy(),
            &7u32.to_le_bytes(),
            wgpu::TexelCopyBufferLayout::default(),
            texture.size(),
        );
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        mask.clear(&mut encoder);
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &read,
                layout: wgpu::TexelCopyBufferLayout::default(),
            },
            texture.size(),
        );
        queue.submit([encoder.finish()]);
        read.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(&read.slice(..).get_mapped_range().unwrap()[..], &[0; 4]);
    }

    /// `scale.composite2 = 0.25 0.5 0.25` on a 1280×720 picture: Iris 1.11.7 draws x 640..960
    /// and GL y 180..360 (a probe pack of our own). Rows are GL's here, so that is rows 180..360.
    #[test]
    fn a_scaled_pass_draws_its_part_from_gl_s_corner() {
        assert_eq!(
            viewport([1280, 720], [0.25, 0.5, 0.25]),
            [640.0, 180.0, 320.0, 180.0]
        );
        assert_eq!(
            viewport([1280, 720], [0.5, 0.0, 0.0]),
            [0.0, 0.0, 640.0, 360.0]
        );
        // Whole pixels, at least one, and never past the buffer's edge.
        assert_eq!(viewport([5, 3], [0.5, 0.0, 0.0]), [0.0, 0.0, 2.0, 1.0]);
        assert_eq!(viewport([4, 4], [0.01, 0.0, 0.0]), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(
            viewport([100, 100], [1.0, 0.5, 0.75]),
            [50.0, 75.0, 50.0, 25.0]
        );
    }

    /// `blend.<program>` of a composite program blends its output onto what the buffer's side
    /// holds (Iris 1.11.7, probed: 0.25 over 0.75 with SRC_ALPHA ONE_MINUS_SRC_ALPHA ONE ZERO and
    /// a source alpha of 0.25 reads 0.625).
    #[test]
    fn a_composite_program_blends_onto_its_buffer() {
        let _gpu = crate::gfx::test_gpu::lock();
        let instance = crate::gfx::test_gpu::instance();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            return;
        };
        // A pass's pipeline layout has six bind group slots.
        let desc = wgpu::DeviceDescriptor {
            required_limits: adapter.limits(),
            ..Default::default()
        };
        let Ok((device, queue)) = pollster::block_on(adapter.request_device(&desc)) else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("hz-composite-blend-{}", std::process::id()));
        let shaders = dir.join("shaders");
        std::fs::create_dir_all(&shaders).unwrap();
        let file = |name: &str, text: &str| std::fs::write(shaders.join(name), text).unwrap();
        file(
            "composite.vsh",
            "#version 120\nvoid main() { gl_Position = ftransform(); }\n",
        );
        file(
            "composite.fsh",
            "#version 120\n/* DRAWBUFFERS:3 */\n\
             void main() { gl_FragData[0] = vec4(0.25, 0.25, 1.0, 0.25); }\n",
        );
        let pack = sunrise_shaders::Pack::open(&dir).unwrap();
        let properties = sunrise_shaders::Properties::parse(
            "blend.composite = SRC_ALPHA ONE_MINUS_SRC_ALPHA ONE ZERO\n",
            &sunrise_shaders::Macros::default(),
        );
        let mut post = sunrise_shaders::load_post(
            &pack,
            "",
            &properties,
            &sunrise_shaders::Macros::standard(),
            &Default::default(),
        );
        std::fs::remove_dir_all(&dir).unwrap();
        let program = post.programs.remove(0);
        let buffers = pack_images::Buffers::new(
            &device,
            Default::default(),
            Default::default(),
            wgpu::ShaderStages::FRAGMENT,
        );
        let stand_ins = std::cell::RefCell::new(Vec::new());
        let pack = Resources {
            textures: &[],
            images: &[],
            left_out: &[],
            buffers: &buffers,
            indices: None,
            gpu: Gpu::of(&adapter, device.features()),
            storable: 0,
            rgb: 0,
            packed: [None; BUFFERS],
            released: std::cell::Cell::new(("", 0)),
            stand_ins: &stand_ins,
            shadow_linear: 0,
            jobs: None,
        };
        let draws = DrawUniforms::new(&device);
        let mut notes = Vec::new();
        let pass = build_pass(
            &device,
            program,
            &[T::Rgba8Unorm; BUFFERS],
            None,
            T::Rgba8Unorm,
            &mut notes,
            None,
            &[],
            &draws.layout,
            &pack,
            false,
        )
        .unwrap_or_else(|| panic!("the pass builds: {notes:?}"));
        // colortex3 holds (0.75, 0.5, 0.25, 0.5) before the pass.
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: T::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let head = [64.0f32, 1.0, 1.0 / 64.0, 1.0];
        queue.write_buffer(&pass.ubo, 0, bytemuck::cast_slice(&head));
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pass.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: pass.ubo.as_entire_binding(),
            }],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        let before = wgpu::Color {
            r: 0.75,
            g: 0.5,
            b: 0.25,
            a: 0.5,
        };
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[attach(
                &view,
                wgpu::Operations {
                    load: wgpu::LoadOp::Clear(before),
                    store: wgpu::StoreOp::Store,
                },
            )],
            ..Default::default()
        });
        {
            // Loaded, as `PackPost::render` attaches a buffer a composite pass writes.
            let load = wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            };
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[attach(&view, load)],
                ..Default::default()
            });
            rp.set_pipeline(pass.render());
            rp.set_bind_group(0, &group, &[]);
            rp.draw(0..6, 0..1);
        }
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &read,
                layout: wgpu::TexelCopyBufferLayout::default(),
            },
            texture.size(),
        );
        queue.submit([encoder.finish()]);
        read.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let texel: Vec<u8> = read.slice(..).get_mapped_range().unwrap()[..4].to_vec();
        // rgb = 0.25 × src + 0.75 × dst, alpha = the source's.
        let expected = [0.625, 0.4375, 0.4375, 0.25].map(|v: f32| v * 255.0);
        for (got, want) in texel.iter().zip(expected) {
            assert!(
                (f32::from(*got) - want).abs() <= 1.0,
                "{texel:?} vs {expected:?}"
            );
        }
    }

    #[test]
    fn pack_formats_map_to_renderable_ones() {
        let f = |name| texture_format(Format::parse(name).unwrap(), wgpu::Features::empty());
        assert_eq!(f("RGBA"), T::Rgba8Unorm);
        assert_eq!(f("RGB8"), T::Rgba8Unorm, "no three-channel formats in wgpu");
        assert_eq!(f("RG16F"), T::Rg16Float);
        assert_eq!(f("R32F"), T::R32Float);
        assert_eq!(f("RGB10_A2"), T::Rgb10a2Unorm);
        assert_eq!(f("R32UI"), T::R32Uint);
        assert_eq!(f("RGBA16"), T::Rgba16Float, "16-bit norm needs a feature");
        assert_eq!(f("RGBA8_SNORM"), T::Rgba16Float, "snorm is not renderable");
        assert_eq!(f("R11F_G11F_B10F"), T::Rgba16Float);
        let all = wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
            | wgpu::Features::RG11B10UFLOAT_RENDERABLE;
        let g = |name| texture_format(Format::parse(name).unwrap(), all);
        assert_eq!(g("RGBA16"), T::Rgba16Unorm);
        assert_eq!(g("R11F_G11F_B10F"), T::Rg11b10Ufloat);
    }

    #[test]
    fn float_targets_fall_back_when_not_renderable() {
        let all = |_| true;
        assert_eq!(renderable_float(T::Rgba32Float, all), T::Rgba32Float);
        // OpenGL without GL_EXT_color_buffer_float: only half floats render.
        let half_only = |f: T| matches!(f, T::R16Float | T::Rg16Float | T::Rgba16Float);
        assert_eq!(renderable_float(T::Rgba32Float, half_only), T::Rgba16Float);
        assert_eq!(renderable_float(T::Rg32Float, half_only), T::Rg16Float);
        assert_eq!(renderable_float(T::R32Float, half_only), T::R16Float);
        assert_eq!(renderable_float(T::Rgba16Float, half_only), T::Rgba16Float);
        // Neither renders: the 8-bit last resort.
        assert_eq!(renderable_float(T::Rgba32Float, |_| false), T::Rgba8Unorm);
        assert_eq!(renderable_float(T::Rg16Float, |_| false), T::Rgba8Unorm);
        // Other formats are never replaced.
        assert_eq!(renderable_float(T::Rgba8Uint, |_| false), T::Rgba8Uint);
        assert_eq!(
            renderable_float(T::Rgb10a2Unorm, |_| false),
            T::Rgb10a2Unorm
        );
    }

    #[test]
    fn an_image_is_made_in_its_layout_qualifiers_format() {
        // `image.x = … RGB10_A2 …` read through `layout(rgb10_a2) image2D x`.
        for q in [
            "rgba32f",
            "rgba16f",
            "rg16f",
            "r11f_g11f_b10f",
            "r32f",
            "rgba16",
            "rgb10_a2",
            "rgba8",
            "rg8",
            "r16",
            "rgba16_snorm",
            "rgba8_snorm",
            "r8_snorm",
            "rgba32i",
            "r16i",
            "rgb10_a2ui",
            "rgba8ui",
            "r32ui",
        ] {
            let def = Format::parse(q).unwrap();
            assert_eq!(Some(pack_images::format(def, true)), image_format(q), "{q}");
        }
    }

    #[test]
    fn samplers_read_custom_textures_of_their_type() {
        use wgpu::TextureSampleType as T;
        let float = T::Float { filterable: true };
        assert!(fits("sampler2D", float));
        assert!(fits("sampler3D", float));
        assert!(fits("isampler2D", T::Sint));
        assert!(fits("usampler1D", T::Uint));
        assert!(!fits("sampler2D", T::Uint));
        assert!(!fits("isampler2D", float));
        assert!(!fits("sampler2DShadow", float));
    }

    #[test]
    fn sampler_names() {
        let main = |n| source(n, false, false, false);
        assert!(main("gaux1") == Source::Color(4));
        assert!(main("gdepthtex") == Source::Depth);
        assert!(main("depthtex1") == Source::Depth1);
        assert!(main("depthtex2") == Source::Depth2);
        assert!(main("normals") == Source::Normals);
        assert!(main("specular") == Source::Specular);
        assert!(main("noisetex") == Source::Noise);
        // Without the shadow pass, the shadow buffers are stand-ins.
        assert!(main("shadowcolor0") == Source::Dummy);
        assert!(main("shadowtex0") == Source::Dummy);
        // With it: shadowcolor n is buffer n of the shadow programs, `shadow` is shadowtex1
        // beside `watershadow` (6.3), and colortex is not theirs.
        assert!(source("shadowcolor", false, true, false) == Source::ShadowColor(0));
        assert!(source("shadowcolor1", true, true, false) == Source::Color(1));
        assert!(source("colortex1", true, true, false) == Source::Dummy);
        assert!(source("shadow", false, true, false) == Source::ShadowDepth(0, false));
        assert!(source("shadow", false, true, true) == Source::ShadowDepth(1, false));
        assert!(source("watershadow", false, true, true) == Source::ShadowDepth(0, false));
        assert!(source("shadowtex1HW", false, true, false) == Source::ShadowDepth(1, true));
    }
}

#[cfg(test)]
mod destination_tests;

#[cfg(test)]
#[path = "../pack_material_tests.rs"]
mod material_tests;

#[cfg(test)]
#[path = "../pack_paged_maps_tests.rs"]
mod paged_maps_tests;

#[cfg(test)]
mod rgb_alpha_tests;

#[cfg(test)]
mod packed_tests;

#[cfg(test)]
mod trinary_tests;

#[cfg(test)]
mod const_offset_tests;

#[cfg(test)]
mod integer_colortex0_tests;

#[cfg(test)]
mod atlas_mip_tests;

#[cfg(test)]
mod wrong_type_uniform_tests;

#[cfg(test)]
mod atlas_name_post_tests;

#[cfg(test)]
mod unknown_sampler_terrain_tests;

#[cfg(test)]
mod mipmap_box_tests;

#[cfg(test)]
mod colortex_mip_blend_tests;

#[cfg(test)]
mod material_route_tests;
