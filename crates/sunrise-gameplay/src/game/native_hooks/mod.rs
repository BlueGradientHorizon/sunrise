
#[cfg(test)]
pub(in crate::game) mod item_picked_up_tests;
#[cfg(any(test, feature = "test-support"))]
pub mod on_block_placed_by_tests;
pub(in crate::game) mod xp_repair_ratio;

pub(in crate::game) mod xp_orb_targeting;

pub(in crate::game) mod wind_burst;

pub(in crate::game) mod vanilla_game_event;

pub(in crate::game) mod validate_tickets;

pub(in crate::game) mod teleport_command;

pub(in crate::game) mod spread_players_teleport;

pub(in crate::game) mod set_placed_by;

#[path = "cut_merged_server_tick_rate_changed.rs"]
pub(in crate::game) mod server_tick_rate_changed;

pub(in crate::game) mod server_chat;

pub(in crate::game) mod respawn_position;

pub(in crate::game) mod register_game_tests;

pub(in crate::game) mod register_configuration_tasks;

pub(in crate::game) mod recipe_sources;

pub(in crate::game) mod recipe_book_place;

pub(in crate::game) mod potion_brew_pre;

pub(in crate::game) mod potion_brew_post;

pub(in crate::game) mod player_xp_level_change;

pub(in crate::game) mod player_xp_change;

pub(in crate::game) mod player_swap_hands;

pub(in crate::game) mod player_start_spectating_entity;

pub(in crate::game) mod player_spawn_phantoms;

pub(in crate::game) mod player_set_spawn;

pub(in crate::game) mod player_pong;

pub(in crate::game) mod player_passage_cross;

pub(in crate::game) mod player_move;

pub(in crate::game) mod player_item_smithed;

pub(in crate::game) mod player_inventory_changed;

pub(in crate::game) mod player_game_mode_changed;

pub(in crate::game) mod player_flyable_fall;

pub(in crate::game) mod player_enchant_item;

pub(in crate::game) mod player_destroyed_block;

pub(in crate::game) mod player_brewed_potion;

pub(in crate::game) mod permissions_changed;

pub(in crate::game) mod on_item_use_first;

pub(in crate::game) mod on_dropped_by_player;

pub(in crate::game) mod on_destroyed_by_player;

pub(in crate::game) mod on_block_placed_by;

pub(in crate::game) mod on_block_exploded;

pub(in crate::game) mod native_village_siege;

pub(in crate::game) mod native_potential_spawns;

pub(in crate::game) mod mob_split;

pub(in crate::game) mod mob_spawn_checks;

pub(in crate::game) mod mob_effect_events;

pub(in crate::game) mod minecart_pass;

pub(in crate::game) mod menu_transfer;

pub(in crate::game) mod menu_slot_changed;

pub(in crate::game) mod menu_links;

pub(in crate::game) mod menu_extra_data;

pub(in crate::game) mod makes_piglins_neutral;

pub(in crate::game) mod living_visibility;

pub(in crate::game) mod living_experience_drop;

pub(in crate::game) mod living_drops;

pub(in crate::game) mod living_destroy_block;

#[path = "cut_merged_living_death_post.rs"]
pub mod living_death_post;

pub(in crate::game) mod living_death_message;

pub(in crate::game) mod living_conversion_pre;

pub(in crate::game) mod living_conversion_post;

#[path = "cut_merged_living_breathe.rs"]
pub(in crate::game) mod living_breathe;

pub(in crate::game) mod level_save;

#[path = "cut_merged_item_toss.rs"]
pub(in crate::game) mod item_toss;

pub(in crate::game) mod item_smelted;

#[path = "cut_merged_item_picked_up.rs"]
pub(in crate::game) mod item_picked_up;

pub(in crate::game) mod item_fished;

pub(in crate::game) mod item_destroyed;

pub(in crate::game) mod item_crafted;

pub(in crate::game) mod is_primary_item_for;

pub(in crate::game) mod is_possible_to_respawn_in;

pub(in crate::game) mod is_pathfindable;

pub(in crate::game) mod is_not_replaceable_by_pick_action;

pub(in crate::game) mod grindstone_take;

pub(in crate::game) mod grindstone_place;

pub(in crate::game) mod game_rule_changed;

pub(in crate::game) mod fire;

pub(in crate::game) mod finalize_spawn;

#[path = "cut_merged_farmland_trample.rs"]
pub(in crate::game) mod farmland_trample;

pub(in crate::game) mod falling_tick;

pub(in crate::game) mod explosion_start;

pub(in crate::game) mod explosion_knockback;

pub(in crate::game) mod explosion_detonate;

pub(in crate::game) mod entity_travel_to_dimension;

pub(in crate::game) mod entity_struck_by_lightning;

pub(in crate::game) mod entity_place;

pub(in crate::game) mod entity_mob_griefing;

pub(in crate::game) mod entity_interact_specific;

#[path = "cut_merged_entity_inside.rs"]
pub mod entity_inside;

pub(in crate::game) mod entity_change_block_pre;

pub(in crate::game) mod enderman_anger;

pub(in crate::game) mod enchantment_level_set;

pub(in crate::game) mod enchanted_entity_loot;

pub(in crate::game) mod empty_contents;

pub(in crate::game) mod effect_particle_modification;

pub(in crate::game) mod dispense_method;

#[path = "cut_merged_damage_item.rs"]
pub(in crate::game) mod damage_item;

pub(in crate::game) mod custom_spawners;

pub(in crate::game) mod custom_click_action;

pub(in crate::game) mod create_spawn_position;

pub(in crate::game) mod crafting_remainder;

pub(in crate::game) mod console_line;

pub(in crate::game) mod clone_item_stack;

#[path = "cut_merged_client_information_updated.rs"]
pub(in crate::game) mod client_information_updated;

pub(in crate::game) mod chunk_watch;

pub(in crate::game) mod chunk_ticket_level_updated;

pub(in crate::game) mod chat_recipient;

pub(in crate::game) mod can_player_sleep;

pub(in crate::game) mod can_entity_destroy;

pub(in crate::game) mod can_continue_sleeping;

pub(in crate::game) mod block_use_without_item;

pub(in crate::game) mod block_use_item_on;

pub(in crate::game) mod baby_entity_spawn;

pub(in crate::game) mod arrow_nock;

pub(in crate::game) mod armor_hurt;

pub(in crate::game) mod apply_enchantments;

pub(in crate::game) mod animal_tame;

pub(in crate::game) mod adjacent_block_path_type;

pub(in crate::game) mod add_server_reload_listeners;

include!("mod_server.rs");
