//! Core rendering primitives, math layouts, and GPU POD types for Sunrise.

pub mod cull;
pub mod frustum;
pub mod terrain;

pub use cull::{BoundingBox, FrustumGpu, Record, SectionMeta, VisibleSection};
pub use frustum::Frustum;
pub use terrain::{
    BRIGHT, FLAT_NORMAL, INCANDESCENT, LOW, NO_SPECULAR, OPAQUE_LAYER, SectionKey, TerrainSource,
    TerrainVertex, UV_STEPS, VERY_BRIGHT, Vertex, WHOLE, chunk_visibility, emitting, fade_millis,
    section_instance,
};
