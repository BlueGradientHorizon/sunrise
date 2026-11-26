

#[cfg(test)]
pub(in crate::game) mod animate_tick_tests;
#[cfg(test)]
pub(in crate::game) mod block_entity_client_tick_tests;
#[cfg(test)]
pub(in crate::game) mod block_entity_neighbors_tests;
#[cfg(test)]
pub(in crate::game) mod can_be_connected_to_tests;
#[cfg(test)]
pub(in crate::game) mod client_chunk_hooks_tests;
#[cfg(feature = "client")]
#[cfg(test)]
pub(in crate::game) mod client_pick;
#[cfg(test)]
pub(in crate::game) mod creative_tabs_read_tests;
#[cfg(test)]
pub(in crate::game) mod death_source_tests;
#[cfg(test)]
pub(in crate::game) mod enchantment_and_lang_tests;
#[cfg(test)]
pub(in crate::game) mod fence_gate_in_wall_tests;
#[cfg(test)]
pub(in crate::game) mod fluid_draw;
#[cfg(test)]
pub(in crate::game) mod guest_death_lifecycle_tests;
#[cfg(test)]
pub(in crate::game) mod on_block_state_change_tests;
#[cfg(test)]
pub(in crate::game) mod on_chunk_unloaded_tests;
#[cfg(test)]
pub(in crate::game) mod on_destroyed_by_push_reaction_tests;
#[cfg(test)]
pub(in crate::game) mod on_entity_item_update;
#[cfg(test)]
pub(in crate::game) mod piston_push_reaction_tests;
#[cfg(test)]
pub(in crate::game) mod placement_context_tests;
#[cfg(test)]
pub(in crate::game) mod play_hit_sound_tests;
#[cfg(test)]
pub(in crate::game) mod registered_potions_tests;
#[cfg(test)]
pub(in crate::game) mod remove_block_tests;
#[cfg(test)]
pub(in crate::game) mod trident_family_tests;
pub(in crate::game) mod tick_target_millis;

include!("shared.rs");

pub(in crate::game) mod server_tick_rate_changed;

pub(in crate::game) mod runtime_environment;

pub(in crate::game) mod render_tooltip_texture;

pub(in crate::game) mod render_gui_post;

pub(in crate::game) mod render_frame_pre;

pub(in crate::game) mod register_preset_editors;

pub(in crate::game) mod player_leave;

pub(in crate::game) mod player_heart_type;

pub(in crate::game) mod player_game_mode_changed;

pub(in crate::game) mod player_bed_leave;

pub(in crate::game) mod mod_mismatch;

pub(in crate::game) mod math;

pub(in crate::game) mod living_death_post;

pub(in crate::game) mod level_tick_tests;

pub(in crate::game) mod item_toss;

pub(in crate::game) mod item_destroyed;

pub(in crate::game) mod ingredient_test;

pub(in crate::game) mod fluid_tick;

pub(in crate::game) mod flip_frame;

pub(in crate::game) mod farmland_trample;

pub(in crate::game) mod entity_place;

pub(in crate::game) mod entity_interact_specific;

pub(in crate::game) mod entity_inside;

pub(in crate::game) mod customize_boss_event_progress;

pub(in crate::game) mod component_hover;

pub(in crate::game) mod client_visual_text;

pub(in crate::game) mod client_text;

pub(in crate::game) mod client_packet_send;

pub(in crate::game) mod client_information_updated;

pub(in crate::game) mod build_creative_mode_tab_contents;

pub(in crate::game) mod block_landing;

pub(in crate::game) mod block_entity_save_additional_tests;

pub(in crate::game) mod block_entity_on_load_tests;

pub(in crate::game) mod block_entity_on_load_routes_tests;

pub(in crate::game) mod base;
