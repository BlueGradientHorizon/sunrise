//! Portable simulation engines built on `sunrise-level`.
//! Zero desktop, GPU, window, audio, gamepad, clipboard or HTTP dependencies.
#![forbid(unsafe_code)]

#[cfg(test)]
mod test_log;

pub use sunrise_assets::resource_source;
pub use sunrise_assets::zip;
pub use sunrise_client_core::Result;
pub use sunrise_client_core::block_entity_nbt;
pub use sunrise_client_core::block_entity_types;
pub use sunrise_client_core::bossbar;
pub use sunrise_client_core::component;
pub use sunrise_client_core::fastutil_order;
pub use sunrise_client_core::gamerules;
pub use sunrise_client_core::lang;
pub use sunrise_client_core::nbt_path;
pub use sunrise_client_core::scoreboard;
pub use sunrise_client_core::snbt;
pub use sunrise_client_core::text;
pub use sunrise_command as command;
pub use sunrise_core::json_nbt;
pub use sunrise_items::blocks;
pub use sunrise_items::bundle;
pub use sunrise_items::cooldowns;
pub use sunrise_items::enchant;
pub use sunrise_items::flowers;
pub use sunrise_items::item_data;
pub use sunrise_items::items;
pub use sunrise_items::random_sequences;
pub use sunrise_world::json_folder;
pub use sunrise_world::moving_colliders;

// --- lowered level re-exports ---
pub use sunrise_level::api_context;
pub use sunrise_level::bidi;
pub use sunrise_level::carrier_fences;
pub use sunrise_level::columns;
pub use sunrise_level::crash;
pub use sunrise_level::fast_hash;
pub use sunrise_level::jigsaw_block;
pub use sunrise_level::logging;
pub use sunrise_level::mod_chunk_tickets;
pub use sunrise_level::pack_tags;
pub use sunrise_level::par;
pub use sunrise_level::providers;
pub use sunrise_level::random_source;
pub use sunrise_level::section_mask;
pub use sunrise_level::sound_events;
pub use sunrise_level::spans;
pub use sunrise_level::structure_block;
pub use sunrise_level::test_block;
pub use sunrise_level::test_instance_block;
pub use sunrise_level::worldborder;
// --- end lowered level re-exports ---

// --- lowered modules ---
// --- end lowered modules ---
