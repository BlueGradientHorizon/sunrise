//! Entering a world without blocking the window: the deferred entry, world creation, the world
//! resources' background load, and the world's own content tables.
use std::sync::atomic::{AtomicU64, Ordering};

pub(in crate::game) static BIOME_KEYS_SESSION: AtomicU64 = AtomicU64::new(1);

pub(in crate::game) fn next_biome_keys_session() -> u64 {
    BIOME_KEYS_SESSION.fetch_add(1, Ordering::Relaxed)
}
