//! The first-person hand and held items drawn by a shader pack's `gbuffers_hand` (re-exported from sunrise_render_shaderpack).
pub use sunrise_render_shaderpack::pack_hand::*;

/// What `gbuffers_hand` reads: the entity inputs, and the hand's projection from set 1.
pub fn inputs() -> sunrise_shaders::GbuffersInputs {
    adapt_hand_inputs(super::pack_entities::inputs())
}
