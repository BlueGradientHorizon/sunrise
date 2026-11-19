//! A shader pack's post-processing over Sunrise's own world (spec 2.1 rows 4, 6, 8, 9; 6.1; 6.2;
//! 6.4): the world renders into its own target as with a resolution scale, its colour becomes
//! colortex0 and its depth `depthtex0`, then the pack's `prepare`, `deferred`, `composite` and
//! `final` programs run in order, and `final` draws onto the screen under the hand and HUD.
//!
//! What the pack is and what it builds (`types`, `construction`: its entries and exits, its
//! passes, buffers and bindings, and `PackPost::new`) lives in sunrise_render_shaderpack's
//! `pack_post`; this module runs it. Its `types` and `construction` re-export that crate's,
//! and `PackPost` wraps what `new` makes, so the names the game and the tests reach here are
//! unchanged.
//
// - `types`: the pack's entries and exits and the puzzle of passes, buffers and bindings.
// - `construction`: `PackPost` itself and `PackPost::new`.
// - `execution`: from `take_center_depth` on: the frame's passes, its flips and resizes.
mod construction;
mod execution;
mod types;

// The names other modules reach through `gfx::pack_post` keep the paths and visibility they had.
pub(crate) use construction::{Geometry, TerrainList, TerrainPage};
pub use construction::{PackPost, PackPostExt};
pub use sunrise_render_shaderpack::pack_post::Refused;
// The tests' fixtures reach these where they used to (one file had one namespace).
#[cfg(test)]
pub(crate) use construction::{TerrainDraw, block_ids, terrain_tables};
#[allow(unused_imports)]
pub(crate) use types::{Clouds, Lines, Weather, World};

// Every other name the tests reach unqualified through `super::*` (their modules name only
// what they use, as the one file's tests did). The traits come first: a glob import names a
// trait only as a value.
#[cfg(test)]
#[allow(unused_imports)]
use construction::PackPostExt as _;
#[cfg(test)]
#[allow(unused_imports)]
use sunrise_render_shaderpack::pack_post::*;
#[cfg(test)]
#[allow(unused_imports)]
use types::*;

pub use construction::PackPostExt as _;

pub(crate) use super::external::ShadowView;

// What `execution` and the tests reach through `super::*` (as they did when this was one file).
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
#[allow(unused_imports)]
use glam::{DMat4, DVec3};
#[allow(unused_imports)]
use smallvec::SmallVec;
#[allow(unused_imports)]
use std::collections::HashMap;
#[cfg(test)]
use sunrise_render_shaderpack::pack_post::{
    ComputeJob, Job, Resources, Slot, Source, bindings, build_pass, checked, compile, sampler,
};
#[allow(unused_imports)]
use sunrise_render_shaderpack::pack_post::{
    FinalTarget, Gpu, Image, Mips, Pass, Pipeline, SINK, ShadowCache, ShadowCacheKey, Sky, Targets,
    attach, befores, image_buffers, share_samplers,
};
#[allow(unused_imports)]
use sunrise_shaders::{BUFFERS, Format, Group, Post, SHADOW_COLORS};

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
mod terrain_parts_tests;

#[cfg(test)]
mod clear_budget_tests;

#[cfg(test)]
mod clear_false_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod depth_test;

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
