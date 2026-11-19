//! The block atlas of shader packs (re-exported from sunrise_render_shaderpack).

pub use sunrise_render_shaderpack::pack_atlas::*;

#[cfg(test)]
#[path = "pack_atlas/fade_test.rs"]
mod fade_test;

#[cfg(test)]
#[path = "pack_atlas/midpoint_test.rs"]
mod midpoint_test;

#[cfg(test)]
#[path = "pack_atlas/source_ownership_test.rs"]
mod source_ownership_test;
