//! Portable level data, chunks, lighting, block entities, biomes and engine utilities.
//! Zero desktop, GPU, window, audio, gamepad, clipboard or HTTP dependencies.
#![forbid(unsafe_code)]

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
pub use sunrise_items::blocks;
pub use sunrise_items::bundle;
pub use sunrise_items::cooldowns;
pub use sunrise_items::enchant;
pub use sunrise_items::item_data;
pub use sunrise_items::items;
pub use sunrise_items::random_sequences;
pub use sunrise_world::json_folder;

// --- lowered modules ---
// --- end lowered modules ---
