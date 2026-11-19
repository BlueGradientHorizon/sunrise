//! `PackPost`: a pack's passes and what they run on. `PackPost::new` is
//! `sunrise_render_shaderpack::pack_post::PackPostBuild`'s build; this holds what it makes and
//! the frame's own work (`execution.rs`) is an extension trait on it.

#![allow(unused_imports)]

use std::collections::HashMap;

use sunrise_render_shaderpack::pack_draw::DrawUniforms;
use sunrise_render_shaderpack::pack_post::*;
use sunrise_render_shaderpack::pack_uniforms::{FrameUniforms, PackInputs};
use sunrise_shaders::{BUFFERS, Post};

use super::ShadowView;

pub use sunrise_render_shaderpack::pack_post::construction::*;
pub use sunrise_render_shaderpack::pack_post::types::*;

/// A pack's post-processing pipelines and what they run on (`mod.rs`'s `execution` uses its
/// fields; `new` makes them).
pub struct PackPost {
    pub setup_pending: bool,
    pub notes: Vec<String>,
    pub passes: Vec<Pass>,
    /// Shadow cache for temporal reprojection.
    pub shadow_cache: ShadowCache,
    /// What draws terrain (`gbuffers_terrain` or its fallback), between `prepare` and `deferred`;
    /// when the pack draws terrain in two parts, solid terrain only (`gbuffers_terrain_solid` or
    /// its fallback).
    pub terrain: Option<Pass>,
    /// What draws cutout terrain (`gbuffers_terrain_cutout` or its fallback), after `terrain`,
    /// when the pack draws terrain in two parts. A pack with no program for one part has only
    /// the other.
    pub terrain_cutout: Option<Pass>,
    /// What draws translucent terrain (`gbuffers_water` or its fallback), after `deferred`.
    pub water: Option<Pass>,
    /// The pack has no program for solid and cutout terrain, or for translucent terrain:
    /// nothing draws it (`sunrise_shaders::Post::terrain_undrawn`).
    pub terrain_undrawn: bool,
    pub water_undrawn: bool,
    /// What draws the sky (`gbuffers_skybasic` or its fallback), before terrain, and its vertices.
    pub sky: Option<Sky>,
    /// What draws entities and block entities, after terrain, and their vertices.
    pub entities: Option<Entities>,
    /// What draws the clouds, after entities, and this frame's clouds.
    pub clouds: Option<Pass>,
    /// What draws the block outline (`gbuffers_line`, else `gbuffers_basic`), and this frame's.
    pub lines: Option<Pass>,
    pub line_draw: Option<Lines>,
    pub cloud_draw: Option<Clouds>,
    /// The pack draws the clouds (`gbuffers_clouds` or its fallback), even with a program that
    /// writes nothing (a pack drawing its own clouds discards vanilla's), or `clouds=off`: the
    /// game's clouds are not drawn.
    pub hides_clouds: bool,
    /// The pack has a sky (`gbuffers_skybasic`) or sun and moon (`gbuffers_skytextured`)
    /// program: vanilla's stays hidden even when the program writes nothing.
    pub hides_sky: bool,
    pub hides_celestial: bool,
    /// `colortex0ClearColor`, when the pack sets it.
    pub sky_clear: Option<[f32; 4]>,
    /// What draws rain and snow (the same program, each with its kind's sprite as `texture`),
    /// after translucent terrain, this frame's weather and the sprites' generation bound.
    pub weather: Option<[Pass; 2]>,
    pub weather_draw: Option<Weather>,
    pub weather_generation: u64,
    /// What draws the first-person hand (`gbuffers_hand` or its fallback), after the opaque
    /// world, and its vertices.
    pub hand: Option<(Pass, crate::gfx::pack_hand::PackHand)>,
    /// `gbuffers_hand_water` (or its fallback): the hand's translucent quads, after deferred.
    pub hand_water: Option<Pass>,
    /// The pre-translucent copy: `depthtex1`, or `depthtex2` without a shader hand.
    pub depth1: bool,
    /// A program after the hand reads `depthtex2`: it is copied before the hand.
    pub depth2: bool,
    /// Translucent terrain reads the boats' water patches (`Targets::water_mask`).
    pub water_mask: bool,
    /// The water patches' depth holds patches (or was never cleared): cleared before the next
    /// frame's are drawn.
    pub water_mask_dirty: bool,
    /// The shadow buffers, when the shadow pass runs (2.5), and the `shadowcomp` programs.
    pub shadow_targets: Option<crate::gfx::pack_shadow::ShadowTargets>,
    /// What draws terrain from the light (`shadow`), and translucent terrain (`shadow_water`).
    pub shadow_terrain: Option<Pass>,
    /// `end_portal.png`'s atlas rect this frame, and a texture of its own copied from it that
    /// End portal draws bind as their atlas.
    pub portal_rect: [f32; 4],
    pub portal_texture: Option<([f32; 4], wgpu::TextureView)>,
    pub shadow_water: Option<Pass>,
    pub shadow_comp: Vec<Pass>,
    /// Buffers with mipmaps (filterable ones a program asks them for).
    pub mipped: u16,
    /// Buffers whose mip chain matches their first level, this frame.
    pub mips_valid: u16,
    /// The downsampling pipeline of each format with mipmaps.
    pub down: HashMap<wgpu::TextureFormat, (wgpu::RenderPipeline, wgpu::BindGroupLayout)>,
    /// The per-draw uniforms of gbuffers programs.
    pub draws: DrawUniforms,
    /// Set 3 of flat terrain: an unknown-source fallback and immutable state/sprite tables.
    /// Each page's bind group tracks both its vertex buffer and optional source sidecar.
    pub block_ids: Option<(wgpu::BindGroupLayout, wgpu::Buffer, wgpu::Buffer)>,
    pub page_groups: HashMap<u32, (wgpu::Buffer, Option<wgpu::Buffer>, wgpu::BindGroup)>,
    pub gui_size: [u32; 2],
    /// GPU time of each pass, while the renderer asks for it (`pack_timing.rs`).
    pub timing: Option<crate::gfx::pack_timing::PassTiming>,
    /// `depthtex0` at the screen's centre, read back for `centerDepthSmooth`.
    pub center_depth: Option<crate::gfx::pack_center_depth::CenterDepth>,
    /// `shaders.properties`' custom uniforms, after the frame's in `uniforms.values`.
    pub custom: sunrise_shaders::Custom,
    pub atlas: Option<wgpu::TextureView>,
    pub material_maps: Option<[wgpu::TextureView; 2]>,
    /// The shared atlas's normal and specular paged maps (`set_paged_maps`).
    pub paged_maps: [Option<wgpu::TextureView>; 2],
    pub lightmap: wgpu::TextureView,
    pub deferred_pre: u16,
    pub composite_pre: u16,
    pub formats: [wgpu::TextureFormat; BUFFERS],
    pub clears: [Option<[f32; 4]>; BUFFERS],
    /// Buffers of a three-channel format, and whether their textures are new: then they get
    /// alpha 1 before the next frame.
    pub rgb: u16,
    pub rgb_unset: bool,
    pub used: u16,
    /// Buffers some pass draws into the "alt" side of or flips; the others' B is their A.
    pub doubled: u16,
    /// `size.buffer.*`.
    pub sizes: [Option<sunrise_shaders::BufferSize>; BUFFERS],
    /// Buffers bound as images, and those also copied for a pass that samples them.
    pub storage: u16,
    pub copied: u16,
    /// Bit n set: B is buffer n's "main".
    pub main: u16,
    pub targets: Option<Targets>,
    pub noise: wgpu::TextureView,
    pub dummy: wgpu::TextureView,
    pub dummy_black: wgpu::TextureView,
    pub dummy_normal: wgpu::TextureView,
    pub dummy_3d: wgpu::TextureView,
    pub dummy_1d: wgpu::TextureView,
    /// `image.*` and `bufferObject.*`.
    pub images: Vec<sunrise_render_shaderpack::pack_images::PackImage>,
    /// Set 5 of programs with a geometry stage.
    pub indices: Option<wgpu::BindGroupLayout>,
    pub shadow_culling: sunrise_shaders::ShadowCulling,
    pub gpu: Gpu,
    /// Zeros to clear images by copying, where the device has no `clear_texture`.
    pub zeros: Option<wgpu::Buffer>,
    /// What the images no `image.*` gives bind.
    pub stand_ins: Vec<sunrise_render_shaderpack::pack_images::StandIn>,
    pub dump: Option<sunrise_render_shaderpack::pack_images::Dump>,
    pub buffers: sunrise_render_shaderpack::pack_images::Buffers,
    pub textures: Vec<sunrise_render_shaderpack::pack_textures::PackTexture>,
    /// The stand-in shadow map and its comparison sampler; cleared on the first frame.
    pub shadow: (wgpu::TextureView, wgpu::Sampler),
    pub shadow_cleared: bool,
    pub linear: wgpu::Sampler,
    pub nearest: wgpu::Sampler,
    pub repeat: wgpu::Sampler,
    pub nearest_repeat: wgpu::Sampler,
    pub atlas_sampler: wgpu::Sampler,
    /// colortex buffers' (`SamplerKind::Buffer`).
    pub buffer_sampler: wgpu::Sampler,
    /// None when it would write nothing: colortex0 holds integers and there is no depth to copy
    /// nor buffer to clear.
    pub blit: Option<(wgpu::RenderPipeline, wgpu::BindGroupLayout)>,
    /// The blit also writes depthtex0 (some program reads it) and clears these buffers.
    pub blit_depth: bool,
    pub blit_clears: Vec<usize>,
    /// Buffers cleared in passes of their own (more than the blit's eight attachments).
    pub other_clears: Vec<usize>,
    /// Source views identify the world target, including replacements at the same size.
    pub blit_group: Option<([wgpu::TextureView; 2], wgpu::BindGroup)>,
    /// Shows colortex0 when the pack has no `final` (OQ-5).
    pub copy: (wgpu::RenderPipeline, wgpu::BindGroupLayout),
    /// Copies the picture `final` drew at the world's size onto the window: no flip, it is
    /// already in the screen's rows.
    pub upscale: (wgpu::RenderPipeline, wgpu::BindGroupLayout),
    /// The picture `final` draws while the world's size is not the window's.
    pub final_target: Option<FinalTarget>,
    pub screen: wgpu::TextureFormat,
    pub uniforms: FrameUniforms,
}

impl From<PackPostBuild> for PackPost {
    fn from(built: PackPostBuild) -> Self {
        let PackPostBuild {
            setup_pending,
            notes,
            passes,
            shadow_cache,
            terrain,
            terrain_cutout,
            water,
            terrain_undrawn,
            water_undrawn,
            sky,
            entities,
            clouds,
            line_draw: _,
            cloud_draw: _,
            lines,
            hides_clouds,
            hides_sky,
            hides_celestial,
            sky_clear,
            weather,
            weather_draw,
            weather_generation,
            hand,
            hand_water,
            depth1,
            depth2,
            water_mask,
            water_mask_dirty,
            shadow_targets,
            shadow_terrain,
            portal_rect,
            portal_texture,
            shadow_water,
            shadow_comp,
            mipped,
            mips_valid,
            down,
            draws,
            block_ids,
            page_groups,
            gui_size,
            timing,
            center_depth,
            custom,
            atlas,
            material_maps,
            paged_maps,
            lightmap,
            deferred_pre,
            composite_pre,
            formats,
            clears,
            rgb,
            rgb_unset,
            used,
            doubled,
            sizes,
            storage,
            copied,
            main,
            targets,
            noise,
            dummy,
            dummy_black,
            dummy_normal,
            dummy_3d,
            dummy_1d,
            images,
            indices,
            shadow_culling,
            gpu,
            zeros,
            stand_ins,
            dump,
            buffers,
            textures,
            shadow,
            shadow_cleared,
            linear,
            nearest,
            repeat,
            nearest_repeat,
            atlas_sampler,
            buffer_sampler,
            blit,
            blit_depth,
            blit_clears,
            other_clears,
            blit_group,
            copy,
            upscale,
            final_target,
            screen,
            uniforms,
        } = built;
        Self {
            setup_pending,
            notes,
            passes,
            shadow_cache,
            terrain,
            terrain_cutout,
            water,
            terrain_undrawn,
            water_undrawn,
            sky,
            entities,
            clouds,
            lines,
            line_draw: None,
            cloud_draw: None,
            hides_clouds,
            hides_sky,
            hides_celestial,
            sky_clear,
            weather,
            weather_draw,
            weather_generation,
            hand,
            hand_water,
            depth1,
            depth2,
            water_mask,
            water_mask_dirty,
            shadow_targets,
            shadow_terrain,
            portal_rect,
            portal_texture,
            shadow_water,
            shadow_comp,
            mipped,
            mips_valid,
            down,
            draws,
            block_ids,
            page_groups,
            gui_size,
            timing,
            center_depth,
            custom,
            atlas,
            material_maps,
            paged_maps,
            lightmap,
            deferred_pre,
            composite_pre,
            formats,
            clears,
            rgb,
            rgb_unset,
            used,
            doubled,
            sizes,
            storage,
            copied,
            main,
            targets,
            noise,
            dummy,
            dummy_black,
            dummy_normal,
            dummy_3d,
            dummy_1d,
            images,
            indices,
            shadow_culling,
            gpu,
            zeros,
            stand_ins,
            dump,
            buffers,
            textures,
            shadow,
            shadow_cleared,
            linear,
            nearest,
            repeat,
            nearest_repeat,
            atlas_sampler,
            buffer_sampler,
            blit,
            blit_depth,
            blit_clears,
            other_clears,
            blit_group,
            copy,
            upscale,
            final_target,
            screen,
            uniforms,
        }
    }
}

impl PackPost {
    /// The shared atlas's normal and specular paged maps (`paged_maps.rs`), which the programs
    /// drawing its geometry read as `normals` and `specular`; set before the first frame (a pass's
    /// bind groups are kept once made). A missing one reads as the dummy (flat, no specular).
    pub fn set_paged_maps(&mut self, maps: [Option<wgpu::TextureView>; 2]) {
        self.paged_maps = maps;
    }

    /// Builds the pipelines of `post` for a screen of `screen` format. An error when the pack
    /// can't show anything on the screen: the game turns it off, as a pack that fails to load.
    ///
    /// The pack is laid out first and its pipelines are made after, all at once (`compile`).
    /// When wgpu or the driver refuses one, the pack is built again a program at a time, so that
    /// the program is skipped with its note as it always was.
    ///
    /// `cancel` set (the pack isn't wanted any more) stops the build between programs: a refusal
    /// whose `is_cancelled` is true.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        adapter: &wgpu::Adapter,
        post: Post,
        screen: wgpu::TextureFormat,
        world: World<'_>,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Self, Refused> {
        PackPostBuild::new(device, queue, adapter, post, screen, world, cancel).map(Self::from)
    }
}

/// The frame's work on a `PackPost`, from `take_center_depth` on (`execution.rs` in the client
/// crate; `impl PackPostExt for PackPost`).
pub trait PackPostExt: Sized {
    /// `Renderer::finish_sequence_frame`: the centre depth the finished frame copied.
    fn take_center_depth(&mut self, device: &wgpu::Device) -> Result<(), String>;
    /// Forgets what the pack kept from earlier frames, as a freshly loaded pack's.
    fn reset_history(&mut self, device: &wgpu::Device);
    /// Whether the pack draws the outline itself (or hides it).
    fn has_lines(&self) -> bool;
    /// The block outline to draw this frame (`None`: none).
    fn set_lines(&mut self, lines: Option<Lines>);
    /// Whether the pack draws the clouds itself (or hides them).
    fn has_clouds(&self) -> bool;
    /// The clouds to draw this frame (`None`: none).
    fn set_clouds(&mut self, clouds: Option<Clouds>);
    /// Whether the pack draws rain and snow itself.
    fn has_weather(&self) -> bool;
    /// The rain and snow to draw this frame (`None`: none).
    fn set_weather(&mut self, weather: Option<Weather>);
    fn has_terrain(&self) -> bool;
    fn has_shadow(&self) -> bool;
    fn has_entities(&self) -> bool;
    fn has_hand(&self) -> bool;
    fn has_sky(&self) -> bool;
    fn has_celestial(&self) -> bool;
    fn has_water(&self) -> bool;
    fn sky_clear(&self) -> Option<[f32; 4]>;
    fn programs(&self) -> Vec<&str>;
    /// The shadow pass this frame wants, when the pack draws one.
    fn shadow_view(&self, frame: &crate::gfx::Frame) -> Option<ShadowView>;
    /// The shadow casters the pack's shadow programs draw.
    fn shadow_casters(&self) -> sunrise_render_shaderpack::pack_entities::Casters;
    fn skins_entities(&self) -> bool;
    fn skin_target(&self) -> Option<&wgpu::Buffer>;
    /// The centre depth readback, to start right after the frame's submission.
    fn submitted(&mut self) -> Option<impl FnOnce() + Send + 'static>;
    /// The entities and block entities the pack draws this frame, into its own buffers.
    #[allow(clippy::too_many_arguments)]
    fn prepare_entities(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        entities: &[crate::gfx::Vertex],
        spans: &[crate::gfx::pack_draw::EntitySpan],
        crumbling: [f32; 4],
        portal: [f32; 4],
        rest: &mut Vec<crate::gfx::Vertex>,
        skin: Option<&crate::gfx::entity_skin::SkinFrame>,
        motion: (&[(u32, u64)], &[u32]),
    );
    /// The frame's passes, in order.
    #[allow(clippy::too_many_arguments)]
    fn run(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        world: &wgpu::TextureView,
        world_depth: &wgpu::TextureView,
        screen: &wgpu::TextureView,
        frame: &crate::gfx::Frame,
        inputs: &PackInputs,
        size: [u32; 2],
        window: [u32; 2],
        pages: &[TerrainPage],
        terrain: &mut TerrainDraw<'_>,
    );
}
