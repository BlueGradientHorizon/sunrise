//! Server-owned fields of [`super::Game`].
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;

#[path = "network/lan_actions/acting_state.rs"]
pub(in crate::game) mod acting_state;
#[path = "interface/advancements/mod.rs"]
pub(in crate::game) mod advancements;
#[path = "entities/ahead.rs"]
pub(in crate::game) mod ahead;
#[path = "mobs/allays.rs"]
pub(in crate::game) mod allays;
#[path = "modding/api_values.rs"]
pub(crate) mod api_values;
#[cfg(test)]
#[path = "entities/armor_stand_refind_tests.rs"]
pub(in crate::game) mod armor_stand_refind_tests;
#[path = "entities/armor_stands.rs"]
pub(in crate::game) mod armor_stands;
#[path = "modding/attachment_index/mod.rs"]
pub(in crate::game) mod attachment_index;
#[path = "blocks/beds.rs"]
pub(in crate::game) mod beds;
#[path = "mobs/beehives.rs"]
pub(in crate::game) mod beehives;
#[cfg(test)]
#[path = "blocks/block_drops_pops_tests.rs"]
pub(in crate::game) mod block_drops_pops_tests;
#[path = "blocks/block_edits.rs"]
pub(in crate::game) mod block_edits;
#[path = "blocks/block_entity_anims.rs"]
pub(in crate::game) mod block_entity_anims;
#[cfg(test)]
#[path = "modding/block_entity_part_state_tests.rs"]
pub(in crate::game) mod block_entity_part_state_tests;
#[path = "blocks/block_particles.rs"]
pub(in crate::game) mod block_particles;
#[path = "blocks/block_ticks.rs"]
pub(in crate::game) mod block_ticks;
#[path = "modding/block_use_completion.rs"]
pub(in crate::game) mod block_use_completion;
#[cfg(test)]
#[path = "modding/block_use_completion_tests.rs"]
pub(in crate::game) mod block_use_completion_tests;
#[path = "entities/boarding.rs"]
pub(in crate::game) mod boarding;
#[path = "entities/boat_authority_fixture.rs"]
pub(in crate::game) mod boat_authority_fixture;
#[path = "entities/boat_leads.rs"]
pub(in crate::game) mod boat_leads;
#[path = "entities/boat_passengers.rs"]
pub(in crate::game) mod boat_passengers;
#[path = "entities/boats/mod.rs"]
pub(in crate::game) mod boats;
#[path = "interface/bossbars.rs"]
pub(in crate::game) mod bossbars;
#[path = "blocks/brewing_remainder.rs"]
pub(in crate::game) mod brewing_remainder;
#[path = "blocks/brush.rs"]
pub(in crate::game) mod brush;
#[path = "items/bucket_empty.rs"]
pub(in crate::game) mod bucket_empty;
#[path = "items/buckets.rs"]
pub(in crate::game) mod buckets;
#[path = "interface/building_players.rs"]
pub(in crate::game) mod building_players;
#[path = "blocks/campfire.rs"]
pub(in crate::game) mod campfire;
#[path = "blocks/campfire_douse.rs"]
pub(in crate::game) mod campfire_douse;
#[path = "world/carrier_transfers.rs"]
pub(in crate::game) mod carrier_transfers;
#[path = "blocks/cauldron_interactions.rs"]
pub(in crate::game) mod cauldron_interactions;
#[cfg(test)]
#[path = "blocks/cauldron_underwater_tests.rs"]
pub(in crate::game) mod cauldron_underwater_tests;
#[cfg(test)]
#[path = "administration/clone_dimensions_tests.rs"]
pub(in crate::game) mod clone_dimensions_tests;
#[path = "items/combat.rs"]
pub(in crate::game) mod combat;
#[path = "administration/command_harness.rs"]
pub(in crate::game) mod command_harness;
#[path = "administration/command_minecarts_server.rs"]
pub(in crate::game) mod command_minecarts_server;
#[path = "administration/commands/mod.rs"]
pub(in crate::game) mod commands;
#[path = "blocks/composter.rs"]
pub(in crate::game) mod composter;
#[path = "items/consume.rs"]
pub(in crate::game) mod consume;
#[path = "world/container_loot.rs"]
pub(in crate::game) mod container_loot;
#[path = "mobs/creaking.rs"]
pub(in crate::game) mod creaking;
#[path = "items/crossbow.rs"]
pub(in crate::game) mod crossbow;
#[path = "entities/cushions.rs"]
pub(crate) mod cushions;
#[cfg(test)]
#[path = "items/damage_item_tests.rs"]
pub(in crate::game) mod damage_item_tests;
#[cfg(test)]
#[path = "items/damage_resistant_tests.rs"]
pub(in crate::game) mod damage_resistant_tests;
#[path = "blocks/decorated_pot.rs"]
pub(in crate::game) mod decorated_pot;
#[path = "interface/dedicated/mod.rs"]
pub(crate) mod dedicated;
#[path = "items/deflect.rs"]
pub(in crate::game) mod deflect;
#[cfg(test)]
#[path = "modding/disabled_block_tests.rs"]
pub(in crate::game) mod disabled_block_tests;
#[path = "interface/dispense_fixture.rs"]
pub(in crate::game) mod dispense_fixture;
#[path = "blocks/dispenser.rs"]
pub(in crate::game) mod dispenser;
#[path = "mobs/dragon.rs"]
pub(in crate::game) mod dragon;
#[cfg(test)]
#[path = "mobs/dragon_fireball_motion_tests.rs"]
pub(in crate::game) mod dragon_fireball_motion_tests;
#[path = "modding/dropped_item_setter.rs"]
pub(in crate::game) mod dropped_item_setter;
#[path = "world/dynamic_light_emission.rs"]
pub(in crate::game) mod dynamic_light_emission;
#[path = "items/enchant_effects.rs"]
pub(in crate::game) mod enchant_effects;
#[cfg(test)]
#[path = "items/enchant_pick_parts_tests.rs"]
pub(in crate::game) mod enchant_pick_parts_tests;
#[path = "world/end.rs"]
pub(in crate::game) mod end;
#[cfg(test)]
#[path = "mobs/enderman_hurt_teleport_tests.rs"]
pub(in crate::game) mod enderman_hurt_teleport_tests;
#[path = "entities/entity.rs"]
pub(in crate::game) mod entity;
#[cfg(test)]
#[path = "entities/entity_die_tests.rs"]
pub(in crate::game) mod entity_die_tests;
#[path = "modding/entity_initialization.rs"]
pub(in crate::game) mod entity_initialization;
#[cfg(test)]
#[path = "modding/entity_part_state_tests.rs"]
pub(in crate::game) mod entity_part_state_tests;
#[path = "entities/entity_residency.rs"]
pub(in crate::game) mod entity_residency;
#[path = "modding/entity_sizes.rs"]
pub(in crate::game) mod entity_sizes;
#[path = "entities/explosion.rs"]
pub(in crate::game) mod explosion;
#[path = "entities/explosion_detonate.rs"]
pub(in crate::game) mod explosion_detonate;
#[path = "entities/explosion_settings.rs"]
pub(in crate::game) mod explosion_settings;
#[path = "entities/extinguish.rs"]
pub(in crate::game) mod extinguish;
#[path = "blocks/falling_blocks.rs"]
pub(in crate::game) mod falling_blocks;
#[path = "entities/falling_damage.rs"]
pub(in crate::game) mod falling_damage;
#[path = "entities/falling_save.rs"]
pub(in crate::game) mod falling_save;
#[path = "entities/falling_tick.rs"]
pub(in crate::game) mod falling_tick;
#[path = "mobs/fangs.rs"]
pub(in crate::game) mod fangs;
#[cfg(test)]
#[path = "mobs/fence_leash_force_tests.rs"]
pub(in crate::game) mod fence_leash_force_tests;
#[path = "mobs/finalize_spawn.rs"]
pub(in crate::game) mod finalize_spawn;
#[cfg(test)]
#[path = "blocks/first_tick_registration_tests.rs"]
pub(in crate::game) mod first_tick_registration_tests;
#[path = "items/fishing.rs"]
pub(in crate::game) mod fishing;
#[path = "modding/fluid_interactions.rs"]
pub(in crate::game) mod fluid_interactions;
#[path = "mobs/foxes.rs"]
pub(in crate::game) mod foxes;
#[path = "entities/frames.rs"]
pub(in crate::game) mod frames;
#[path = "administration/functions.rs"]
pub(in crate::game) mod functions;
#[path = "interface/game_mode.rs"]
pub(in crate::game) mod game_mode;
#[path = "administration/game_tests.rs"]
pub(in crate::game) mod game_tests;
#[path = "entities/general.rs"]
pub(in crate::game) mod general;
#[cfg(test)]
#[path = "mobs/golem_defense_tests.rs"]
pub(in crate::game) mod golem_defense_tests;
#[path = "mobs/golems.rs"]
pub(in crate::game) mod golems;
#[cfg(test)]
#[path = "modding/grindstone_decisions_tests.rs"]
pub(in crate::game) mod grindstone_decisions_tests;
#[cfg(test)]
#[path = "mobs/hoglin_repeat_attack_tests.rs"]
pub(in crate::game) mod hoglin_repeat_attack_tests;
#[cfg(test)]
#[path = "modding/hook_meddler_tests.rs"]
pub(in crate::game) mod hook_meddler_tests;
#[path = "runtime/hot_paths/mod.rs"]
pub(in crate::game) mod hot_paths;
#[path = "world/infiniburn.rs"]
pub(in crate::game) mod infiniburn;
#[cfg(test)]
#[path = "world/infiniburn_tests.rs"]
pub(in crate::game) mod infiniburn_tests;
#[path = "world/initial_spawn.rs"]
pub(in crate::game) mod initial_spawn;
#[cfg(test)]
#[path = "administration/inline_feature_parse_tests.rs"]
pub(in crate::game) mod inline_feature_parse_tests;
#[cfg(test)]
#[path = "administration/inline_loot_reference_tests.rs"]
pub(in crate::game) mod inline_loot_reference_tests;
#[path = "world/integrated.rs"]
pub(in crate::game) mod integrated;
#[cfg(test)]
#[path = "entities/interaction_data_tests.rs"]
pub(in crate::game) mod interaction_data_tests;
#[cfg(test)]
#[path = "items/item_abilities_tests.rs"]
pub(in crate::game) mod item_abilities_tests;
#[path = "items/item_actions.rs"]
pub(in crate::game) mod item_actions;
#[cfg(test)]
#[path = "entities/item_contact_tests.rs"]
pub(in crate::game) mod item_contact_tests;
#[cfg(test)]
#[path = "modding/item_drop_state_tests.rs"]
pub(in crate::game) mod item_drop_state_tests;
#[path = "items/item_durability.rs"]
pub(in crate::game) mod item_durability;
#[path = "items/item_entities.rs"]
pub(in crate::game) mod item_entities;
#[path = "items/item_entity_spawn.rs"]
pub(in crate::game) mod item_entity_spawn;
#[path = "entities/item_hurt.rs"]
pub(in crate::game) mod item_hurt;
#[cfg(test)]
#[path = "items/item_pickup_shape_tests.rs"]
pub(in crate::game) mod item_pickup_shape_tests;
#[path = "items/item_use.rs"]
pub(in crate::game) mod item_use;
#[cfg(test)]
#[path = "entities/item_void_tests.rs"]
pub(in crate::game) mod item_void_tests;
#[path = "modding/journal_level_write.rs"]
pub(in crate::game) mod journal_level_write;
#[cfg(test)]
#[path = "entities/kill_die_vibration_tests.rs"]
pub(in crate::game) mod kill_die_vibration_tests;
#[cfg(test)]
#[path = "entities/knot_residency_tests.rs"]
pub(in crate::game) mod knot_residency_tests;
#[path = "network/lan/mod.rs"]
pub(in crate::game) mod lan;
#[path = "network/lan_camera.rs"]
pub(in crate::game) mod lan_camera;
#[path = "network/lan_chat.rs"]
pub(in crate::game) mod lan_chat;
#[path = "network/lan_configuration_tasks.rs"]
pub(in crate::game) mod lan_configuration_tasks;
#[path = "network/lan_crafting.rs"]
pub(in crate::game) mod lan_crafting;
#[path = "network/lan_entities.rs"]
pub(in crate::game) mod lan_entities;
#[cfg(test)]
#[path = "network/lan_menus_tests.rs"]
pub(in crate::game) mod lan_menus_tests;
#[path = "network/lan_mobs.rs"]
pub(in crate::game) mod lan_mobs;
#[path = "network/lan_particles.rs"]
pub(in crate::game) mod lan_particles;
#[path = "network/lan_pvp.rs"]
pub(in crate::game) mod lan_pvp;
#[path = "network/lan_trade_decisions.rs"]
pub(in crate::game) mod lan_trade_decisions;
#[cfg(test)]
#[path = "network/lan_trade_decisions_tests.rs"]
pub(in crate::game) mod lan_trade_decisions_tests;
#[path = "network/lan_trading.rs"]
pub(in crate::game) mod lan_trading;
#[path = "network/lan_vehicles.rs"]
pub(in crate::game) mod lan_vehicles;
#[path = "network/lan_waypoints.rs"]
pub(in crate::game) mod lan_waypoints;
#[path = "entities/leads.rs"]
pub(in crate::game) mod leads;
#[path = "interface/level_particles.rs"]
pub(in crate::game) mod level_particles;
#[path = "interface/level_sounds.rs"]
pub(in crate::game) mod level_sounds;
#[path = "world/level_tick.rs"]
pub(in crate::game) mod level_tick;
#[path = "world/levels.rs"]
pub(in crate::game) mod levels;
#[path = "mobs/living_attributes.rs"]
pub(in crate::game) mod living_attributes;
#[path = "mobs/llamas.rs"]
pub(in crate::game) mod llamas;
#[path = "modding/load_conditions.rs"]
pub(in crate::game) mod load_conditions;
#[cfg(test)]
#[path = "administration/loot_kill_guest_tests.rs"]
pub(in crate::game) mod loot_kill_guest_tests;
#[path = "mobs/loot_rolls.rs"]
pub(in crate::game) mod loot_rolls;
#[path = "items/mace.rs"]
pub(in crate::game) mod mace;
#[path = "blocks/machines/mod.rs"]
pub(in crate::game) mod machines;
#[path = "blocks/maps/mod.rs"]
pub(in crate::game) mod maps;
#[path = "entities/melee.rs"]
pub(in crate::game) mod melee;
#[path = "runtime/memory.rs"]
pub(in crate::game) mod memory;
#[cfg(test)]
#[path = "items/mending_pick_tests.rs"]
pub(in crate::game) mod mending_pick_tests;
#[path = "entities/minecart_moves.rs"]
pub(in crate::game) mod minecart_moves;
#[cfg(test)]
#[path = "runtime/mining_component_tests.rs"]
pub(in crate::game) mod mining_component_tests;
#[path = "mobs/mob_gear.rs"]
pub(in crate::game) mod mob_gear;
#[path = "mobs/mob_level.rs"]
pub(in crate::game) mod mob_level;
#[path = "mobs/mob_looting.rs"]
pub(in crate::game) mod mob_looting;
#[path = "modding/mod_api_host.rs"]
pub(in crate::game) mod mod_api_host;
#[path = "modding/mod_container_openers.rs"]
pub(in crate::game) mod mod_container_openers;
#[cfg(test)]
#[path = "modding/mod_container_openers_tests.rs"]
pub(in crate::game) mod mod_container_openers_tests;
#[path = "side_ops/modapi_server.rs"]
pub(in crate::game) mod modapi_server;
#[path = "mobs/mounts.rs"]
pub(in crate::game) mod mounts;
#[path = "modding/native_ai_pause.rs"]
pub(in crate::game) mod native_ai_pause;
#[path = "modding/native_biome_keys.rs"]
pub(in crate::game) mod native_biome_keys;
#[cfg(test)]
#[path = "modding/native_biome_keys_tests.rs"]
pub(in crate::game) mod native_biome_keys_tests;
#[cfg(test)]
#[path = "modding/native_block_entity_item_tests.rs"]
pub(in crate::game) mod native_block_entity_item_tests;
#[path = "modding/native_block_entity_refs.rs"]
pub(in crate::game) mod native_block_entity_refs;
#[path = "modding/native_block_sounds.rs"]
pub(crate) mod native_block_sounds;
#[path = "modding/native_book_apply.rs"]
pub(in crate::game) mod native_book_apply;
#[path = "modding/native_book_backings.rs"]
pub(in crate::game) mod native_book_backings;
#[path = "modding/native_book_grid.rs"]
pub(in crate::game) mod native_book_grid;
#[path = "modding/native_book_invalidation.rs"]
pub(in crate::game) mod native_book_invalidation;
#[path = "modding/native_book_net.rs"]
pub(in crate::game) mod native_book_net;
#[path = "modding/native_book_scope.rs"]
pub(in crate::game) mod native_book_scope;
#[path = "modding/native_book_settings.rs"]
pub(in crate::game) mod native_book_settings;
#[path = "modding/native_boss_bar_refs.rs"]
pub(in crate::game) mod native_boss_bar_refs;
#[path = "modding/native_bots.rs"]
pub(in crate::game) mod native_bots;
#[cfg(test)]
#[path = "modding/native_chat_recipients_tests.rs"]
pub(in crate::game) mod native_chat_recipients_tests;
#[path = "modding/native_chunk_areas.rs"]
pub(in crate::game) mod native_chunk_areas;
#[path = "modding/native_chunk_save.rs"]
pub(in crate::game) mod native_chunk_save;
#[path = "modding/native_colliders.rs"]
pub(in crate::game) mod native_colliders;
#[cfg(test)]
#[path = "modding/native_colliders_tests.rs"]
pub(in crate::game) mod native_colliders_tests;
#[path = "modding/native_collision_exclusions.rs"]
pub(in crate::game) mod native_collision_exclusions;
#[path = "modding/native_config_files.rs"]
pub(in crate::game) mod native_config_files;
#[path = "modding/native_connection_groups.rs"]
pub(in crate::game) mod native_connection_groups;
#[path = "modding/native_console.rs"]
pub(in crate::game) mod native_console;
#[cfg(test)]
#[path = "modding/native_console_tests.rs"]
pub(in crate::game) mod native_console_tests;
#[path = "modding/native_criteria.rs"]
pub(in crate::game) mod native_criteria;
#[cfg(test)]
#[path = "modding/native_damage_tests.rs"]
pub(in crate::game) mod native_damage_tests;
#[path = "modding/native_data_maps.rs"]
pub(in crate::game) mod native_data_maps;
#[path = "modding/native_display_refs.rs"]
pub(in crate::game) mod native_display_refs;
#[path = "modding/native_dragon_refs.rs"]
pub(in crate::game) mod native_dragon_refs;
#[path = "modding/native_entities.rs"]
pub(in crate::game) mod native_entities;
#[path = "modding/native_entity_commands.rs"]
pub(in crate::game) mod native_entity_commands;
#[path = "modding/native_entity_data.rs"]
pub(in crate::game) mod native_entity_data;
#[cfg(test)]
#[path = "modding/native_entity_extinguish_tests.rs"]
pub(in crate::game) mod native_entity_extinguish_tests;
#[cfg(test)]
#[path = "modding/native_entity_index_parity_tests.rs"]
pub(in crate::game) mod native_entity_index_parity_tests;
#[path = "modding/native_entity_invulnerable.rs"]
pub(in crate::game) mod native_entity_invulnerable;
#[path = "modding/native_entity_kind.rs"]
pub(in crate::game) mod native_entity_kind;
#[path = "modding/native_entity_motion.rs"]
pub(in crate::game) mod native_entity_motion;
#[cfg(test)]
#[path = "modding/native_entity_move_correction_tests.rs"]
pub(in crate::game) mod native_entity_move_correction_tests;
#[cfg(test)]
#[path = "modding/native_entity_move_player_tests.rs"]
pub(in crate::game) mod native_entity_move_player_tests;
#[cfg(test)]
#[path = "modding/native_entity_move_sweep_tests.rs"]
pub(in crate::game) mod native_entity_move_sweep_tests;
#[path = "modding/native_entity_moves.rs"]
pub(in crate::game) mod native_entity_moves;
#[path = "modding/native_entity_physics.rs"]
pub(in crate::game) mod native_entity_physics;
#[path = "modding/native_entity_refs.rs"]
pub(in crate::game) mod native_entity_refs;
#[path = "modding/native_entity_remove.rs"]
pub(in crate::game) mod native_entity_remove;
#[path = "modding/native_entity_riding.rs"]
pub(in crate::game) mod native_entity_riding;
#[path = "modding/native_entity_selectors.rs"]
pub(in crate::game) mod native_entity_selectors;
#[path = "modding/native_entity_sounds.rs"]
pub(in crate::game) mod native_entity_sounds;
#[path = "modding/native_entity_state.rs"]
pub(in crate::game) mod native_entity_state;
#[path = "modding/native_explosion_destroy.rs"]
pub(in crate::game) mod native_explosion_destroy;
#[path = "modding/native_explosion_resistance.rs"]
pub(in crate::game) mod native_explosion_resistance;
#[path = "modding/native_explosions.rs"]
pub(in crate::game) mod native_explosions;
#[cfg(test)]
#[path = "modding/native_explosions_tests.rs"]
pub(in crate::game) mod native_explosions_tests;
#[cfg(test)]
#[path = "modding/native_fake_disconnect_tests.rs"]
pub(in crate::game) mod native_fake_disconnect_tests;
#[path = "modding/native_falling.rs"]
pub(in crate::game) mod native_falling;
#[path = "modding/native_fishing_hook_refs.rs"]
pub(in crate::game) mod native_fishing_hook_refs;
#[path = "modding/native_game_event_context.rs"]
pub(in crate::game) mod native_game_event_context;
#[cfg(test)]
#[path = "modding/native_game_event_keys_tests.rs"]
pub(in crate::game) mod native_game_event_keys_tests;
#[path = "modding/native_goal_ticks/mod.rs"]
pub(in crate::game) mod native_goal_ticks;
#[cfg(test)]
#[path = "modding/native_guest_turn_tests.rs"]
pub(in crate::game) mod native_guest_turn_tests;
#[path = "modding/native_hanging.rs"]
pub(in crate::game) mod native_hanging;
#[cfg(test)]
#[path = "modding/native_hanging_tests.rs"]
pub(in crate::game) mod native_hanging_tests;
#[path = "modding/native_harvest_queries.rs"]
pub(in crate::game) mod native_harvest_queries;
#[cfg(test)]
#[path = "modding/native_hit_attempt_tests.rs"]
pub(in crate::game) mod native_hit_attempt_tests;
#[cfg(test)]
#[path = "modding/native_hit_causer_tests.rs"]
pub(in crate::game) mod native_hit_causer_tests;
#[cfg(test)]
#[path = "modding/native_hit_sounds_tests.rs"]
pub(in crate::game) mod native_hit_sounds_tests;
#[cfg(test)]
#[path = "modding/native_hit_view_tests.rs"]
pub(in crate::game) mod native_hit_view_tests;
#[path = "modding/native_inventories.rs"]
pub(in crate::game) mod native_inventories;
#[cfg(test)]
#[path = "modding/native_invulnerability_check_tests.rs"]
pub(in crate::game) mod native_invulnerability_check_tests;
#[path = "modding/native_item_places.rs"]
pub(in crate::game) mod native_item_places;
#[path = "modding/native_knockback_credit.rs"]
pub(in crate::game) mod native_knockback_credit;
#[cfg(test)]
#[path = "modding/native_knockback_tests.rs"]
pub(in crate::game) mod native_knockback_tests;
#[path = "modding/native_level_border.rs"]
pub(in crate::game) mod native_level_border;
#[path = "modding/native_level_effects.rs"]
pub(in crate::game) mod native_level_effects;
#[path = "modding/native_level_events.rs"]
pub(in crate::game) mod native_level_events;
#[path = "modding/native_level_items.rs"]
pub(in crate::game) mod native_level_items;
#[cfg(test)]
#[path = "modding/native_level_placement_tests.rs"]
pub(in crate::game) mod native_level_placement_tests;
#[path = "modding/native_level_players.rs"]
pub(in crate::game) mod native_level_players;
#[path = "modding/native_level_refs.rs"]
pub(in crate::game) mod native_level_refs;
#[path = "modding/native_level_signals.rs"]
pub(in crate::game) mod native_level_signals;
#[cfg(test)]
#[path = "modding/native_level_sound_except_tests.rs"]
pub(in crate::game) mod native_level_sound_except_tests;
#[path = "modding/native_level_spawning.rs"]
pub(in crate::game) mod native_level_spawning;
#[path = "modding/native_level_ticking.rs"]
pub(in crate::game) mod native_level_ticking;
#[path = "modding/native_level_weather.rs"]
pub(in crate::game) mod native_level_weather;
#[path = "modding/native_level_world.rs"]
pub(in crate::game) mod native_level_world;
#[path = "modding/native_lifecycle.rs"]
pub(in crate::game) mod native_lifecycle;
#[path = "modding/native_living_body.rs"]
pub(in crate::game) mod native_living_body;
#[path = "modding/native_living_effects.rs"]
pub(in crate::game) mod native_living_effects;
#[path = "modding/native_living_flags.rs"]
pub(in crate::game) mod native_living_flags;
#[path = "modding/native_living_position.rs"]
pub(in crate::game) mod native_living_position;
#[path = "modding/native_living_refs.rs"]
pub(in crate::game) mod native_living_refs;
#[path = "modding/native_living_writes.rs"]
pub(in crate::game) mod native_living_writes;
#[cfg(test)]
#[path = "modding/native_menu_inventory_tests.rs"]
pub(in crate::game) mod native_menu_inventory_tests;
#[path = "modding/native_menu_places.rs"]
pub(in crate::game) mod native_menu_places;
#[path = "modding/native_menus.rs"]
pub(in crate::game) mod native_menus;
#[path = "modding/native_merchant_refs.rs"]
pub(in crate::game) mod native_merchant_refs;
#[path = "modding/native_minecart_refs.rs"]
pub(in crate::game) mod native_minecart_refs;
#[path = "modding/native_mob_goals.rs"]
pub(in crate::game) mod native_mob_goals;
#[path = "modding/native_mob_navigation.rs"]
pub(in crate::game) mod native_mob_navigation;
#[path = "modding/native_mob_refs.rs"]
pub(in crate::game) mod native_mob_refs;
#[path = "modding/native_mob_wanted_position.rs"]
pub(in crate::game) mod native_mob_wanted_position;
#[cfg(test)]
#[path = "modding/native_mob_wanted_position_tests.rs"]
pub(in crate::game) mod native_mob_wanted_position_tests;
#[path = "modding/native_mounts.rs"]
pub(in crate::game) mod native_mounts;
#[cfg(test)]
#[path = "modding/native_move_by_tests.rs"]
pub(in crate::game) mod native_move_by_tests;
#[path = "modding/native_passages.rs"]
pub(in crate::game) mod native_passages;
#[cfg(test)]
#[path = "modding/native_path_types_tests.rs"]
pub(in crate::game) mod native_path_types_tests;
#[path = "modding/native_pending_children.rs"]
pub(in crate::game) mod native_pending_children;
#[path = "modding/native_place_feature.rs"]
pub(in crate::game) mod native_place_feature;
#[cfg(test)]
#[path = "modding/native_placement_state_tests.rs"]
pub(in crate::game) mod native_placement_state_tests;
#[path = "modding/native_player_break.rs"]
pub(in crate::game) mod native_player_break;
#[path = "modding/native_player_cooldowns.rs"]
pub(in crate::game) mod native_player_cooldowns;
#[cfg(test)]
#[path = "modding/native_player_destroyed_block_tests.rs"]
pub(in crate::game) mod native_player_destroyed_block_tests;
#[path = "modding/native_player_disconnect.rs"]
pub(in crate::game) mod native_player_disconnect;
#[path = "modding/native_player_ender_chest.rs"]
pub(in crate::game) mod native_player_ender_chest;
#[cfg(test)]
#[path = "modding/native_player_ender_chest_tests.rs"]
pub(in crate::game) mod native_player_ender_chest_tests;
#[cfg(test)]
#[path = "modding/native_player_level_tests.rs"]
pub(in crate::game) mod native_player_level_tests;
#[path = "modding/native_player_modes.rs"]
pub(in crate::game) mod native_player_modes;
#[cfg(test)]
#[path = "modding/native_player_move_tests.rs"]
pub(in crate::game) mod native_player_move_tests;
#[path = "modding/native_player_respawn.rs"]
pub(in crate::game) mod native_player_respawn;
#[cfg(test)]
#[path = "modding/native_player_spectating_tests.rs"]
pub(in crate::game) mod native_player_spectating_tests;
#[path = "modding/native_player_stats.rs"]
pub(in crate::game) mod native_player_stats;
#[path = "modding/native_player_visibility.rs"]
pub(in crate::game) mod native_player_visibility;
#[cfg(test)]
#[path = "modding/native_player_visibility_tests.rs"]
pub(in crate::game) mod native_player_visibility_tests;
#[path = "modding/native_projectile_launch.rs"]
pub(in crate::game) mod native_projectile_launch;
#[cfg(test)]
#[path = "modding/native_projectile_launch_tests.rs"]
pub(in crate::game) mod native_projectile_launch_tests;
#[path = "modding/native_projectile_refs.rs"]
pub(in crate::game) mod native_projectile_refs;
#[path = "modding/native_projectiles.rs"]
pub(in crate::game) mod native_projectiles;
#[path = "modding/native_quick_move.rs"]
pub(in crate::game) mod native_quick_move;
#[path = "modding/native_raid_refs.rs"]
pub(in crate::game) mod native_raid_refs;
#[path = "modding/native_recipe_properties.rs"]
pub(in crate::game) mod native_recipe_properties;
#[cfg(test)]
#[path = "modding/native_recipe_sync_tests.rs"]
pub(in crate::game) mod native_recipe_sync_tests;
#[path = "modding/native_registry_entries.rs"]
pub(in crate::game) mod native_registry_entries;
#[cfg(test)]
#[path = "modding/native_registry_ids_tests.rs"]
pub(in crate::game) mod native_registry_ids_tests;
#[path = "modding/native_resources.rs"]
pub(in crate::game) mod native_resources;
#[path = "modding/native_ride_player.rs"]
pub(in crate::game) mod native_ride_player;
#[path = "modding/native_scoreboard.rs"]
pub(in crate::game) mod native_scoreboard;
#[path = "modding/native_server_distances.rs"]
pub(in crate::game) mod native_server_distances;
#[cfg(test)]
#[path = "modding/native_shield_totem_tests.rs"]
pub(in crate::game) mod native_shield_totem_tests;
#[path = "modding/native_singleplayer_owner.rs"]
pub(in crate::game) mod native_singleplayer_owner;
#[cfg(test)]
#[path = "modding/native_singleplayer_owner_tests.rs"]
pub(in crate::game) mod native_singleplayer_owner_tests;
#[path = "modding/native_sound_refs.rs"]
pub(in crate::game) mod native_sound_refs;
#[path = "modding/native_structure_placement.rs"]
pub(in crate::game) mod native_structure_placement;
#[path = "modding/native_structures.rs"]
pub(in crate::game) mod native_structures;
#[cfg(test)]
#[path = "modding/native_success_broadcast_tests.rs"]
pub(in crate::game) mod native_success_broadcast_tests;
#[path = "modding/native_surface.rs"]
pub(crate) mod native_surface;
#[path = "modding/native_tick_time.rs"]
pub(in crate::game) mod native_tick_time;
#[path = "modding/native_ticks.rs"]
pub(in crate::game) mod native_ticks;
#[path = "modding/native_unspawned_mob.rs"]
pub(in crate::game) mod native_unspawned_mob;
#[path = "modding/native_world_edit.rs"]
pub(in crate::game) mod native_world_edit;
#[path = "modding/native_world_read.rs"]
pub(in crate::game) mod native_world_read;
#[path = "mobs/natural_spawning.rs"]
pub(in crate::game) mod natural_spawning;
#[path = "modding/neoforge_block_maps.rs"]
pub(in crate::game) mod neoforge_block_maps;
#[path = "modding/neoforge_mob_maps.rs"]
pub(in crate::game) mod neoforge_mob_maps;
#[path = "blocks/nether_portal.rs"]
pub(in crate::game) mod nether_portal;
#[path = "mobs/ominous_item_spawner/mod.rs"]
pub(in crate::game) mod ominous_item_spawner;
#[cfg(test)]
#[path = "entities/orb_data_tests.rs"]
pub(in crate::game) mod orb_data_tests;
#[path = "world/ordered_tick.rs"]
pub(in crate::game) mod ordered_tick;
#[path = "modding/original_player.rs"]
pub(in crate::game) mod original_player;
#[path = "world/owner_link.rs"]
pub(in crate::game) mod owner_link;
#[path = "modding/pack_finders/mod.rs"]
pub(in crate::game) mod pack_finders;
#[path = "entities/paintings.rs"]
pub(in crate::game) mod paintings;
#[path = "mobs/pandas.rs"]
pub(in crate::game) mod pandas;
#[path = "mobs/part_shearing.rs"]
pub(in crate::game) mod part_shearing;
#[path = "entities/passengers.rs"]
pub(in crate::game) mod passengers;
#[path = "mobs/pending_births.rs"]
pub(in crate::game) mod pending_births;
#[path = "interface/perf_remote.rs"]
pub(in crate::game) mod perf_remote;
#[path = "world/persist.rs"]
pub(in crate::game) mod persist;
#[cfg(test)]
#[path = "mobs/piglin_admire_tests.rs"]
pub(in crate::game) mod piglin_admire_tests;
#[cfg(test)]
#[path = "mobs/piglin_conversion_tests.rs"]
pub(in crate::game) mod piglin_conversion_tests;
#[path = "mobs/piglins.rs"]
pub(in crate::game) mod piglins;
#[path = "blocks/piston_carry_state.rs"]
pub(in crate::game) mod piston_carry_state;
#[cfg(test)]
#[path = "blocks/place_feature_tests.rs"]
pub(in crate::game) mod place_feature_tests;
#[cfg(test)]
#[path = "interface/play_ping_tests.rs"]
pub(in crate::game) mod play_ping_tests;
#[cfg(test)]
#[path = "blocks/player_destroy_tests.rs"]
pub(in crate::game) mod player_destroy_tests;
#[path = "entities/player_equip.rs"]
pub(in crate::game) mod player_equip;
#[path = "entities/player_residency.rs"]
pub(in crate::game) mod player_residency;
#[cfg(test)]
#[path = "entities/player_residency_tests.rs"]
pub(in crate::game) mod player_residency_tests;
#[cfg(test)]
#[path = "world/player_save_projection_tests.rs"]
pub(in crate::game) mod player_save_projection_tests;
#[path = "player_server.rs"]
pub(in crate::game) mod player_server;
#[path = "world/player_spawn.rs"]
pub(in crate::game) mod player_spawn;
#[path = "blocks/potent_sulfur.rs"]
pub(in crate::game) mod potent_sulfur;
#[path = "items/primary_items.rs"]
pub(in crate::game) mod primary_items;
#[path = "mobs/projectile_block_impacts.rs"]
pub(in crate::game) mod projectile_block_impacts;
#[path = "items/projectile_spawned.rs"]
pub(in crate::game) mod projectile_spawned;
#[cfg(test)]
#[path = "items/projectile_spawned_tests.rs"]
pub(in crate::game) mod projectile_spawned_tests;
#[path = "mobs/raids/mod.rs"]
pub(in crate::game) mod raids;
#[cfg(test)]
#[path = "entities/rail_parts_tests.rs"]
pub(in crate::game) mod rail_parts_tests;
#[cfg(test)]
#[path = "interface/recipe_book_variants_tests.rs"]
pub(in crate::game) mod recipe_book_variants_tests;
#[cfg(test)]
#[path = "modding/rectangular_recipe_remainder_tests.rs"]
pub(in crate::game) mod rectangular_recipe_remainder_tests;
#[path = "blocks/respawn_anchor.rs"]
pub(in crate::game) mod respawn_anchor;
#[path = "world/runtime_levels.rs"]
pub(in crate::game) mod runtime_levels;
#[cfg(test)]
#[path = "world/runtime_levels_tests.rs"]
pub(in crate::game) mod runtime_levels_tests;
#[cfg(test)]
#[path = "world/save_load_pass_tests.rs"]
pub(in crate::game) mod save_load_pass_tests;
#[cfg(test)]
#[path = "world/save_pass_tests.rs"]
pub(in crate::game) mod save_pass_tests;
#[path = "interface/scoreboard/mod.rs"]
pub(in crate::game) mod scoreboard;
#[path = "blocks/sculk_fixture.rs"]
pub(in crate::game) mod sculk_fixture;
#[path = "blocks/sculk_server.rs"]
pub(in crate::game) mod sculk_server;
#[path = "entities/seats.rs"]
pub(in crate::game) mod seats;
#[path = "server_side/server_effects.rs"]
pub(in crate::game) mod server_effects;
#[path = "world/server_lifecycle/mod.rs"]
pub(in crate::game) mod server_lifecycle;
#[path = "world/server_reload/mod.rs"]
pub(in crate::game) mod server_reload;
#[path = "world/server_travel.rs"]
pub(in crate::game) mod server_travel;
#[cfg(test)]
#[path = "mobs/shear_refind_tests.rs"]
pub(in crate::game) mod shear_refind_tests;
#[path = "items/shield.rs"]
pub(in crate::game) mod shield;
#[path = "mobs/shoulders.rs"]
pub(in crate::game) mod shoulders;
#[path = "modding/slot_changes.rs"]
pub(in crate::game) mod slot_changes;
#[cfg(test)]
#[path = "modding/slot_take_tests.rs"]
pub(in crate::game) mod slot_take_tests;
#[path = "modding/slot_takes.rs"]
pub(in crate::game) mod slot_takes;
#[cfg(test)]
#[path = "world/spawn_save_tests.rs"]
pub(in crate::game) mod spawn_save_tests;
#[path = "mobs/spawner.rs"]
pub(in crate::game) mod spawner;
#[path = "items/spear.rs"]
pub(in crate::game) mod spear;
#[path = "items/spear_players.rs"]
pub(in crate::game) mod spear_players;
#[path = "items/special_attack_fixture.rs"]
pub(in crate::game) mod special_attack_fixture;
#[path = "items/special_attacks.rs"]
pub(in crate::game) mod special_attacks;
#[path = "blocks/spill.rs"]
pub(in crate::game) mod spill;
#[path = "blocks/splash_water.rs"]
pub(in crate::game) mod splash_water;
#[cfg(test)]
#[path = "entities/stand_break_loot_tests.rs"]
pub(in crate::game) mod stand_break_loot_tests;
#[cfg(test)]
#[path = "entities/stand_command_hurt_tests.rs"]
pub(in crate::game) mod stand_command_hurt_tests;
#[cfg(feature = "state-digest")]
#[path = "world/state_digest.rs"]
pub(in crate::game) mod state_digest;
#[path = "interface/stats.rs"]
pub(in crate::game) mod stats;
#[cfg(test)]
#[path = "runtime/storage_alias_tests.rs"]
pub(in crate::game) mod storage_alias_tests;
#[path = "administration/structure_blocks.rs"]
pub(in crate::game) mod structure_blocks;
#[path = "mobs/sulfur_cubes.rs"]
pub(in crate::game) mod sulfur_cubes;
#[path = "administration/test_entities.rs"]
pub(in crate::game) mod test_entities;
#[path = "administration/test_environments.rs"]
pub(in crate::game) mod test_environments;
#[path = "administration/test_instances.rs"]
pub(in crate::game) mod test_instances;
#[cfg(test)]
#[path = "runtime/test_pos_guest_tests.rs"]
pub(in crate::game) mod test_pos_guest_tests;
#[path = "entities/tnt.rs"]
pub(in crate::game) mod tnt;
#[path = "items/trading.rs"]
pub(in crate::game) mod trading;
#[cfg(test)]
#[path = "runtime/transaction_entities_tests.rs"]
pub(in crate::game) mod transaction_entities_tests;
#[path = "runtime/transactions_server.rs"]
pub(in crate::game) mod transactions_server;
#[path = "mobs/trial_spawner.rs"]
pub(in crate::game) mod trial_spawner;
#[path = "items/trident.rs"]
pub(in crate::game) mod trident;
#[path = "items/use_hand.rs"]
pub(in crate::game) mod use_hand;
#[cfg(test)]
#[path = "items/use_stop_tests.rs"]
pub(in crate::game) mod use_stop_tests;
#[path = "items/using_item.rs"]
pub(in crate::game) mod using_item;
#[path = "blocks/vault.rs"]
pub(in crate::game) mod vault;
#[cfg(test)]
#[path = "entities/vehicle_removal_tests.rs"]
pub(in crate::game) mod vehicle_removal_tests;
#[path = "entities/vehicle_teleport.rs"]
pub(in crate::game) mod vehicle_teleport;
#[cfg(test)]
#[path = "blocks/vibration_source_tests.rs"]
pub(in crate::game) mod vibration_source_tests;
#[cfg(test)]
#[path = "blocks/vibration_tests.rs"]
pub(in crate::game) mod vibration_tests;
#[path = "world/view_pass.rs"]
pub(in crate::game) mod view_pass;
#[path = "world/villager_professions.rs"]
pub(in crate::game) mod villager_professions;
#[path = "mobs/villagers.rs"]
pub(in crate::game) mod villagers;
#[cfg(test)]
#[path = "world/visibility_unwind_tests.rs"]
pub(in crate::game) mod visibility_unwind_tests;
#[path = "world/wake_time.rs"]
pub(in crate::game) mod wake_time;
#[path = "mobs/wandering_trader.rs"]
pub(in crate::game) mod wandering_trader;
#[path = "interface/wind.rs"]
pub(in crate::game) mod wind;
#[path = "mobs/wither.rs"]
pub(in crate::game) mod wither;
#[cfg(test)]
#[path = "mobs/wither_star_tests.rs"]
pub(in crate::game) mod wither_star_tests;
#[path = "world/world_entry.rs"]
pub(in crate::game) mod world_entry;
#[path = "blocks/world_fluids.rs"]
pub(in crate::game) mod world_fluids;
#[path = "world/world_loading.rs"]
pub(in crate::game) mod world_loading;
#[cfg(test)]
#[path = "blocks/world_test_def_tests.rs"]
pub(in crate::game) mod world_test_def_tests;
#[cfg(test)]
#[path = "items/xp_repair_ratio_tests.rs"]
pub(in crate::game) mod xp_repair_ratio_tests;
#[path = "mobs/zombie_reinforcements.rs"]
pub(in crate::game) mod zombie_reinforcements;

#[path = "runtime/agent/mod_server.rs"]
pub(in crate::game) mod agent_server;
#[path = "world/autosave_server.rs"]
pub(in crate::game) mod autosave_server;
#[path = "blocks/block_breaking_server.rs"]
pub(in crate::game) mod block_breaking_server;
#[cfg(test)]
#[path = "interface/block_screen_effect_draw_tests_server.rs"]
pub(in crate::game) mod block_screen_effect_draw_tests_server;
#[path = "interface/books_server.rs"]
pub(in crate::game) mod books_server;
#[path = "mobs/breezes_server.rs"]
pub(in crate::game) mod breezes_server;
#[path = "interface/chat_server.rs"]
pub(in crate::game) mod chat_server;
#[path = "administration/command_blocks_server.rs"]
pub(in crate::game) mod command_blocks_server;
#[path = "administration/commands/mod_server.rs"]
pub(in crate::game) mod commands_server;
#[cfg(test)]
#[path = "runtime/core_tests_server.rs"]
pub(in crate::game) mod core_tests_server;
#[cfg(test)]
#[path = "modding/crafting_recipe_kind_tests_server.rs"]
pub(in crate::game) mod crafting_recipe_kind_tests_server;
#[path = "mobs/creatures/mod_server.rs"]
pub(in crate::game) mod creatures_server;
#[path = "interface/debug_lines_server.rs"]
pub(in crate::game) mod debug_lines_server;
#[path = "entities/fireworks_server.rs"]
pub(in crate::game) mod fireworks_server;
#[path = "game_server.rs"]
pub(in crate::game) mod game_server;
#[cfg(test)]
#[path = "items/hand_part_tests_server.rs"]
pub(in crate::game) mod hand_part_tests_server;
#[path = "items/hand_server.rs"]
pub(in crate::game) mod hand_server;
#[path = "blocks/jukebox_server.rs"]
pub(in crate::game) mod jukebox_server;
#[path = "network/lan_actions/mod_server.rs"]
pub(in crate::game) mod lan_actions_server;
#[path = "entities/lightning_server.rs"]
pub(in crate::game) mod lightning_server;
#[path = "interface/menu_clicks_server.rs"]
pub(in crate::game) mod menu_clicks_server;
#[path = "entities/minecarts_server.rs"]
pub(in crate::game) mod minecarts_server;
#[path = "modding/mods/mod_server.rs"]
pub(in crate::game) mod mods_server;
#[path = "interface/names_server.rs"]
pub(in crate::game) mod names_server;
#[path = "modding/native_components_server.rs"]
pub(in crate::game) mod native_components_server;
#[path = "native_hooks/mod_server.rs"]
pub(in crate::game) mod native_hooks_server;
#[path = "modding/native_menu_refs_server.rs"]
pub(in crate::game) mod native_menu_refs_server;
#[path = "modding/native_server_server.rs"]
pub(in crate::game) mod native_server_server;
#[path = "mobs/nautili_server.rs"]
pub(in crate::game) mod nautili_server;
#[path = "world/packs/mod_server.rs"]
pub(in crate::game) mod packs_server;
#[path = "interface/pad/mod_server.rs"]
pub(in crate::game) mod pad_server;
#[path = "entities/pick_entity_server.rs"]
pub(in crate::game) mod pick_entity_server;
#[path = "blocks/piston_push_server.rs"]
pub(in crate::game) mod piston_push_server;
#[path = "interface/play_keys_server.rs"]
pub(in crate::game) mod play_keys_server;
#[path = "items/potions_server.rs"]
pub(in crate::game) mod potions_server;
#[path = "network/remote/mod_server.rs"]
pub(in crate::game) mod remote_server;
#[path = "network/remote_tags_server.rs"]
pub(in crate::game) mod remote_tags_server;
#[path = "runtime/repro_server.rs"]
pub(in crate::game) mod repro_server;
#[path = "world/saving_server.rs"]
pub(in crate::game) mod saving_server;
#[path = "interface/shader_packs_server.rs"]
pub(in crate::game) mod shader_packs_server;
#[path = "interface/sign_edit_server.rs"]
pub(in crate::game) mod sign_edit_server;
#[path = "mobs/spawner_server.rs"]
pub(in crate::game) mod spawner_server;
#[cfg(test)]
#[path = "modding/stacked_on_other_tests_server.rs"]
pub(in crate::game) mod stacked_on_other_tests_server;
#[path = "blocks/stations_server.rs"]
pub(in crate::game) mod stations_server;
#[path = "world/storage_server.rs"]
pub(in crate::game) mod storage_server;
#[path = "entities/text_displays_server.rs"]
pub(in crate::game) mod text_displays_server;
#[path = "interface/titles_server.rs"]
pub(in crate::game) mod titles_server;
#[path = "items/totem_server.rs"]
pub(in crate::game) mod totem_server;
#[path = "world/world_update_server.rs"]
pub(in crate::game) mod world_update_server;

pub(in crate::game) struct ServerState {
    /// The registries native mods registered data map types for, as of the
    /// server tables' last build: what `data_maps_updated` fires for.
    pub(in crate::game::server_side) data_map_registries: Vec<String>,
    /// The crafting table the 3x3 grid belongs to (`CraftingMenu.access`).
    pub(in crate::game::server_side) craft_table: Option<[i32; 3]>,
    /// The open container and the slots it had when it opened:
    /// `Container.stillValid` holds only while every block entity the menu
    /// took is still there, so a half of a double chest (or the container)
    /// going away closes it.
    pub(in crate::game::server_side) open_container_slots: Option<((i32, i32, i32), usize)>,
    pub(in crate::game::server_side) destroy_delay: u32,
    /// The prepared world is installed. The session lock is taken earlier so tag
    /// reads see ready tables, and is not this marker: a final save before the
    /// prepared `Level` is installed would replace the world with placeholder state.
    pub(in crate::game::server_side) world_adopted: bool,
    /// An early shutdown must not replace on-disk tickets while the server is
    /// still publishing levels or activating their tickets.
    pub(in crate::game::server_side) startup_ticket_restore_pending: bool,
    pub(in crate::game) next_entity: u32,
    /// The last entity id when this level's entity pass began: an item with a
    /// later one was added during that pass and first ticks on the next
    /// (`EntityTickList`). `u32::MAX` until a tick has run.
    pub(in crate::game) ticked_entities_through: u32,
    /// The items `tick_blocks` dropped while applying the entities' block
    /// contacts (a boat breaking a lily pad). Vanilla runs those in each
    /// entity's own tick (`applyEffectsFromBlocks`), inside the entity pass,
    /// so they too first tick on the next tick.
    pub(in crate::game::server_side) contact_drops: std::ops::Range<u32>,
    /// Lecterns whose page-turn pulse is running (ticks left).
    pub(in crate::game::server_side) lectern_pulses: Vec<((i32, i32, i32), i32)>,
    /// The world spawn search's origin chunk and fallback height, once known.
    pub(in crate::game::server_side) spawn_origin: Option<((i32, i32), i32)>,
    /// `native_create_spawn_position` was asked, and a mod chose the spawn.
    pub(in crate::game::server_side) spawn_hook_fired: bool,
    pub(in crate::game::server_side) spawn_hook_cancelled: bool,
    pub(in crate::game::server_side) spawn_spiral: sunrise_client::world_spawn::ChunkSpiral,
    /// A new player's spawn search, while its candidates' chunks load.
    pub(in crate::game::server_side) player_spawn:
        Option<sunrise_client::world_spawn::PlayerSpawnFinder>,
    /// Chunks a respawn search waits for this tick, kept loading beside the
    /// player's own (taken by `update_world`).
    pub(in crate::game::server_side) spawn_wait: Vec<(i32, i32)>,
    pub(in crate::game::server_side) attack_ticks: u32,
    /// The main hand's item at the last player tick, whose attack damage and
    /// speed modifiers apply (`LivingEntity.detectEquipmentUpdates`): a hotbar
    /// swap just before an attack keeps them, with the new item's enchantments
    /// (attribute swapping).
    pub(in crate::game::server_side) attack_item: Option<Arc<str>>,
    /// Mobs a player hurt (`LivingEntity.lastHurtByPlayer`), by player UUID.
    pub(in crate::game) last_hurt_by_player: HashMap<u32, combat::PlayerCredit>,
    /// Leashed mobs a player killed, with the killer's UUID: the lead drops
    /// on the corpse's next tick (`Leashable.tickLeash`), for the mods. With
    /// the corpse's dimension: each level's tick prunes only its own.
    pub(in crate::game::server_side) lead_killers: HashMap<u32, (u128, Dim)>,
    /// Animals a player fed into love (`Animal.loveCause`), with the guest it was by
    /// connection (None: the host). Breeding credits them, wherever they are.
    pub(in crate::game::server_side) love_cause: HashMap<u32, Option<u32>>,
    /// Mobs whose death (`LivingEntity.die`) has been handled.
    /// Looked up per mob each tick: a fast hasher (never iterated).
    pub(in crate::game::server_side) died: sunrise_client::fast_hash::FxHashSet<u32>,
    /// The crossbow charge's start and middle loading sounds played (`game/items/crossbow.rs`):
    /// each charge plays each once, however far a mod's tick moved the charge at a time.
    pub(in crate::game::server_side) crossbow_sounds: [bool; 2],
    /// The shield's item cooldown after an axe disabled it (ticks).
    pub(in crate::game::server_side) shield_cooldown: u32,
    /// The source of a command run through /execute (the player otherwise).
    pub(in crate::game::server_side) command_source: Option<commands::execute::Source>,
    pub(in crate::game::server_side) native_command_filter: bool,
    /// `MinecraftServer.ticksUntilAutosave`: the ticks left before the next
    /// autosave, counted down a tick at a time.
    pub(in crate::game::server_side) ticks_until_autosave: i64,
    pub(in crate::game::server_side) applied_modifiers: Vec<(String, String)>,
    pub(in crate::game::server_side) xp_delay: u32,
    pub(in crate::game::server_side) ordered: ordered_tick::OrderedTicks,
    pub(in crate::game::server_side) level_up_sound: sunrise_client::entities::LevelUpSound,
    /// Swings of uses whose client predicts them (`SwingSource.PREDICTED`):
    /// a server does not send these back to the player who swung.
    pub(in crate::game::server_side) predicted_swings: u32,
    /// The beds players sleep in that this game marked `occupied`, by level
    /// (game/blocks/beds.rs).
    pub(in crate::game::server_side) player_beds: Vec<(Dim, [i32; 3])>,
    /// `SleepStatus`'s active and sleeping players at the last announcement.
    pub(in crate::game::server_side) sleep_seen: (i64, i64),
    pub(in crate::game) world_root: PathBuf,
    pub(in crate::game::server_side) carrier_retention_snapshot: Vec<(u128, u32)>,
    /// Offline-to-account UUID migration: loaded entities retain the old UUID until saved.
    pub(in crate::game::server_side) owner_migration: Option<(u128, u128)>,
    /// The world is the Debug preset (`Level.isDebug`, for every dimension): its blocks
    /// cannot be changed and nothing ticks.
    pub(in crate::game::server_side) world_debug: bool,
    /// A server's players on their way to another level, each waiting for
    /// the destination to load around them.
    pub(in crate::game::server_side) server_trips: Vec<server_travel::Trip>,
    /// Players a command sends to another level, once it has run.
    pub(in crate::game::server_side) level_teleports: Vec<server_travel::LevelTeleport>,
    /// Teleports a command made inside a level, for `native_player_teleported`
    /// once the player's move has reached their client (the put-back at the end of `guest_turn`).
    pub(in crate::game) teleport_notes: Vec<server_travel::TeleportNote>,
    /// Which pass of a server tick runs (`tick_server`): the first one reads
    /// the network, the last one ends the players' tick.
    pub(in crate::game::server_side) level_pass: levels::Pass,
    /// The respawn point when it is not the world spawn or a bed: a respawn
    /// anchor (`game/blocks/respawn_anchor.rs`).
    pub(in crate::game::server_side) respawn_config: Option<respawn_anchor::RespawnConfig>,
    /// The next enter_dimension call comes from an End portal.
    pub(in crate::game::server_side) end_portal_travel: bool,
    pub(in crate::game::server_side) tick_scratch: TickScratch,
    pub(in crate::game::server_side) tick_chunks:
        sunrise_client::tick_chunk_membership::TickChunkMembership,
    /// Worn armor as of the last equipment check (frost walker).
    pub(in crate::game::server_side) location_armor: [Option<ItemStack>; 4],
    /// The boats a mob's lead can hang on, for this tick's mob ticks.
    pub(in crate::game::server_side) leash_boats:
        Vec<(u32, sunrise_client::mobs::leash::HolderState)>,
    /// The boats and minecarts `native_vehicle_moved` follows (lan_vehicles.rs).
    pub(in crate::game::server_side) watched_vehicles: Vec<lan_vehicles::WatchedVehicle>,
    /// The ender dragon fight (game/mobs/dragon.rs).
    pub(in crate::game::server_side) fight: dragon::Fight,
    /// The entity ids a hosted server gives the knots its mobs hang on, by
    /// block, while a lead is tied there.
    pub(in crate::game) knot_ids: HashMap<[i32; 3], u32>,
    /// Fence posts whose natural knot was killed and whose leads drop next tick.
    pub(in crate::game) broken_knots: Vec<[i32; 3]>,
    /// This window's player's place in `riding`'s passengers, as `Guest.vehicle_order` is a guest's.
    pub(in crate::game::server_side) riding_order: i64,
    /// Vehicles that gained a rider this tick whose `START_RIDING_TRIGGER` was
    /// not fired yet (`game/entities/boarding.rs`), for a guest's turn.
    pub(in crate::game::server_side) started_riding: Vec<u32>,
    /// A rotation-only write (`/rotate`, `Entity.forceSetRotation`) to the player of this
    /// turn: the put-back at the end of `guest_turn` sends it as `ClientboundPlayerRotationPacket`, not a teleport.
    pub(in crate::game::server_side) rotation_forced: bool,
    /// The vehicle the player's file names, waiting for it to be in the world
    /// (`ServerPlayer.loadAndSpawnParentVehicle`).
    pub(in crate::game::server_side) pending_vehicle: Option<[i32; 4]>,
    /// The ender inventory (27 slots) and the ender chest showing it.
    pub(in crate::game::server_side) ender_items: Vec<Option<ItemStack>>,
    pub(in crate::game::server_side) ender_open: Option<(i32, i32, i32)>,
    pub(in crate::game::server_side) creaking: creaking::CreakingState,
    /// Reused candidate buffers for natural spawning.
    pub(in crate::game::server_side) spawn_scratch: natural_spawning::SpawnScratch,
    pub(in crate::game::server_side) hoppers: HashMap<(i32, i32, i32), machines::HopperState>,
    /// The source of the running dispenser behaviour, including across mod callbacks.
    pub(in crate::game::server_side) dispense_source: Option<dispenser::DispenseSource>,
    /// Open mod menus whose `menu_slot_changed` dispatch is currently on the call stack.
    pub(in crate::game::server_side) slot_changed_active: Vec<slot_changes::SlotChangedTarget>,
    /// Backing blocks whose contents notice is on the call stack, innermost last.
    pub(in crate::game) contents_changing: Vec<(mod_api::LevelRef, BlockPos)>,
    /// The players with each mod container open, by level (`tick_mod_container_viewers`).
    pub(in crate::game::server_side) mod_container_viewers:
        HashMap<(&'static str, (i32, i32, i32)), mod_container_openers::ModContainerOpeners>,
    /// Where item entities are, while the block entities tick (hoppers).
    pub(in crate::game) item_columns: Option<machines::ItemColumns>,
    /// The item columns for one pass of the players' pickups
    /// (`tick_guests`), rebuilt after any write to the items (a pickup, a
    /// command, an explosion): see `item_columns`.
    pub(in crate::game::server_side) pickup_columns: Option<machines::StampedItemColumns>,
    /// Where the mobs are, while the block entities tick (potent sulfur gas).
    pub(in crate::game) pass_mobs: Option<potent_sulfur::PassMobs>,
    /// Entities whose copies are held out of storage; their handles refuse until stored back.
    pub(in crate::game::server_side) held_out: Vec<u32>,
    /// What the boats' and minecarts' ticks know about the mobs: their box reach
    /// (`vehicle_mob_reach`) and a grid of their boxes (`mobs_meeting`).
    pub(in crate::game::server_side) vehicle_mobs: boat_passengers::VehicleMobs,
    /// Missing profession IDs already reported for this world session.
    pub(in crate::game::server_side) missing_professions: HashSet<String>,
    /// What changed in entities' colliders this tick, for the players who track them.
    pub(in crate::game::server_side) collider_changes: Vec<native_colliders::ColliderChange>,
    pub(in crate::game::server_side) entity_initialization:
        entity_initialization::EntityInitialization,
    /// The shapes of the open world's levels, from its dimension types.
    pub(in crate::game::server_side) level_shapes: sunrise_client::chunk::LevelShapes,
    /// `IntegratedServer`'s single-player profile, fixed when the server thread is created,
    /// before its startup hooks (game/modding/native_singleplayer_owner.rs). None elsewhere.
    pub(in crate::game::server_side) singleplayer_profile:
        Option<native_singleplayer_owner::SingleplayerProfile>,
    /// `StructureTemplateManager.structureRepository`: the templates in memory
    /// by id, a miss included (game/administration/structure_blocks.rs).
    pub(in crate::game::server_side) structure_templates:
        HashMap<String, Option<structure_blocks::TemplateEntry>>,
    pub(in crate::game::server_side) structure_spawns:
        sunrise_client::mobs::spawn_selection::StructureSpawns,
    /// `test_defs` holds the world's whole test instance registry
    /// (`snapshot_test_defs`), so a miss is final.
    pub(in crate::game::server_side) test_defs_snapshot: bool,
    /// `ServerChunkCache.lastSpawnState` of each level: its spawnable chunks
    /// and each `MobCategory`'s count.
    pub(in crate::game::server_side) spawn_counts: HashMap<Dim, (i32, [i32; 8])>,
    /// The natural spawning policy mods set on each level (`LevelRef::set_spawn_limits`,
    /// `set_spawn_flags`); a level with none spawns as vanilla. Kept while the world is open.
    pub(in crate::game::server_side) spawn_policies:
        HashMap<mod_api::DimensionKey, natural_spawning::SpawnPolicy>,
    /// Scheduled end of the server tick (`MinecraftServer.haveTime`).
    pub(in crate::game::server_side) server_tick_deadline: Option<Instant>,
    /// The block a guest is acting on (game/network/lan_actions/interaction.rs).
    pub(in crate::game::server_side) forced_hit: Option<Hit>,
    /// The part of a right click a guest's packet asks for (game/network/lan_actions/interaction.rs).
    pub(in crate::game::server_side) guest_use: Option<GuestUse>,
    /// The last `use_hand` that ended the click did so with `FAIL`, which
    /// does not consume the action (handleUseItemOn's ANY_BLOCK_USE).
    pub(in crate::game::server_side) use_failed: bool,
    pub(in crate::game::server_side) block_use_original: Vec<(Hand, Option<ItemStack>)>,
    /// Level events of this tick for the guests, with their excluded source.
    pub(in crate::game::server_side) lan_events: Vec<QueuedLevelEvent>,
    /// Jukebox level events 1010 (song name) and 1011 (`None`) for the guests;
    /// the song's network id is only known once the hub is in hand.
    pub(in crate::game::server_side) lan_songs: Vec<([i32; 3], Option<String>)>,
    pub(in crate::game::server_side) lan_cushion_breaks:
        Vec<(sunrise_world::registry::BlockStateId, [f64; 3])>,
    /// Block events (position, b0, b1, state) and entity events (id, event) for the guests.
    pub(in crate::game) lan_block_events: Vec<([i32; 3], i32, i32, u32)>,
    pub(in crate::game) lan_entity_events: Vec<(i32, i8)>,
    /// Successful crits (attacking guest, hit entity), queued while the hub is taken.
    pub(in crate::game::server_side) lan_critical_hits: Vec<(Option<u32>, i32, bool)>,
    /// Spear stabs that landed (the stabbing guest), queued while the hub is
    /// taken: `broadcastEntityEvent(player, 2)`.
    pub(in crate::game::server_side) lan_kinetic_hits: Vec<Option<u32>>,
    /// `LivingEntity.recentKineticEnemies`: when each player (a guest, or the
    /// window's own) last stabbed each entity this use.
    pub(in crate::game::server_side) stabbed: HashMap<(Option<u32>, u32), i64>,
    /// The other players the acting player's spear can reach this turn,
    /// taken from the hub before it (`spear_players`).
    pub(in crate::game::server_side) stab_players: Vec<spear_players::StabPlayer>,
    /// Spear stabs on players worked out in a guest's turn, landed once the
    /// hub is back (`land_player_stabs`).
    pub(in crate::game::server_side) player_stabs: Vec<spear_players::StabBatch>,
    /// The movement a guest's client reported this tick
    /// (`ServerPlayer.getKnownMovement`), for their turn.
    pub(in crate::game::server_side) known_movement: Vec3,
    /// `LivingEntity.lastKineticHitFeedbackTime` of the entities that felt a
    /// spear's hit (`spear::OWN` is this window's player), in game time.
    pub(in crate::game::server_side) kinetic_feedback: HashMap<i32, i64>,
    /// Global level events for every guest (`ServerLevel.globalLevelEvent`), with their level.
    pub(in crate::game::server_side) lan_global_events: Vec<(&'static str, i32, [i32; 3], i32)>,
    /// `ContainerOpenersCounter`: how many players (the host and the guests)
    /// have each chest open.
    pub(in crate::game) lid_openers: HashMap<(i32, i32, i32), i32>,
    pub(in crate::game) lan_acting: Option<u32>,
    /// The abilities of the server player whose turn it is (`as_guest`): their record's
    /// copy, which the turn reads and writes and the put-back at the end of `guest_turn` stores. None outside a turn.
    pub(in crate::game::server_side) acting_abilities:
        Option<sunrise_client::lan::abilities::PlayerAbilities>,
    /// A guest's `/save-all`: saved once their turn ends (`run_guest_command`),
    /// when their own state is back in their record.
    pub(in crate::game::server_side) save_after_guest_turn: bool,
    /// Guests whose records are displaced by nested player swaps, outer first.
    pub(in crate::game::server_side) lan_guest_swaps: Vec<u32>,
    /// Connections whose turn a panic interrupted this tick: the turn helpers record the
    /// player before re-raising, the end-of-turn path records it before its callbacks run and
    /// clears it after a clean put-back. Cleared at the start of every server tick; the crash
    /// save leaves these players' files as last saved (`server_lifecycle`).
    pub(in crate::game::server_side) interrupted_turns: Vec<u32>,
    /// What teardown decided about the interrupted turns, for the writes after it.
    pub(in crate::game::server_side) crash_save: server_lifecycle::CrashSave,
    /// Ender chests a mod opened for a guest inside that guest's turn
    /// (`player_open_ender_chest`), by connection, with their titles: opened as the turn
    /// ends, once the guest's own ender chest is back in their record.
    pub(in crate::game::server_side) lan_ender_menus: Vec<(u32, sunrise_core::nbt::Tag)>,
    /// The block the acting player is breaking (`break_block`): its destroy
    /// event, and the ones before it, are the player's own (`Level.levelEvent`
    /// with the player excluded); the events after it are not.
    pub(in crate::game::server_side) lan_break_pos: Option<[i32; 3]>,
    /// Where a player's break puts the block's own drops instead of popping
    /// them (`PlayerRef::destroy_block` with `into_inventory`).
    pub(in crate::game::server_side) break_collect: Option<Vec<ItemStack>>,
    /// The name of the guest `lan_acting` acts for (`Player.getName`).
    pub(in crate::game) lan_acting_name: Option<String>,
    /// Their UUID (`Player.getUUID`).
    pub(in crate::game) lan_acting_uuid: Option<u128>,
    /// Where the mods refused to prime TNT: the sound and game event the level
    /// queued right after the prime are dropped (`prime_tnt_event`).
    pub(in crate::game::server_side) tnt_refused: Option<BlockPos>,
    /// Their permission level: an op's above 0 (`PlayerList.isOp`).
    pub(in crate::game::server_side) lan_acting_permission: Option<u8>,
    /// The spawn protection radius that holds the acting guest back
    /// (`DedicatedServer.isUnderSpawnProtection` before the position test):
    /// None for an operator, without operators, or off a dedicated server.
    pub(in crate::game::server_side) lan_acting_spawn_protection: Option<i32>,
    /// Their entity id, which the hub (often taken out of `lan` while they
    /// act) would otherwise have to be asked for.
    pub(in crate::game) lan_acting_entity: Option<u32>,
    /// Whether the player currently swapped into the game is connectionless.
    pub(in crate::game::server_side) lan_acting_fake: bool,
    /// Command feedback to forward to a guest.
    pub(in crate::game) chat_capture: Option<Vec<sunrise_client::text::Text>>,
    /// Packets for the guest acting now, sent once its action returns.
    pub(in crate::game) guest_packets: Vec<sunrise_protocol::packets::play::Clientbound>,
    /// The acting guest's cooldowns, as (item, ticks): their packet needs the
    /// hub's item registry, which the guest's turn does not hold.
    pub(in crate::game::server_side) guest_cooldowns: Vec<(Arc<str>, i32)>,
    /// The acting guest's own entity events (`ServerPlayer.connection.send`),
    /// sent with their entity id, which the guest's turn does not hold.
    pub(in crate::game) guest_entity_events: Vec<i8>,
    /// The acting guest's equipment breaks (`onEquippedItemBroken`'s entity
    /// events), broadcast with their entity id once their turn ends.
    pub(in crate::game::server_side) guest_equipment_breaks: Vec<i8>,
    /// Block sounds queued by the acting guest, excluding that guest.
    pub(in crate::game::server_side) guest_block_sounds: Vec<(String, [f64; 3], f32, f32)>,
    /// `Player.playSound` of the acting guest (event, position, volume,
    /// pitch): `level.playSound(player, ...)` leaves the guest out, whose
    /// own client plays it.
    pub(in crate::game::server_side) guest_player_sounds: Vec<(String, [f64; 3], f32, f32)>,
    /// Sounds bound to an entity this tick, for the players (`game/interface/level_sounds.rs`).
    pub(in crate::game) bound_sounds: Vec<level_sounds::BoundSound>,
    /// `broadcastToAdmins` lines for the other operators that a command queued,
    /// sent once it returns (`send_admin_feedback`), each with the guest whose
    /// command it was (`commandSource() != source`): None for a block, a
    /// minecart, the console or the host.
    pub(in crate::game::server_side) admin_feedback: Vec<(sunrise_client::text::Text, Option<u32>)>,
    /// The server console is running a command: whether its feedback goes to
    /// the operators (`DedicatedServer.shouldInformAdmins`,
    /// `broadcast-console-to-ops`).
    pub(in crate::game::server_side) console_broadcast: Option<bool>,
    /// The command the console is running came over RCON: its source is named `Rcon`
    /// (`RconConsoleSource.createCommandSourceStack`), not `Server`.
    pub(in crate::game::server_side) rcon_source: bool,
    pub(in crate::game::server_side) bosses: bossbars::BossState,
    pub(in crate::game::server_side) rand_sequences:
        sunrise_client::random_sequences::RandomSequences,
    /// `BrushableBlockEntity`'s brush counts of the blocks being brushed.
    pub(in crate::game::server_side) brushing: brush::Brushing,
    pub(in crate::game::server_side) restore_player_state: bool,
    pub(in crate::game::server_side) acting_input: acting_state::ServerActingInput,
    pub(in crate::game::server_side) pending_guest_ui: acting_state::ServerPendingGuestUi,
    pub(in crate::game) audio_capture_active: bool,
    pub(in crate::game) pending_sounds: Vec<sunrise_client_core::sound_source::ServerSound>,
    pub(in crate::game) pending_particles: Vec<crate::lan::server_effects::ServerQueuedParticle>,
    pub(in crate::game) pending_explosions: Vec<crate::lan::server_effects::ServerExplosion>,
    pub(in crate::game) pending_native_sounds: Vec<creatures::NativeSound>,
    pub(in crate::game) effect_rng: sunrise_core::random::LegacyRandom,
    pub(in crate::game::server_side) sculk: sculk_server::SculkState,
}

impl ServerState {
    // Kept out-of-line so its field temporaries do not bloat `Game::construct`'s unoptimised frame.
    #[inline(never)]
    pub(in crate::game) fn new(
        structure_spawns: sunrise_client::mobs::spawn_selection::StructureSpawns,
    ) -> Self {
        Self {
            structure_spawns,
            data_map_registries: Vec::new(),
            craft_table: None,
            open_container_slots: None,
            destroy_delay: 0,
            world_adopted: false,
            startup_ticket_restore_pending: false,
            next_entity: 0,
            ticked_entities_through: u32::MAX,
            contact_drops: 0..0,
            lectern_pulses: Vec::new(),
            spawn_origin: None,
            spawn_hook_fired: false,
            spawn_hook_cancelled: false,
            spawn_spiral: Default::default(),
            player_spawn: None,
            spawn_wait: Vec::new(),
            attack_ticks: 100,
            attack_item: None,
            last_hurt_by_player: HashMap::new(),
            lead_killers: HashMap::new(),
            love_cause: HashMap::new(),
            died: Default::default(),
            crossbow_sounds: [false; 2],
            shield_cooldown: 0,
            command_source: None,
            native_command_filter: false,
            // `computeNextAutosaveInterval` at the clock's 20 ticks a second.
            ticks_until_autosave: 6000,
            applied_modifiers: Vec::new(),
            xp_delay: 0,
            ordered: Default::default(),
            level_up_sound: Default::default(),
            predicted_swings: 0,
            player_beds: Vec::new(),
            sleep_seen: (0, 0),
            world_root: PathBuf::new(),
            carrier_retention_snapshot: Vec::new(),
            owner_migration: None,
            world_debug: false,
            server_trips: Vec::new(),
            level_teleports: Vec::new(),
            teleport_notes: Vec::new(),
            level_pass: Default::default(),
            respawn_config: None,
            end_portal_travel: false,
            tick_scratch: TickScratch::default(),
            tick_chunks: Default::default(),
            location_armor: Default::default(),
            leash_boats: Vec::new(),
            watched_vehicles: Vec::new(),
            fight: dragon::Fight::default(),
            knot_ids: Default::default(),
            broken_knots: Vec::new(),
            riding_order: 0,
            started_riding: Vec::new(),
            rotation_forced: false,
            pending_vehicle: None,
            ender_items: vec![None; 27],
            ender_open: None,
            creaking: Default::default(),
            spawn_scratch: Default::default(),
            hoppers: HashMap::new(),
            dispense_source: None,
            slot_changed_active: Vec::new(),
            contents_changing: Vec::new(),
            mod_container_viewers: HashMap::new(),
            item_columns: None,
            pickup_columns: None,
            pass_mobs: None,
            held_out: Vec::with_capacity(4),
            vehicle_mobs: Default::default(),
            missing_professions: Default::default(),
            collider_changes: Vec::new(),
            entity_initialization: Default::default(),
            level_shapes: Default::default(),
            singleplayer_profile: None,
            structure_templates: HashMap::new(),
            test_defs_snapshot: false,
            spawn_counts: HashMap::new(),
            spawn_policies: HashMap::new(),
            server_tick_deadline: None,
            forced_hit: None,
            guest_use: None,
            use_failed: false,
            block_use_original: Vec::new(),
            lan_events: Vec::new(),
            lan_songs: Vec::new(),
            lan_cushion_breaks: Vec::new(),
            lan_block_events: Vec::new(),
            lan_entity_events: Vec::new(),
            lan_critical_hits: Vec::new(),
            lan_kinetic_hits: Vec::new(),
            stabbed: HashMap::new(),
            stab_players: Vec::new(),
            player_stabs: Vec::new(),
            known_movement: Vec3::ZERO,
            kinetic_feedback: HashMap::new(),
            lan_global_events: Vec::new(),
            lid_openers: HashMap::new(),
            lan_acting: None,
            acting_abilities: None,
            save_after_guest_turn: false,
            lan_guest_swaps: Vec::new(),
            interrupted_turns: Vec::new(),
            crash_save: Default::default(),
            lan_ender_menus: Vec::new(),
            lan_break_pos: None,
            break_collect: None,
            guest_packets: Vec::new(),
            guest_cooldowns: Vec::new(),
            guest_entity_events: Vec::new(),
            guest_equipment_breaks: Vec::new(),
            guest_block_sounds: Vec::new(),
            guest_player_sounds: Vec::new(),
            bound_sounds: Vec::new(),
            lan_acting_name: None,
            lan_acting_uuid: None,
            tnt_refused: None,
            lan_acting_permission: None,
            lan_acting_spawn_protection: None,
            lan_acting_entity: None,
            lan_acting_fake: false,
            chat_capture: None,
            admin_feedback: Vec::new(),
            console_broadcast: None,
            rcon_source: false,
            bosses: Default::default(),
            rand_sequences: Default::default(),
            brushing: Default::default(),
            restore_player_state: true,
            acting_input: acting_state::ServerActingInput::default(),
            pending_guest_ui: acting_state::ServerPendingGuestUi::default(),
            audio_capture_active: false,
            pending_sounds: Vec::new(),
            pending_particles: Vec::new(),
            pending_explosions: Vec::new(),
            pending_native_sounds: Vec::new(),
            effect_rng: sunrise_core::random::LegacyRandom::new(0),
            sculk: Default::default(),
        }
    }
}

#[allow(dead_code)]
#[repr(transparent)]
pub(in crate::game) struct ServerOnly<T>(pub(in crate::game::server_side) T);

#[allow(dead_code)]
impl<T> ServerOnly<T> {
    #[inline(always)]
    pub(in crate::game) fn new(value: T) -> Self {
        Self(value)
    }
}

#[allow(dead_code)]
#[repr(transparent)]
pub(in crate::game) struct ServerShared<T>(pub(in crate::game) T);

#[allow(dead_code)]
impl<T> ServerShared<T> {
    #[inline(always)]
    pub(in crate::game) fn new(value: T) -> Self {
        Self(value)
    }
}
