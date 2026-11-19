//! Shader pack pipeline implementation for Sunrise (OptiFine / Iris).
//!
//! Provides GPU timing, atlas stitching, passes, and shaderpack uniforms.

pub mod pack_atlas;
pub mod pack_center_depth;
pub mod pack_draw;
pub mod pack_entities;
pub mod pack_hand;
pub mod pack_images;
pub mod pack_lines;
pub mod pack_shadow;
pub mod pack_sky;
pub mod pack_textures;
pub mod pack_timing;
pub mod pack_weather;
pub(crate) mod timing;

#[cfg(test)]
pub(crate) mod test_gpu;
