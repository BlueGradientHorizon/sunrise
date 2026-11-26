
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "blocks/beacon_cull_tests.rs"]
pub(in crate::game) mod beacon_cull_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "entities/crosshair_pick_tests.rs"]
pub(in crate::game) mod crosshair_pick_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/dragon_boss_bar_name_tests.rs"]
pub(in crate::game) mod dragon_boss_bar_name_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "blocks/enchant_power_beam_tests.rs"]
pub(in crate::game) mod enchant_power_beam_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/gui_layer_tests.rs"]
pub(in crate::game) mod gui_layer_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "items/hand_part_tests.rs"]
pub(in crate::game) mod hand_part_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/highlight_tip_tests.rs"]
pub(in crate::game) mod highlight_tip_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/mod_menu_decisions_tests.rs"]
pub(in crate::game) mod mod_menu_decisions_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/mod_raycast_tests.rs"]
pub(in crate::game) mod mod_raycast_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/mod_recipe_property_tests.rs"]
pub(in crate::game) mod mod_recipe_property_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_general_renderers_tests.rs"]
pub(in crate::game) mod native_general_renderers_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_key_mappings_tests.rs"]
pub(in crate::game) mod native_key_mappings_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_scene_live_tests.rs"]
pub(in crate::game) mod native_scene_live_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_small_reads_tests.rs"]
pub(in crate::game) mod native_small_reads_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/options_close_tests.rs"]
pub(in crate::game) mod options_close_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/polytone_frame_tests.rs"]
pub(in crate::game) mod polytone_frame_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/recipe_properties_tests.rs"]
pub(in crate::game) mod recipe_properties_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/screen_render_phase_tests.rs"]
pub(in crate::game) mod screen_render_phase_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/tests/sign_chain_tests.rs"]
pub(in crate::game) mod sign_chain_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "mobs/spawner_client_tick_tests.rs"]
pub(in crate::game) mod spawner_client_tick_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "blocks/sulfur_bucket_tests.rs"]
pub(in crate::game) mod sulfur_bucket_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "runtime/core_tests_client.rs"]
pub(in crate::game) mod core_tests_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/disabled_block_tests_client.rs"]
pub(in crate::game) mod disabled_block_tests_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/initial_client_models_tests_client.rs"]
pub(in crate::game) mod initial_client_models_tests_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_guest_turn_tests_client.rs"]
pub(in crate::game) mod native_guest_turn_tests_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_level_sound_except_tests_client.rs"]
pub(in crate::game) mod native_level_sound_except_tests_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "items/projectile_spawned_tests_client.rs"]
pub(in crate::game) mod projectile_spawned_tests_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/stacked_on_other_tests_client.rs"]
pub(in crate::game) mod stacked_on_other_tests_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "entities/vehicle_removal_tests_client.rs"]
pub(in crate::game) mod vehicle_removal_tests_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "blocks/world_fluids_tests_client.rs"]
pub(in crate::game) mod world_fluids_tests_client;
#[cfg(feature = "client")]
#[path = "world/server_packs.rs"]
pub(in crate::game) mod server_packs;

#[path = "world/packs/mod.rs"]
pub(in crate::game) mod packs;

#[cfg(feature = "client")]
#[path = "runtime/launch_timeout.rs"]
pub(in crate::game) mod launch_timeout;

#[path = "runtime/agent/mod.rs"]
pub(in crate::game) mod agent;

#[path = "network/viewed_worlds/mod.rs"]
pub(in crate::game) mod viewed_worlds;

#[path = "network/remote_vehicles.rs"]
pub(in crate::game) mod remote_vehicles;

#[cfg(feature = "client")]
#[path = "network/remote_tags.rs"]
pub(in crate::game) mod remote_tags;

#[cfg(feature = "client")]
#[path = "network/remote_removal.rs"]
pub(in crate::game) mod remote_removal;

#[path = "network/remote_projectiles.rs"]
pub(in crate::game) mod remote_projectiles;

#[path = "network/remote_chunks.rs"]
pub(in crate::game) mod remote_chunks;

#[path = "network/remote_camera.rs"]
pub(in crate::game) mod remote_camera;

#[path = "network/remote/mod_client.rs"]
pub(in crate::game) mod remote_client;

#[cfg(feature = "client")]
#[path = "network/lan_particles_client.rs"]
pub(in crate::game) mod lan_particles_client;

#[cfg(feature = "client")]
#[path = "network/lan/mod_client.rs"]
pub(in crate::game) mod lan_client;

#[cfg(feature = "client")]
#[path = "native_hooks/mod_client.rs"]
pub(in crate::game) mod native_hooks_client;

#[cfg(feature = "client")]
#[path = "modding/native_visuals.rs"]
pub(in crate::game) mod native_visuals;

#[cfg(feature = "client")]
#[path = "modding/native_viewed_levels.rs"]
pub(in crate::game) mod native_viewed_levels;

#[cfg(feature = "client")]
#[path = "modding/native_special_renderers.rs"]
pub(in crate::game) mod native_special_renderers;

#[cfg(feature = "client")]
#[path = "modding/native_panel.rs"]
pub(in crate::game) mod native_panel;

#[cfg(feature = "client")]
#[path = "modding/native_packet_builders.rs"]
pub(in crate::game) mod native_packet_builders;

#[path = "modding/native_net.rs"]
pub(in crate::game) mod native_net;

#[cfg(feature = "client")]
#[path = "modding/native_local_player_writes.rs"]
pub(in crate::game) mod native_local_player_writes;

#[cfg(feature = "client")]
#[path = "modding/native_living_effects_client.rs"]
pub(in crate::game) mod native_living_effects_client;

#[cfg(feature = "client")]
#[path = "modding/native_key_mappings.rs"]
pub(in crate::game) mod native_key_mappings;

#[cfg(feature = "client")]
#[path = "modding/native_item_particles.rs"]
pub(in crate::game) mod native_item_particles;

#[path = "modding/native_hotbar_slot.rs"]
pub(in crate::game) mod native_hotbar_slot;

#[cfg(feature = "client")]
#[path = "modding/native_entity_riding_client.rs"]
pub(in crate::game) mod native_entity_riding_client;

#[cfg(feature = "client")]
#[path = "modding/native_entity_refs_client.rs"]
pub(in crate::game) mod native_entity_refs_client;

#[path = "modding/native_connection_groups.rs"]
pub(in crate::game) mod native_connection_groups;

#[cfg(feature = "client")]
#[path = "modding/native_client_particles.rs"]
pub(in crate::game) mod native_client_particles;

#[cfg(feature = "client")]
#[path = "modding/native_block_sprites.rs"]
pub(in crate::game) mod native_block_sprites;

#[cfg(feature = "client")]
#[path = "modding/native_block_entity_draw.rs"]
pub mod native_block_entity_draw;

#[cfg(feature = "client")]
#[path = "modding/gui_clips.rs"]
pub(in crate::game) mod gui_clips;

#[cfg(feature = "client")]
#[path = "modding/entity_sizes_client.rs"]
pub(in crate::game) mod entity_sizes_client;

#[cfg(feature = "client")]
#[path = "modding/client_executor.rs"]
pub(in crate::game) mod client_executor;

#[cfg(feature = "client")]
#[path = "modding/client_cracks.rs"]
pub(in crate::game) mod client_cracks;

#[cfg(feature = "client")]
#[path = "mobs/sniffers.rs"]
pub(in crate::game) mod sniffers;

#[cfg(feature = "client")]
#[path = "mobs/ominous_item_spawner/mod_client.rs"]
pub(in crate::game) mod ominous_item_spawner_client;

#[cfg(feature = "client")]
#[path = "mobs/nautili.rs"]
pub(in crate::game) mod nautili;

#[cfg(feature = "client")]
#[path = "mobs/dragon_client.rs"]
pub(in crate::game) mod dragon_client;

#[cfg(feature = "client")]
#[path = "mobs/creatures/mod_client.rs"]
pub(in crate::game) mod creatures_client;

#[cfg(feature = "client")]
#[path = "mobs/breezes_client.rs"]
pub(in crate::game) mod breezes_client;

#[cfg(feature = "client")]
#[path = "items/use_hand_client.rs"]
pub(in crate::game) mod use_hand_client;

#[cfg(feature = "client")]
#[path = "items/trident_client.rs"]
pub(in crate::game) mod trident_client;

#[cfg(feature = "client")]
#[path = "items/trading_client.rs"]
pub(in crate::game) mod trading_client;

#[path = "items/recipe_book.rs"]
pub(in crate::game) mod recipe_book;

#[path = "items/potions.rs"]
pub(in crate::game) mod potions;

#[cfg(feature = "client")]
#[path = "items/item_use_client.rs"]
pub(in crate::game) mod item_use_client;

#[cfg(feature = "client")]
#[path = "items/item_entities_client.rs"]
pub(in crate::game) mod item_entities_client;

#[cfg(feature = "client")]
#[path = "items/fishing_client.rs"]
pub(in crate::game) mod fishing_client;

#[cfg(feature = "client")]
#[path = "items/crossbow_client.rs"]
pub(in crate::game) mod crossbow_client;

#[cfg(feature = "client")]
#[path = "items/combat_client.rs"]
pub(in crate::game) mod combat_client;

#[cfg(feature = "client")]
#[path = "items/client_attack_targets.rs"]
pub(in crate::game) mod client_attack_targets;

#[cfg(feature = "client")]
#[path = "interface/wind_client.rs"]
pub(in crate::game) mod wind_client;

#[cfg(feature = "client")]
#[path = "interface/tutorial.rs"]
pub(in crate::game) mod tutorial;

#[cfg(feature = "client")]
#[path = "interface/titles.rs"]
pub(in crate::game) mod titles;

#[cfg(feature = "client")]
#[path = "interface/startup_menu.rs"]
pub(in crate::game) mod startup_menu;

#[cfg(feature = "client")]
#[path = "interface/spectator.rs"]
pub(in crate::game) mod spectator;

#[cfg(feature = "client")]
#[path = "interface/sign_edit.rs"]
pub(in crate::game) mod sign_edit;

#[cfg(feature = "client")]
#[path = "interface/shader_packs.rs"]
pub(in crate::game) mod shader_packs;

#[path = "interface/screen_build.rs"]
pub(in crate::game) mod screen_build;

#[path = "interface/render_tick.rs"]
pub(in crate::game) mod render_tick;

#[cfg(feature = "client")]
#[path = "interface/pointer_capture.rs"]
pub(in crate::game) mod pointer_capture;

#[cfg(feature = "client")]
#[path = "interface/play_keys.rs"]
pub(in crate::game) mod play_keys;

#[cfg(feature = "client")]
#[path = "interface/pad/mod.rs"]
pub(in crate::game) mod pad;

#[path = "interface/options.rs"]
pub(in crate::game) mod options;

#[cfg(feature = "client")]
#[path = "interface/native_client_screens.rs"]
pub(in crate::game) mod native_client_screens;

#[path = "interface/names.rs"]
pub(in crate::game) mod names;

#[cfg(feature = "client")]
#[path = "interface/music.rs"]
pub(in crate::game) mod music;

#[path = "interface/mount_menu.rs"]
pub(in crate::game) mod mount_menu;

#[cfg(feature = "client")]
#[path = "interface/mod_toasts.rs"]
pub(in crate::game) mod mod_toasts;

#[cfg(feature = "client")]
#[path = "interface/locator_bar.rs"]
pub(in crate::game) mod locator_bar;

#[cfg(feature = "client")]
#[path = "interface/level_particles_client.rs"]
pub(in crate::game) mod level_particles_client;

#[path = "interface/item_draw.rs"]
pub(in crate::game) mod item_draw;

#[cfg(feature = "client")]
#[path = "interface/gui_entity.rs"]
pub(in crate::game) mod gui_entity;

#[cfg(feature = "client")]
#[path = "interface/extras.rs"]
pub(in crate::game) mod extras;

#[path = "interface/dialogs.rs"]
pub(in crate::game) mod dialogs;

#[cfg(feature = "client")]
#[path = "interface/demo_intro.rs"]
pub(in crate::game) mod demo_intro;

#[cfg(feature = "client")]
#[path = "interface/debug_options.rs"]
pub(in crate::game) mod debug_options;

#[path = "interface/debug_keys.rs"]
pub(in crate::game) mod debug_keys;

#[cfg(feature = "client")]
#[path = "interface/container_hooks.rs"]
pub(in crate::game) mod container_hooks;

#[cfg(feature = "client")]
#[path = "interface/clipboard.rs"]
pub(in crate::game) mod clipboard;

#[cfg(feature = "client")]
#[path = "interface/client_reload/mod.rs"]
pub(in crate::game) mod client_reload;

#[cfg(feature = "client")]
#[path = "interface/client_ambient.rs"]
pub(in crate::game) mod client_ambient;

#[cfg(feature = "client")]
#[path = "interface/chat.rs"]
pub(in crate::game) mod chat;

#[path = "interface/books.rs"]
pub(in crate::game) mod books;

#[cfg(feature = "client")]
#[path = "interface/advancements_screen.rs"]
pub(in crate::game) mod advancements_screen;

#[cfg(feature = "client")]
#[path = "interface/advancements/mod_client.rs"]
pub(in crate::game) mod advancements_client;

#[path = "entities/text_displays.rs"]
pub(in crate::game) mod text_displays;

#[cfg(feature = "client")]
#[path = "entities/pick_accumulator.rs"]
pub(in crate::game) mod pick_accumulator;

#[cfg(feature = "client")]
#[path = "entities/minecarts_client.rs"]
pub(in crate::game) mod minecarts_client;

#[path = "entities/lightning.rs"]
pub(in crate::game) mod lightning;

#[cfg(feature = "client")]
#[path = "entities/fireworks_client.rs"]
pub(in crate::game) mod fireworks_client;

#[path = "blocks/stations.rs"]
pub(in crate::game) mod stations;

#[cfg(feature = "client")]
#[path = "blocks/shelf.rs"]
pub(in crate::game) mod shelf;

#[path = "blocks/sculk.rs"]
pub(in crate::game) mod sculk;

#[cfg(feature = "client")]
#[path = "blocks/piston_push_client.rs"]
pub(in crate::game) mod piston_push_client;

#[cfg(feature = "client")]
#[path = "blocks/nether_portal_client.rs"]
pub(in crate::game) mod nether_portal_client;

#[cfg(feature = "client")]
#[path = "blocks/cauldron_interactions_client.rs"]
pub(in crate::game) mod cauldron_interactions_client;

#[cfg(feature = "client")]
#[path = "blocks/block_entity_anims_client.rs"]
pub(in crate::game) mod block_entity_anims_client;

#[cfg(feature = "client")]
#[path = "administration/commands/mod_client.rs"]
pub(in crate::game) mod commands_client;

#[cfg(feature = "client")]
#[path = "administration/command_minecarts.rs"]
pub(in crate::game) mod command_minecarts;
