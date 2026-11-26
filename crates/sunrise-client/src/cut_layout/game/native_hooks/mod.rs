

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
pub(crate) mod tick_target_millis;

include!("shared.rs");

pub(crate) mod server_tick_rate_changed;

pub(crate) mod runtime_environment;

pub(crate) mod render_tooltip_texture;

pub(crate) mod render_gui_post;

pub(crate) mod render_frame_pre;

pub(crate) mod register_preset_editors;

pub(crate) mod player_leave;

pub(crate) mod player_heart_type;

pub(crate) mod player_game_mode_changed;

pub(crate) mod player_bed_leave;

pub(crate) mod mod_mismatch;

pub(crate) mod math;

pub(crate) mod living_death_post;

pub(crate) mod level_tick_tests;

pub(crate) mod item_toss;

pub(crate) mod item_destroyed;

pub(crate) mod ingredient_test;

pub(crate) mod fluid_tick;

pub(crate) mod flip_frame;

pub(crate) mod farmland_trample;

pub(crate) mod entity_place;

pub(crate) mod entity_interact_specific;

pub(crate) mod entity_inside;

pub(crate) mod customize_boss_event_progress;

pub(crate) mod component_hover;

pub(crate) mod client_visual_text;

pub(crate) mod client_text;

pub(crate) mod client_packet_send;

pub(crate) mod client_information_updated;

pub(crate) mod build_creative_mode_tab_contents;

pub(crate) mod block_landing;

pub(crate) mod block_entity_save_additional_tests;

pub(crate) mod block_entity_on_load_tests;

pub(crate) mod block_entity_on_load_routes_tests;

pub(crate) mod base;
