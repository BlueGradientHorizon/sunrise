//! Core rendering types, GPU layouts, and geometric primitives for Sunrise.

#![no_std]

extern crate alloc;

pub mod cull;
pub mod frustum;
pub mod random;
pub mod stars;
pub mod terrain;

pub use cull::{CODE_OFFSET, FADING, Record, UNBOUNDED, rounded_box};
pub use frustum::{Cull, Frusta, Frustum, JoinedFrusta, MAX_WORLD_VIEWS};
pub use terrain::{
    BRIGHT, CHUNK_VISIBILITY_THRESHOLD, EntitySpan, FLAT_NORMAL, INCANDESCENT, LOW, NO_SPECULAR,
    OPAQUE_LAYER, SectionKey, TerrainSource, TerrainVertex, UV_STEPS, VERY_BRIGHT, Vertex, WHOLE,
    WeatherTime, WeatherVertex, chunk_visibility, emitting, fade_millis, section_instance,
};

pub mod entity;
pub use entity::{
    EntityVertex, TRANSLUCENT_GLINT_ITEM_OVERLAY, TRANSLUCENT_ITEM_OVERLAY, WATER_MASK_OVERLAY,
};

pub mod frame_view;
pub use frame_view::{FogView, MAX_DYNAMIC_LIGHTS, SkyView, UniformFrameView};
