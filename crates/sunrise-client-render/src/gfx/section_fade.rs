//! How far through vanilla's chunk fade-in a section is, and the instance that carries it to `terrain.wgsl`.
use crate::fast_hash::FxHashMap;
use std::time::Instant;

pub use sunrise_render_core::{
    CHUNK_VISIBILITY_THRESHOLD, chunk_visibility, fade_millis, section_instance,
};
/// `chunk_visibility` at `now` of a section first uploaded at `uploaded`
/// (`Renderer::section_uploaded`; none: one that fades in over far terrain, or
/// whose column left), with the chunk fade-in `fade_ms`.
pub(super) fn section_visibility(uploaded: Option<&Instant>, fade_ms: u64, now: Instant) -> f32 {
    uploaded.map_or(1.0, |&t| {
        let elapsed = now.saturating_duration_since(t).as_millis() as u64;
        chunk_visibility(elapsed, fade_ms)
    })
}
/// What a section's fade depends on besides the section itself
/// (`SectionGpu::drawn_faded`): the time, the chunk fade-in time, the sections'
/// first uploads (`Renderer::section_uploaded`) and those held while far
/// terrain comes in (`Renderer::lod_retiring`).
#[derive(Clone, Copy)]
pub(super) struct FadeInputs<'a> {
    pub(super) now: Instant,
    pub(super) fade_ms: u64,
    pub(super) uploaded: &'a FxHashMap<super::SectionKey, Instant>,
    pub(super) retiring: &'a crate::fast_hash::FxHashSet<super::SectionKey>,
}
