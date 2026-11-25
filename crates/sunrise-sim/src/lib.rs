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
#[cfg(target_os = "linux")]
pub use sunrise_level::api_context;
pub use sunrise_level::assets;
pub use sunrise_level::audio_capture;
pub use sunrise_level::bidi;
pub use sunrise_level::biome;
pub use sunrise_level::block_edit;
pub use sunrise_level::block_entities;
pub use sunrise_level::block_entity_ticks;
pub use sunrise_level::brewing;
pub use sunrise_level::camera_math;
pub use sunrise_level::carrier_fences;
pub use sunrise_level::chunk;
pub use sunrise_level::chunk_map;
pub use sunrise_level::collisions;
pub use sunrise_level::columns;
pub use sunrise_level::command_block;
pub use sunrise_level::containers;
pub use sunrise_level::crash;
pub use sunrise_level::datapack;
pub use sunrise_level::dyes;
pub use sunrise_level::effects;
pub use sunrise_level::environment;
pub use sunrise_level::fast_hash;
pub use sunrise_level::jigsaw_block;
pub use sunrise_level::light;
pub use sunrise_level::light_border;
pub use sunrise_level::lod_data;
pub use sunrise_level::logging;
pub use sunrise_level::mod_chunk_tickets;
pub use sunrise_level::pack_image;
pub use sunrise_level::pack_meta;
pub use sunrise_level::pack_tags;
pub use sunrise_level::par;
pub use sunrise_level::particle_options;
pub use sunrise_level::perf_capture;
pub use sunrise_level::poi_sections;
pub use sunrise_level::profile_lookup;
pub use sunrise_level::providers;
pub use sunrise_level::random_source;
pub use sunrise_level::sculk;
pub use sunrise_level::section_mask;
pub use sunrise_level::shulker_box;
pub use sunrise_level::signs_data;
pub use sunrise_level::sim_settings;
pub use sunrise_level::sound_events;
pub use sunrise_level::spans;
pub use sunrise_level::structure_block;
pub use sunrise_level::test_block;
pub use sunrise_level::test_instance_block;
pub use sunrise_level::tick_mode;
pub use sunrise_level::vault;
pub use sunrise_level::world;
pub use sunrise_level::worldborder;
// --- end lowered level re-exports ---

// --- lowered modules ---
pub mod api_text;
pub mod attack_strength;
pub mod bed_geometry;
pub mod block_pattern;
pub mod block_transform;
pub mod colliders;
pub mod cushion;
pub mod data_cmd;
pub mod enchantment_levels;
pub mod entity_rules;
pub mod entity_solids;
pub mod first_person_sim;
pub mod gossip;
pub mod item_attributes;
pub mod join_seq;
pub mod living_attributes;
pub mod map_decoration_types;
pub mod menu_extra_net;
pub mod native_stack_hooks;
pub mod nether_portal;
pub mod perf_types;
pub mod powder_snow;
pub mod scoreboard_cmd;
pub mod spear_sim;
pub mod text_grammar;
pub mod ticket_levels;
pub mod travel_gate;
pub mod viewed_levels_net;
pub mod world_spawn;
// --- end lowered modules ---
