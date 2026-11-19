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
    EntityVertex, FLUID_OVERLAY, HURT_TEXEL, OIT_ENTITY, PosedModelRuns, SkinRunJob,
    TERRAIN_MODEL_CUTOUT_MIPPED_OVERLAY, TERRAIN_MODEL_CUTOUT_OVERLAY, TERRAIN_MODEL_SOLID_OVERLAY,
    TRANSLUCENT_GLINT_ITEM_OVERLAY, TRANSLUCENT_ITEM_OVERLAY, TRANSLUCENT_PARTICLE_OVERLAY,
    WATER_MASK_OVERLAY, WHITE_OVERLAY, is_native_terrain, skin_runs, stage_light, white_alpha,
};
