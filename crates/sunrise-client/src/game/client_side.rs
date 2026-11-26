//! Client-owned fields of [`super::Game`].
use super::*;

#[cfg(feature = "client")]
#[path = "runtime/activity.rs"]
pub(in crate::game) mod activity;
#[cfg(feature = "client")]
#[path = "interface/advancements/mod_client.rs"]
pub(in crate::game) mod advancements_client;
#[cfg(feature = "client")]
#[path = "interface/advancements_screen.rs"]
pub(in crate::game) mod advancements_screen;
#[path = "runtime/agent/mod.rs"]
pub(in crate::game) mod agent;
#[cfg(feature = "client")]
#[path = "runtime/agent_settings.rs"]
pub(in crate::game) mod agent_settings;
#[cfg(feature = "client")]
#[path = "mobs/allays_client.rs"]
pub(in crate::game) mod allays_client;
#[cfg(feature = "client")]
#[path = "blocks/beacon.rs"]
pub(in crate::game) mod beacon;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "blocks/beacon_cull_tests.rs"]
pub(in crate::game) mod beacon_cull_tests;
#[cfg(feature = "client")]
#[path = "blocks/block_breaking.rs"]
pub(in crate::game) mod block_breaking;
#[cfg(feature = "client")]
#[path = "interface/block_editor.rs"]
pub(in crate::game) mod block_editor;
#[cfg(feature = "client")]
#[path = "blocks/block_entity_anims_client.rs"]
pub(in crate::game) mod block_entity_anims_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/block_screen_effect_draw_tests.rs"]
pub(in crate::game) mod block_screen_effect_draw_tests;
#[cfg(feature = "client")]
#[path = "entities/boat_rider_fixture.rs"]
pub(in crate::game) mod boat_rider_fixture;
#[cfg(feature = "client")]
#[path = "entities/boats/mod_client.rs"]
pub(in crate::game) mod boats_client;
#[path = "interface/books.rs"]
pub(in crate::game) mod books;
#[cfg(feature = "client")]
#[path = "blocks/bookshelves.rs"]
pub(in crate::game) mod bookshelves;
#[cfg(feature = "client")]
#[path = "items/bundles.rs"]
pub(in crate::game) mod bundles;
#[cfg(feature = "client")]
#[path = "interface/camera_hud.rs"]
pub(in crate::game) mod camera_hud;
#[cfg(feature = "client")]
#[path = "network/cauldron_prediction_client.rs"]
pub(in crate::game) mod cauldron_prediction_client;
#[cfg(feature = "client")]
#[path = "interface/chat.rs"]
pub(in crate::game) mod chat;
#[cfg(feature = "client")]
#[path = "interface/client_ambient.rs"]
pub(in crate::game) mod client_ambient;
#[cfg(feature = "client")]
#[path = "items/client_attack_targets.rs"]
pub(in crate::game) mod client_attack_targets;
#[cfg(feature = "client")]
#[path = "modding/client_cracks.rs"]
pub(in crate::game) mod client_cracks;
#[cfg(feature = "client")]
#[path = "modding/client_executor.rs"]
pub(in crate::game) mod client_executor;
#[path = "modding/client_item_extensions.rs"]
pub(in crate::game) mod client_item_extensions;
#[cfg(feature = "client")]
#[path = "interface/client_reload/mod.rs"]
pub(in crate::game) mod client_reload;
#[cfg(feature = "client")]
#[path = "runtime/client_tick_steps.rs"]
pub(in crate::game) mod client_tick_steps;
#[cfg(feature = "client")]
#[path = "interface/clipboard.rs"]
pub(in crate::game) mod clipboard;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/cloud_height_frame_tests.rs"]
pub(in crate::game) mod cloud_height_frame_tests;
#[cfg(feature = "client")]
#[path = "interface/code_of_conduct.rs"]
pub(in crate::game) mod code_of_conduct;
#[path = "administration/command_blocks.rs"]
pub(in crate::game) mod command_blocks;
#[cfg(feature = "client")]
#[path = "administration/command_minecarts.rs"]
pub(in crate::game) mod command_minecarts;
#[cfg(feature = "client")]
#[path = "administration/commands/mod_client.rs"]
pub(in crate::game) mod commands_client;
#[cfg(feature = "client")]
#[path = "blocks/composter_client.rs"]
pub(in crate::game) mod composter_client;
#[cfg(feature = "client")]
#[path = "interface/container_hooks.rs"]
pub(in crate::game) mod container_hooks;
#[path = "interface/container_layout.rs"]
pub(in crate::game) mod container_layout;
#[cfg(feature = "client")]
#[path = "runtime/control.rs"]
pub(in crate::game) mod control;
#[cfg(feature = "client")]
#[path = "items/creative_hotbar.rs"]
pub(in crate::game) mod creative_hotbar;
#[cfg(feature = "client")]
#[path = "mobs/creatures/mod_client.rs"]
pub(in crate::game) mod creatures_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "entities/crosshair_pick_tests.rs"]
pub(in crate::game) mod crosshair_pick_tests;
#[cfg(feature = "client")]
#[path = "interface/custom_sky.rs"]
pub(in crate::game) mod custom_sky;
#[path = "interface/debug_keys.rs"]
pub(in crate::game) mod debug_keys;
#[cfg(feature = "client")]
#[path = "interface/debug_options.rs"]
pub(in crate::game) mod debug_options;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/debug_renderer_tests.rs"]
pub(in crate::game) mod debug_renderer_tests;
#[cfg(feature = "client")]
#[path = "interface/debug_renderers.rs"]
pub(in crate::game) mod debug_renderers;
#[cfg(feature = "client")]
#[path = "interface/demo_intro.rs"]
pub(in crate::game) mod demo_intro;
#[path = "interface/dialogs.rs"]
pub(in crate::game) mod dialogs;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/dragon_boss_bar_name_tests.rs"]
pub(in crate::game) mod dragon_boss_bar_name_tests;
#[cfg(feature = "client")]
#[path = "mobs/dragon_client.rs"]
pub(in crate::game) mod dragon_client;
#[cfg(feature = "client")]
#[path = "modding/draw_boundary.rs"]
pub(in crate::game) mod draw_boundary;
#[cfg(feature = "client")]
#[path = "interface/dynamic_lights.rs"]
pub(in crate::game) mod dynamic_lights;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "blocks/enchant_power_beam_tests.rs"]
pub(in crate::game) mod enchant_power_beam_tests;
#[cfg(feature = "client")]
#[path = "world/end_client.rs"]
pub(in crate::game) mod end_client;
#[path = "entities/entity_client.rs"]
pub(in crate::game) mod entity_client;
#[path = "interface/entity_culling/mod.rs"]
pub(in crate::game) mod entity_culling;
#[cfg(feature = "client")]
#[path = "modding/entity_part_state.rs"]
pub(in crate::game) mod entity_part_state;
#[cfg(feature = "client")]
#[path = "modding/entity_sizes_client.rs"]
pub(in crate::game) mod entity_sizes_client;
#[path = "entities/entity_sounds.rs"]
pub(in crate::game) mod entity_sounds;
#[path = "entities/equipment_piece.rs"]
pub(in crate::game) mod equipment_piece;
#[cfg(feature = "client")]
#[path = "runtime/event_loop.rs"]
pub(in crate::game) mod event_loop;
#[path = "runtime/events.rs"]
pub(in crate::game) mod events;
#[cfg(feature = "client")]
#[path = "interface/extras.rs"]
pub(in crate::game) mod extras;
#[cfg(feature = "client")]
#[path = "mobs/fangs_client.rs"]
pub(in crate::game) mod fangs_client;
#[cfg(feature = "client")]
#[path = "world/far_terrain_edits.rs"]
pub(in crate::game) mod far_terrain_edits;
#[cfg(feature = "client")]
#[path = "entities/fireworks_client.rs"]
pub(in crate::game) mod fireworks_client;
#[cfg(feature = "client")]
#[path = "items/fishing_client.rs"]
pub(in crate::game) mod fishing_client;
#[cfg(feature = "client")]
#[path = "interface/focused_text_field.rs"]
pub(in crate::game) mod focused_text_field;
#[cfg(feature = "client")]
#[path = "interface/fov.rs"]
pub(in crate::game) mod fov;
#[cfg(feature = "client")]
#[path = "interface/frame_geometry.rs"]
pub(in crate::game) mod frame_geometry;
#[cfg(feature = "friends")]
#[path = "network/friends.rs"]
pub(in crate::game) mod friends;
#[cfg(not(feature = "friends"))]
#[path = "network/friends_off.rs"]
pub(in crate::game) mod friends;
#[cfg(feature = "client")]
#[path = "game_client.rs"]
pub(in crate::game) mod game_client;
#[cfg(feature = "client")]
#[path = "interface/game_mode_client.rs"]
pub(in crate::game) mod game_mode_client;
#[cfg(feature = "client")]
#[path = "interface/gamemode_switcher.rs"]
pub(in crate::game) mod gamemode_switcher;
#[cfg(feature = "client")]
#[path = "mobs/general_draw.rs"]
pub(in crate::game) mod general_draw;
#[cfg(feature = "client")]
#[path = "entities/glowing.rs"]
pub(in crate::game) mod glowing;
#[path = "blocks/growth.rs"]
pub(in crate::game) mod growth;
#[cfg(feature = "client")]
#[path = "modding/gui_clips.rs"]
pub(in crate::game) mod gui_clips;
#[cfg(feature = "client")]
#[path = "interface/gui_entity.rs"]
pub(in crate::game) mod gui_entity;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/gui_layer_tests.rs"]
pub(in crate::game) mod gui_layer_tests;
#[cfg(feature = "client")]
#[path = "modding/gui_paint_items.rs"]
pub(in crate::game) mod gui_paint_items;
#[cfg(feature = "client")]
#[path = "modding/gui_pose.rs"]
pub(in crate::game) mod gui_pose;
#[path = "items/hand.rs"]
pub(in crate::game) mod hand;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "items/hand_part_tests.rs"]
pub(in crate::game) mod hand_part_tests;
#[cfg(feature = "client")]
#[path = "runtime/headless.rs"]
pub(in crate::game) mod headless;
#[cfg(feature = "client")]
#[path = "items/held_items_client.rs"]
pub(in crate::game) mod held_items_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/highlight_tip_tests.rs"]
pub(in crate::game) mod highlight_tip_tests;
#[cfg(feature = "client")]
#[path = "interface/hud_editor.rs"]
pub(in crate::game) mod hud_editor;
#[cfg(feature = "client")]
#[path = "interface/hud_room.rs"]
pub(in crate::game) mod hud_room;
#[cfg(feature = "client")]
#[path = "interface/humanoid_setup_anim.rs"]
pub(in crate::game) mod humanoid_setup_anim;
#[cfg(feature = "client")]
#[path = "interface/ime.rs"]
pub(in crate::game) mod ime;
#[cfg(feature = "client")]
#[path = "modding/initial_client_models.rs"]
pub(in crate::game) mod initial_client_models;
#[cfg(feature = "client")]
#[path = "runtime/input_trace.rs"]
pub(in crate::game) mod input_trace;
#[cfg(feature = "client")]
#[path = "world/integrated_client.rs"]
pub(in crate::game) mod integrated_client;
#[path = "items/interactions.rs"]
pub(in crate::game) mod interactions;
#[cfg(feature = "client")]
#[path = "interface/inventory_input.rs"]
pub(in crate::game) mod inventory_input;
#[path = "interface/item_draw.rs"]
pub(in crate::game) mod item_draw;
#[cfg(feature = "client")]
#[path = "modding/item_model_context.rs"]
pub(in crate::game) mod item_model_context;
#[cfg(feature = "client")]
#[path = "items/item_particles.rs"]
pub(in crate::game) mod item_particles;
#[cfg(feature = "client")]
#[path = "administration/jigsaw_edit.rs"]
pub(in crate::game) mod jigsaw_edit;
#[path = "blocks/jukebox.rs"]
pub(in crate::game) mod jukebox;
#[cfg(feature = "client")]
#[path = "network/lan_screen.rs"]
pub(in crate::game) mod lan_screen;
#[cfg(feature = "client")]
#[path = "runtime/launch_timeout.rs"]
pub(in crate::game) mod launch_timeout;
#[cfg(feature = "client")]
#[path = "interface/level_particles_client.rs"]
pub(in crate::game) mod level_particles_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/lightmap_expression_tests.rs"]
pub(in crate::game) mod lightmap_expression_tests;
#[path = "entities/lightning.rs"]
pub(in crate::game) mod lightning;
#[path = "runtime/loading_screen.rs"]
pub(in crate::game) mod loading_screen;
#[cfg(feature = "client")]
#[path = "runtime/loading_window.rs"]
pub(in crate::game) mod loading_window;
#[cfg(feature = "client")]
#[path = "interface/locator_bar.rs"]
pub(in crate::game) mod locator_bar;
#[cfg(feature = "client")]
#[path = "interface/look.rs"]
pub(in crate::game) mod look;
#[path = "blocks/loom.rs"]
pub(in crate::game) mod loom;
#[cfg(feature = "client")]
#[path = "blocks/machines/mod_client.rs"]
pub(in crate::game) mod machines_client;
#[cfg(feature = "client")]
#[path = "runtime/memory_client.rs"]
pub(in crate::game) mod memory_client;
#[cfg(feature = "client")]
#[path = "mobs/mob_blocks.rs"]
pub(in crate::game) mod mob_blocks;
#[cfg(feature = "client")]
#[path = "mobs/mob_light.rs"]
pub(in crate::game) mod mob_light;
#[cfg(feature = "client")]
#[path = "mobs/mob_sounds.rs"]
pub(in crate::game) mod mob_sounds;
#[cfg(feature = "client")]
#[path = "interface/mod_browser.rs"]
pub(in crate::game) mod mod_browser;
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
#[path = "interface/mod_toasts.rs"]
pub(in crate::game) mod mod_toasts;
#[cfg(feature = "client")]
#[path = "side_ops/modapi_client.rs"]
pub(in crate::game) mod modapi_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/model_baking_completed_tests.rs"]
pub(in crate::game) mod model_baking_completed_tests;
#[cfg(feature = "client")]
#[path = "blocks/motion_ids.rs"]
pub(in crate::game) mod motion_ids;
#[path = "interface/mount_menu.rs"]
pub(in crate::game) mod mount_menu;
#[cfg(feature = "client")]
#[path = "interface/music.rs"]
pub(in crate::game) mod music;
#[path = "interface/names.rs"]
pub(in crate::game) mod names;
#[cfg(feature = "client")]
#[path = "interface/narrator.rs"]
pub(in crate::game) mod narrator;
#[cfg(feature = "client")]
#[path = "modding/native_audio_streams.rs"]
pub(in crate::game) mod native_audio_streams;
#[cfg(feature = "client")]
#[path = "modding/native_block_entity_draw.rs"]
pub mod native_block_entity_draw;
#[cfg(feature = "client")]
#[path = "modding/native_block_sprites.rs"]
pub(in crate::game) mod native_block_sprites;
#[cfg(feature = "client")]
#[path = "modding/native_book_net_client.rs"]
pub(in crate::game) mod native_book_net_client;
#[cfg(feature = "client")]
#[path = "modding/native_book_placement_client.rs"]
pub(in crate::game) mod native_book_placement_client;
#[cfg(feature = "client")]
#[path = "modding/native_book_receive.rs"]
pub(in crate::game) mod native_book_receive;
#[cfg(feature = "client")]
#[path = "modding/native_book_settings_client.rs"]
pub(in crate::game) mod native_book_settings_client;
#[cfg(feature = "client")]
#[path = "modding/native_claimed_items.rs"]
pub(in crate::game) mod native_claimed_items;
#[cfg(feature = "client")]
#[path = "modding/native_client_particles.rs"]
pub(in crate::game) mod native_client_particles;
#[cfg(feature = "client")]
#[path = "modding/native_client_particles_fixture.rs"]
pub(in crate::game) mod native_client_particles_fixture;
#[cfg(feature = "client")]
#[path = "interface/native_client_screens.rs"]
pub(in crate::game) mod native_client_screens;
#[cfg(feature = "client")]
#[path = "modding/native_client_services.rs"]
pub mod native_client_services;
#[cfg(feature = "client")]
#[path = "modding/native_client_session.rs"]
pub(in crate::game) mod native_client_session;
#[path = "modding/native_connection_groups.rs"]
pub(in crate::game) mod native_connection_groups;
#[cfg(feature = "client")]
#[path = "modding/native_custom_geometry.rs"]
pub(in crate::game) mod native_custom_geometry;
#[path = "modding/native_disconnect.rs"]
pub(in crate::game) mod native_disconnect;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_disconnect_tests.rs"]
pub(in crate::game) mod native_disconnect_tests;
#[cfg(feature = "client")]
#[path = "modding/native_dynamic_textures.rs"]
pub(in crate::game) mod native_dynamic_textures;
#[cfg(feature = "client")]
#[path = "modding/native_entity_layers.rs"]
pub(in crate::game) mod native_entity_layers;
#[cfg(feature = "client")]
#[path = "modding/native_entity_refs_client.rs"]
pub(in crate::game) mod native_entity_refs_client;
#[cfg(feature = "client")]
#[path = "modding/native_entity_renderers.rs"]
pub(in crate::game) mod native_entity_renderers;
#[cfg(feature = "client")]
#[path = "modding/native_fluid_render.rs"]
pub(in crate::game) mod native_fluid_render;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_general_renderers_tests.rs"]
pub(in crate::game) mod native_general_renderers_tests;
#[cfg(feature = "client")]
#[path = "native_hooks/mod_client.rs"]
pub(in crate::game) mod native_hooks_client;
#[path = "modding/native_hotbar_slot.rs"]
pub(in crate::game) mod native_hotbar_slot;
#[cfg(feature = "client")]
#[path = "modding/native_instance_groups.rs"]
pub(in crate::game) mod native_instance_groups;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_item_frame_renderers_tests.rs"]
pub(in crate::game) mod native_item_frame_renderers_tests;
#[cfg(feature = "client")]
#[path = "modding/native_item_particles.rs"]
pub(in crate::game) mod native_item_particles;
#[cfg(feature = "client")]
#[path = "modding/native_key_mappings.rs"]
pub(in crate::game) mod native_key_mappings;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_key_mappings_tests.rs"]
pub(in crate::game) mod native_key_mappings_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_level_notify_tests.rs"]
pub(in crate::game) mod native_level_notify_tests;
#[cfg(feature = "client")]
#[path = "modding/native_living_effects_client.rs"]
pub(in crate::game) mod native_living_effects_client;
#[cfg(feature = "client")]
#[path = "modding/native_local_player_writes.rs"]
pub(in crate::game) mod native_local_player_writes;
#[cfg(feature = "client")]
#[path = "modding/native_local_sounds.rs"]
pub(in crate::game) mod native_local_sounds;
#[cfg(feature = "client")]
#[path = "modding/native_mapping_presses.rs"]
pub(in crate::game) mod native_mapping_presses;
#[cfg(feature = "client")]
#[path = "modding/native_menu_refs.rs"]
pub(in crate::game) mod native_menu_refs;
#[cfg(feature = "client")]
#[path = "modding/native_menu_slot_kinds.rs"]
pub(in crate::game) mod native_menu_slot_kinds;
#[cfg(feature = "client")]
#[path = "modding/native_mob_draw.rs"]
pub(in crate::game) mod native_mob_draw;
#[cfg(feature = "client")]
#[path = "modding/native_model_3d.rs"]
pub(in crate::game) mod native_model_3d;
#[path = "modding/native_net.rs"]
pub(in crate::game) mod native_net;
#[cfg(feature = "client")]
#[path = "modding/native_panel.rs"]
pub(in crate::game) mod native_panel;
#[cfg(feature = "client")]
#[path = "modding/native_panel_recipes.rs"]
pub(in crate::game) mod native_panel_recipes;
#[cfg(feature = "client")]
#[path = "modding/native_part_tables.rs"]
pub(in crate::game) mod native_part_tables;
#[cfg(feature = "client")]
#[path = "modding/native_passage_cross.rs"]
pub(in crate::game) mod native_passage_cross;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_player_inventory_tests.rs"]
pub(in crate::game) mod native_player_inventory_tests;
#[path = "modding/native_recipes.rs"]
pub(in crate::game) mod native_recipes;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_render_pair_tests.rs"]
pub(in crate::game) mod native_render_pair_tests;
#[cfg(feature = "client")]
#[path = "modding/native_render_visibility.rs"]
pub(in crate::game) mod native_render_visibility;
#[cfg(feature = "client")]
#[path = "modding/native_scene.rs"]
pub(in crate::game) mod native_scene;
#[cfg(feature = "client")]
#[path = "modding/native_scene_live.rs"]
pub(in crate::game) mod native_scene_live;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_scene_live_tests.rs"]
pub(in crate::game) mod native_scene_live_tests;
#[cfg(feature = "client")]
#[path = "modding/native_section_visibility.rs"]
pub(in crate::game) mod native_section_visibility;
#[cfg(feature = "client")]
#[path = "modding/native_shaders.rs"]
pub(in crate::game) mod native_shaders;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_small_reads_tests.rs"]
pub(in crate::game) mod native_small_reads_tests;
#[cfg(feature = "client")]
#[path = "modding/native_special_renderers.rs"]
pub(in crate::game) mod native_special_renderers;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_spectator_shader_tests.rs"]
pub(in crate::game) mod native_spectator_shader_tests;
#[cfg(feature = "client")]
#[path = "modding/native_terrain_model.rs"]
pub(in crate::game) mod native_terrain_model;
#[cfg(feature = "client")]
#[path = "modding/native_viewed_levels.rs"]
pub(in crate::game) mod native_viewed_levels;
#[cfg(feature = "client")]
#[path = "modding/native_visuals.rs"]
pub(in crate::game) mod native_visuals;
#[cfg(feature = "client")]
#[path = "mobs/nautili.rs"]
pub(in crate::game) mod nautili;
#[cfg(feature = "client")]
#[path = "network/net_harness.rs"]
pub(in crate::game) mod net_harness;
#[cfg(feature = "client")]
#[path = "runtime/no_resources.rs"]
pub(in crate::game) mod no_resources;
#[cfg(feature = "client")]
#[path = "mobs/ominous_item_spawner/mod_client.rs"]
pub(in crate::game) mod ominous_item_spawner_client;
#[path = "interface/options.rs"]
pub(in crate::game) mod options;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/options_close_tests.rs"]
pub(in crate::game) mod options_close_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/pack_clouds_frame_tests.rs"]
pub(in crate::game) mod pack_clouds_frame_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/pack_sources_tests.rs"]
pub(in crate::game) mod pack_sources_tests;
#[path = "world/packs/mod.rs"]
pub(in crate::game) mod packs;
#[cfg(feature = "client")]
#[path = "interface/pad/mod.rs"]
pub(in crate::game) mod pad;
#[cfg(feature = "client")]
#[path = "interface/pad_actions.rs"]
pub(in crate::game) mod pad_actions;
#[cfg(feature = "client")]
#[path = "interface/pad_craft.rs"]
pub(in crate::game) mod pad_craft;
#[cfg(feature = "client")]
#[path = "interface/pad_menu.rs"]
pub(in crate::game) mod pad_menu;
#[cfg(feature = "client")]
#[path = "entities/paintings_client.rs"]
pub(in crate::game) mod paintings_client;
#[cfg(feature = "client")]
#[path = "interface/particle_culling.rs"]
pub(in crate::game) mod particle_culling;
#[path = "interface/perf_link.rs"]
pub(in crate::game) mod perf_link;
#[path = "interface/perspective.rs"]
pub(in crate::game) mod perspective;
#[cfg(feature = "client")]
#[path = "entities/pick_accumulator.rs"]
pub(in crate::game) mod pick_accumulator;
#[path = "entities/pick_entity.rs"]
pub(in crate::game) mod pick_entity;
#[cfg(feature = "client")]
#[path = "entities/pickups.rs"]
pub(in crate::game) mod pickups;
#[cfg(feature = "client")]
#[path = "interface/play_input.rs"]
pub(in crate::game) mod play_input;
#[cfg(feature = "client")]
#[path = "interface/play_keys.rs"]
pub(in crate::game) mod play_keys;
#[cfg(feature = "client")]
#[path = "player_client.rs"]
pub(in crate::game) mod player_client;
#[cfg(feature = "client")]
#[path = "interface/pointer_capture.rs"]
pub(in crate::game) mod pointer_capture;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/polytone_frame_tests.rs"]
pub(in crate::game) mod polytone_frame_tests;
#[path = "items/potions.rs"]
pub(in crate::game) mod potions;
#[path = "interface/profile_capture.rs"]
pub(in crate::game) mod profile_capture;
#[cfg(feature = "client")]
#[path = "entities/pvp.rs"]
pub(in crate::game) mod pvp;
#[cfg(feature = "client")]
#[path = "interface/quick_actions.rs"]
pub(in crate::game) mod quick_actions;
#[cfg(feature = "client")]
#[path = "interface/random_textures.rs"]
pub(in crate::game) mod random_textures;
#[path = "items/recipe_book.rs"]
pub(in crate::game) mod recipe_book;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/recipe_properties_tests.rs"]
pub(in crate::game) mod recipe_properties_tests;
#[cfg(feature = "client")]
#[path = "network/remote_block_writes.rs"]
pub(in crate::game) mod remote_block_writes;
#[path = "network/remote_camera.rs"]
pub(in crate::game) mod remote_camera;
#[path = "network/remote_chat.rs"]
pub(in crate::game) mod remote_chat;
#[path = "network/remote_chunks.rs"]
pub(in crate::game) mod remote_chunks;
#[path = "network/remote_mob_data.rs"]
pub(in crate::game) mod remote_mob_data;
#[cfg(feature = "client")]
#[path = "network/remote_mob_particles.rs"]
pub(in crate::game) mod remote_mob_particles;
#[cfg(feature = "client")]
#[path = "network/remote_mob_use.rs"]
pub(in crate::game) mod remote_mob_use;
#[path = "network/remote_objects.rs"]
pub(in crate::game) mod remote_objects;
#[path = "network/remote_projectiles.rs"]
pub(in crate::game) mod remote_projectiles;
#[cfg(feature = "client")]
#[path = "network/remote_removal.rs"]
pub(in crate::game) mod remote_removal;
#[cfg(feature = "client")]
#[path = "network/remote_stations.rs"]
pub(in crate::game) mod remote_stations;
#[cfg(feature = "client")]
#[path = "network/remote_tags.rs"]
pub(in crate::game) mod remote_tags;
#[path = "network/remote_vehicles.rs"]
pub(in crate::game) mod remote_vehicles;
#[cfg(feature = "client")]
#[path = "interface/render_control.rs"]
pub(in crate::game) mod render_control;
#[cfg(feature = "client")]
#[path = "interface/render_settings.rs"]
pub(in crate::game) mod render_settings;
#[path = "interface/render_tick.rs"]
pub(in crate::game) mod render_tick;
#[path = "runtime/replay.rs"]
pub(in crate::game) mod replay;
#[cfg(feature = "client")]
#[path = "runtime/repro.rs"]
pub(in crate::game) mod repro;
#[path = "runtime/resource_loader.rs"]
pub(in crate::game) mod resource_loader;
#[cfg(feature = "client")]
#[path = "runtime/saves_control.rs"]
pub(in crate::game) mod saves_control;
#[cfg(feature = "client")]
#[path = "world/saving_client.rs"]
pub(in crate::game) mod saving_client;
#[path = "interface/screen_build.rs"]
pub(in crate::game) mod screen_build;
#[cfg(feature = "client")]
#[path = "interface/screen_build_fixture.rs"]
pub(in crate::game) mod screen_build_fixture;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/screen_render_phase_tests.rs"]
pub(in crate::game) mod screen_render_phase_tests;
#[cfg(feature = "client")]
#[path = "interface/screen_room.rs"]
pub(in crate::game) mod screen_room;
#[path = "interface/screen_widgets.rs"]
pub(in crate::game) mod screen_widgets;
#[cfg(feature = "client")]
#[path = "interface/screens.rs"]
pub(in crate::game) mod screens;
#[cfg(feature = "client")]
#[path = "runtime/screenshot_sequence.rs"]
pub(in crate::game) mod screenshot_sequence;
#[path = "interface/screenshot_writer.rs"]
pub(in crate::game) mod screenshot_writer;
#[path = "blocks/sculk.rs"]
pub(in crate::game) mod sculk;
#[path = "network/server_links.rs"]
pub(in crate::game) mod server_links;
#[cfg(feature = "client")]
#[path = "world/server_packs.rs"]
pub(in crate::game) mod server_packs;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/shader_pack_switch_tests.rs"]
pub(in crate::game) mod shader_pack_switch_tests;
#[cfg(feature = "client")]
#[path = "interface/shader_packs.rs"]
pub(in crate::game) mod shader_packs;
#[cfg(feature = "client")]
#[path = "blocks/shelf.rs"]
pub(in crate::game) mod shelf;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/tests/sign_chain_tests.rs"]
pub(in crate::game) mod sign_chain_tests;
#[cfg(feature = "client")]
#[path = "interface/sign_edit.rs"]
pub(in crate::game) mod sign_edit;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/tests/sign_outline_tests.rs"]
pub(in crate::game) mod sign_outline_tests;
#[cfg(feature = "client")]
#[path = "mobs/sniffers.rs"]
pub(in crate::game) mod sniffers;
#[cfg(feature = "client")]
#[path = "network/social.rs"]
pub(in crate::game) mod social;
#[cfg(feature = "client")]
#[path = "mobs/spawner_client.rs"]
pub(in crate::game) mod spawner_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "mobs/spawner_client_tick_tests.rs"]
pub(in crate::game) mod spawner_client_tick_tests;
#[cfg(feature = "client")]
#[path = "items/spear_client.rs"]
pub(in crate::game) mod spear_client;
#[cfg(feature = "client")]
#[path = "interface/spectator.rs"]
pub(in crate::game) mod spectator;
#[cfg(feature = "client")]
#[path = "items/stack_attribute_tooltip.rs"]
pub(in crate::game) mod stack_attribute_tooltip;
#[cfg(feature = "client")]
#[path = "interface/startup_menu.rs"]
pub(in crate::game) mod startup_menu;
#[cfg(feature = "client")]
#[path = "runtime/station_control.rs"]
pub(in crate::game) mod station_control;
#[path = "blocks/stations.rs"]
pub(in crate::game) mod stations;
#[cfg(feature = "client")]
#[path = "blocks/stations_client.rs"]
pub(in crate::game) mod stations_client;
#[cfg(feature = "client")]
#[path = "interface/stats_screen.rs"]
pub(in crate::game) mod stats_screen;
#[path = "world/storage.rs"]
pub(in crate::game) mod storage;
#[cfg(feature = "client")]
#[path = "administration/structure_edit.rs"]
pub(in crate::game) mod structure_edit;
#[cfg(feature = "client")]
#[path = "interface/studio.rs"]
pub(in crate::game) mod studio;
#[cfg(feature = "client")]
#[path = "interface/subtitle_hud.rs"]
pub(in crate::game) mod subtitle_hud;
#[cfg(feature = "client")]
#[path = "interface/subtitles.rs"]
pub(in crate::game) mod subtitles;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "blocks/sulfur_bucket_tests.rs"]
pub(in crate::game) mod sulfur_bucket_tests;
#[cfg(feature = "client")]
#[path = "network/swing_packet.rs"]
pub(in crate::game) mod swing_packet;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/sync_background_tests.rs"]
pub(in crate::game) mod sync_background_tests;
#[cfg(feature = "client")]
#[path = "administration/test_block_edit.rs"]
pub(in crate::game) mod test_block_edit;
#[cfg(feature = "client")]
#[path = "administration/test_instance_edit.rs"]
pub(in crate::game) mod test_instance_edit;
#[path = "entities/text_displays.rs"]
pub(in crate::game) mod text_displays;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interface/text_expression_tests.rs"]
pub(in crate::game) mod text_expression_tests;
#[cfg(feature = "client")]
#[path = "mobs/thrown_item_draw.rs"]
pub(in crate::game) mod thrown_item_draw;
#[cfg(feature = "client")]
#[path = "world/tick_script.rs"]
pub(in crate::game) mod tick_script;
#[cfg(feature = "client")]
#[path = "interface/titles.rs"]
pub(in crate::game) mod titles;
#[cfg(feature = "client")]
#[path = "entities/tnt_client.rs"]
pub(in crate::game) mod tnt_client;
#[path = "items/totem.rs"]
pub(in crate::game) mod totem;
#[cfg(feature = "client")]
#[path = "runtime/transactions_client.rs"]
pub(in crate::game) mod transactions_client;
#[cfg(feature = "client")]
#[path = "blocks/transient_blocks.rs"]
pub(in crate::game) mod transient_blocks;
#[cfg(feature = "client")]
#[path = "blocks/trial_visuals.rs"]
pub(in crate::game) mod trial_visuals;
#[cfg(feature = "client")]
#[path = "items/trident_client.rs"]
pub(in crate::game) mod trident_client;
#[cfg(feature = "client")]
#[path = "interface/tutorial.rs"]
pub(in crate::game) mod tutorial;
#[cfg(feature = "client")]
#[path = "runtime/ui_control.rs"]
pub(in crate::game) mod ui_control;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "items/use_same_item_tests.rs"]
pub(in crate::game) mod use_same_item_tests;
#[cfg(feature = "client")]
#[path = "items/using_item_client.rs"]
pub(in crate::game) mod using_item_client;
#[path = "network/viewed_worlds/mod.rs"]
pub(in crate::game) mod viewed_worlds;
#[cfg(feature = "client")]
#[path = "interface/win.rs"]
pub(in crate::game) mod win;
#[cfg(feature = "client")]
#[path = "runtime/window_handler.rs"]
pub(in crate::game) mod window_handler;
#[cfg(feature = "client")]
#[path = "runtime/window_title.rs"]
pub(in crate::game) mod window_title;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/world_name_tag_tests.rs"]
pub(in crate::game) mod world_name_tag_tests;
#[path = "world/world_rest.rs"]
pub(in crate::game) mod world_rest;
#[cfg(feature = "client")]
#[path = "world/world_session.rs"]
pub(in crate::game) mod world_session;
#[cfg(feature = "client")]
#[path = "world/world_update.rs"]
pub(in crate::game) mod world_update;
#[cfg(feature = "client")]
#[path = "mobs/creatures/world_view_client.rs"]
pub(in crate::game) mod world_view_client;
#[cfg(feature = "client")]
#[path = "mobs/zombie_villagers_client.rs"]
pub(in crate::game) mod zombie_villagers_client;

#[path = "modding/api_values_client.rs"]
pub(in crate::game) mod api_values_client;
#[cfg(feature = "client")]
#[path = "entities/armor_stands_client.rs"]
pub(in crate::game) mod armor_stands_client;
#[cfg(feature = "client")]
#[path = "modding/attachment_sync_client.rs"]
pub(in crate::game) mod attachment_sync_client;
#[cfg(feature = "client")]
#[path = "blocks/banners_client.rs"]
pub(in crate::game) mod banners_client;
#[cfg(feature = "client")]
#[path = "blocks/block_entity_anims_common_client.rs"]
pub(in crate::game) mod block_entity_anims_common_client;
#[cfg(feature = "client")]
#[path = "blocks/block_items_client.rs"]
pub(in crate::game) mod block_items_client;
#[cfg(feature = "client")]
#[path = "blocks/block_particles.rs"]
pub(in crate::game) mod block_particles;
#[cfg(feature = "client")]
#[path = "blocks/block_particles_client.rs"]
pub(in crate::game) mod block_particles_client;
#[cfg(feature = "client")]
#[path = "interface/bossbars_client.rs"]
pub(in crate::game) mod bossbars_client;
#[cfg(feature = "client")]
#[path = "mobs/breezes_client.rs"]
pub(in crate::game) mod breezes_client;
#[cfg(feature = "client")]
#[path = "blocks/brush_client.rs"]
pub(in crate::game) mod brush_client;
#[cfg(feature = "client")]
#[path = "items/bucket_empty_client.rs"]
pub(in crate::game) mod bucket_empty_client;
#[cfg(feature = "client")]
#[path = "blocks/campfire_client.rs"]
pub(in crate::game) mod campfire_client;
#[cfg(feature = "client")]
#[path = "blocks/campfire_douse_client.rs"]
pub(in crate::game) mod campfire_douse_client;
#[cfg(feature = "client")]
#[path = "blocks/cartography_client.rs"]
pub(in crate::game) mod cartography_client;
#[cfg(feature = "client")]
#[path = "blocks/cauldron_interactions_client.rs"]
pub(in crate::game) mod cauldron_interactions_client;
#[cfg(feature = "client")]
#[path = "blocks/cauldrons_client.rs"]
pub(in crate::game) mod cauldrons_client;
#[cfg(feature = "client")]
#[path = "interface/chat_common_client.rs"]
pub(in crate::game) mod chat_common_client;
#[cfg(feature = "client")]
#[path = "items/combat_client.rs"]
pub(in crate::game) mod combat_client;
#[cfg(feature = "client")]
#[path = "items/consume_client.rs"]
pub(in crate::game) mod consume_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "runtime/core_tests_client.rs"]
pub(in crate::game) mod core_tests_client;
#[cfg(feature = "client")]
#[path = "interface/creative_screen_client.rs"]
pub(in crate::game) mod creative_screen_client;
#[cfg(feature = "client")]
#[path = "items/crossbow_client.rs"]
pub(in crate::game) mod crossbow_client;
#[cfg(feature = "client")]
#[path = "entities/cushions_client.rs"]
pub(in crate::game) mod cushions_client;
#[cfg(feature = "client")]
#[path = "interface/custom_arm_pose.rs"]
pub(in crate::game) mod custom_arm_pose;
#[cfg(feature = "client")]
#[path = "interface/debug_lines_client.rs"]
pub(in crate::game) mod debug_lines_client;
#[cfg(feature = "client")]
#[path = "interface/developer_launch_client.rs"]
pub(in crate::game) mod developer_launch_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/disabled_block_tests_client.rs"]
pub(in crate::game) mod disabled_block_tests_client;
#[cfg(feature = "client")]
#[path = "modding/entity_initialization_client.rs"]
pub(in crate::game) mod entity_initialization_client;
#[cfg(feature = "client")]
#[path = "modding/entity_sections_client.rs"]
pub(in crate::game) mod entity_sections_client;
#[path = "modding/entity_type_rows_client.rs"]
pub(in crate::game) mod entity_type_rows_client;
#[cfg(feature = "client")]
#[path = "entities/extinguish_client.rs"]
pub(in crate::game) mod extinguish_client;
#[cfg(feature = "client")]
#[path = "blocks/falling_blocks_client.rs"]
pub(in crate::game) mod falling_blocks_client;
#[cfg(feature = "client")]
#[path = "entities/frames_client.rs"]
pub(in crate::game) mod frames_client;
#[cfg(feature = "client")]
#[path = "interface/game_mode_common_client.rs"]
pub(in crate::game) mod game_mode_common_client;
#[cfg(feature = "client")]
#[path = "host/mod_client.rs"]
pub(in crate::game) mod host_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/initial_client_models_tests_client.rs"]
pub(in crate::game) mod initial_client_models_tests_client;
#[cfg(feature = "client")]
#[path = "interface/inventory_slots_client.rs"]
pub(in crate::game) mod inventory_slots_client;
#[cfg(feature = "client")]
#[path = "items/item_entities_client.rs"]
pub(in crate::game) mod item_entities_client;
#[cfg(feature = "client")]
#[path = "items/item_use_client.rs"]
pub(in crate::game) mod item_use_client;
#[cfg(feature = "client")]
#[path = "network/lan_actions/mod_client.rs"]
pub(in crate::game) mod lan_actions_client;
#[cfg(feature = "client")]
#[path = "network/lan/mod_client.rs"]
pub(in crate::game) mod lan_client;
#[cfg(feature = "client")]
#[path = "administration/lan_op_client.rs"]
pub(in crate::game) mod lan_op_client;
#[cfg(feature = "client")]
#[path = "network/lan_particles_client.rs"]
pub(in crate::game) mod lan_particles_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "network/lan_trade_decisions_tests_client.rs"]
pub(in crate::game) mod lan_trade_decisions_tests_client;
#[cfg(feature = "client")]
#[path = "network/lan_vehicles_client.rs"]
pub(in crate::game) mod lan_vehicles_client;
#[path = "runtime/launch_options_client.rs"]
pub(in crate::game) mod launch_options_client;
#[cfg(feature = "client")]
#[path = "entities/leads_client.rs"]
pub(in crate::game) mod leads_client;
#[cfg(feature = "client")]
#[path = "interface/level_sounds_client.rs"]
pub(in crate::game) mod level_sounds_client;
#[cfg(feature = "client")]
#[path = "world/light_emission_client.rs"]
pub(in crate::game) mod light_emission_client;
#[cfg(feature = "client")]
#[path = "items/mace_client.rs"]
pub(in crate::game) mod mace_client;
#[cfg(feature = "client")]
#[path = "blocks/maps/mod_client.rs"]
pub(in crate::game) mod maps_client;
#[path = "runtime/memory_common_client.rs"]
pub(in crate::game) mod memory_common_client;
#[cfg(feature = "client")]
#[path = "interface/menu_clicks_client.rs"]
pub(in crate::game) mod menu_clicks_client;
#[cfg(feature = "client")]
#[path = "entities/minecarts_client.rs"]
pub(in crate::game) mod minecarts_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/mod_container_openers_tests_client.rs"]
pub(in crate::game) mod mod_container_openers_tests_client;
#[path = "modding/mods/mod_client.rs"]
pub(in crate::game) mod mods_client;
#[cfg(feature = "client")]
#[path = "mobs/mounts_client.rs"]
pub(in crate::game) mod mounts_client;
#[cfg(feature = "client")]
#[path = "blocks/moving_pistons_client.rs"]
pub(in crate::game) mod moving_pistons_client;
#[cfg(feature = "client")]
#[path = "modding/native_biome_colors.rs"]
pub(in crate::game) mod native_biome_colors;
#[cfg(feature = "client")]
#[path = "modding/native_block_sounds_client.rs"]
pub(in crate::game) mod native_block_sounds_client;
#[cfg(feature = "client")]
#[path = "modding/native_colliders_client.rs"]
pub(in crate::game) mod native_colliders_client;
#[cfg(feature = "client")]
#[path = "modding/native_configs_client.rs"]
pub(in crate::game) mod native_configs_client;
#[cfg(feature = "client")]
#[path = "modding/native_entity_motion_client.rs"]
pub(in crate::game) mod native_entity_motion_client;
#[cfg(feature = "client")]
#[path = "modding/native_entity_refs_common_client.rs"]
pub(in crate::game) mod native_entity_refs_common_client;
#[cfg(feature = "client")]
#[path = "modding/native_entity_riding_client.rs"]
pub(in crate::game) mod native_entity_riding_client;
#[cfg(feature = "client")]
#[path = "modding/native_entity_state_client.rs"]
pub(in crate::game) mod native_entity_state_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_guest_turn_tests_client.rs"]
pub(in crate::game) mod native_guest_turn_tests_client;
#[cfg(feature = "client")]
#[path = "modding/native_harvest_queries_client.rs"]
pub(in crate::game) mod native_harvest_queries_client;
#[cfg(feature = "client")]
#[path = "modding/native_item_places_client.rs"]
pub(in crate::game) mod native_item_places_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_level_sound_except_tests_client.rs"]
pub(in crate::game) mod native_level_sound_except_tests_client;
#[cfg(feature = "client")]
#[path = "modding/native_lifecycle_client.rs"]
pub(in crate::game) mod native_lifecycle_client;
#[cfg(feature = "client")]
#[path = "modding/native_living_writes_client.rs"]
pub(in crate::game) mod native_living_writes_client;
#[cfg(feature = "client")]
#[path = "modding/native_mob_refs_client.rs"]
pub(in crate::game) mod native_mob_refs_client;
#[cfg(feature = "client")]
#[path = "modding/native_passages_client.rs"]
pub(in crate::game) mod native_passages_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_player_level_tests_client.rs"]
pub(in crate::game) mod native_player_level_tests_client;
#[cfg(feature = "client")]
#[path = "modding/native_scoreboard_client.rs"]
pub(in crate::game) mod native_scoreboard_client;
#[cfg(feature = "client")]
#[path = "modding/native_server_client.rs"]
pub(in crate::game) mod native_server_client;
#[cfg(feature = "client")]
#[path = "modding/native_state_shapes_client.rs"]
pub(in crate::game) mod native_state_shapes_client;
#[cfg(feature = "client")]
#[path = "modding/native_world_columns_client.rs"]
pub(in crate::game) mod native_world_columns_client;
#[cfg(feature = "client")]
#[path = "blocks/nether_portal_client.rs"]
pub(in crate::game) mod nether_portal_client;
#[cfg(feature = "client")]
#[path = "mobs/pandas_client.rs"]
pub(in crate::game) mod pandas_client;
#[cfg(feature = "client")]
#[path = "entities/passengers_client.rs"]
pub(in crate::game) mod passengers_client;
#[cfg(feature = "client")]
#[path = "entities/pick_ray_client.rs"]
pub(in crate::game) mod pick_ray_client;
#[cfg(feature = "client")]
#[path = "blocks/piston_carry_client.rs"]
pub(in crate::game) mod piston_carry_client;
#[cfg(feature = "client")]
#[path = "blocks/piston_push_client.rs"]
pub(in crate::game) mod piston_push_client;
#[cfg(feature = "client")]
#[path = "player_identity_client.rs"]
pub(in crate::game) mod player_identity_client;
#[cfg(feature = "client")]
#[path = "world/player_spawn_client.rs"]
pub(in crate::game) mod player_spawn_client;
#[cfg(feature = "client")]
#[path = "blocks/potent_sulfur_client.rs"]
pub(in crate::game) mod potent_sulfur_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "items/projectile_spawned_tests_client.rs"]
pub(in crate::game) mod projectile_spawned_tests_client;
#[cfg(feature = "client")]
#[path = "mobs/raids/mod_client.rs"]
pub(in crate::game) mod raids_client;
#[path = "network/remote/mod_client.rs"]
pub(in crate::game) mod remote_client;
#[path = "runtime/resource_load_client.rs"]
pub(in crate::game) mod resource_load_client;
#[path = "runtime/resource_tables_client.rs"]
pub(in crate::game) mod resource_tables_client;
#[cfg(feature = "client")]
#[path = "interface/scoreboard/mod_client.rs"]
pub(in crate::game) mod scoreboard_client;
#[cfg(feature = "client")]
#[path = "interface/screen_backgrounds_client.rs"]
pub(in crate::game) mod screen_backgrounds_client;
#[cfg(feature = "client")]
#[path = "mobs/sheep_dye_client.rs"]
pub(in crate::game) mod sheep_dye_client;
#[cfg(feature = "client")]
#[path = "items/stack_names_client.rs"]
pub(in crate::game) mod stack_names_client;
#[cfg(feature = "client")]
#[path = "modding/stacked_on_other_client.rs"]
pub(in crate::game) mod stacked_on_other_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/stacked_on_other_tests_client.rs"]
pub(in crate::game) mod stacked_on_other_tests_client;
#[cfg(feature = "client")]
#[path = "modding/state_resolver_client.rs"]
pub(in crate::game) mod state_resolver_client;
#[cfg(feature = "client")]
#[path = "world/tables_client.rs"]
pub(in crate::game) mod tables_client;
#[cfg(feature = "client")]
#[path = "administration/test_instances_client.rs"]
pub(in crate::game) mod test_instances_client;
#[cfg(feature = "client")]
#[path = "interface/toast_manager_client.rs"]
pub(in crate::game) mod toast_manager_client;
#[cfg(feature = "client")]
#[path = "items/trading_client.rs"]
pub(in crate::game) mod trading_client;
#[cfg(feature = "client")]
#[path = "items/use_hand_client.rs"]
pub(in crate::game) mod use_hand_client;
#[cfg(feature = "client")]
#[path = "blocks/vault_client.rs"]
pub(in crate::game) mod vault_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "entities/vehicle_removal_tests_client.rs"]
pub(in crate::game) mod vehicle_removal_tests_client;
#[cfg(feature = "client")]
#[path = "network/villager_client_client.rs"]
pub(in crate::game) mod villager_client_client;
#[cfg(feature = "client")]
#[path = "mobs/villagers_client.rs"]
pub(in crate::game) mod villagers_client;
#[cfg(feature = "client")]
#[path = "interface/wind_client.rs"]
pub(in crate::game) mod wind_client;
#[cfg(feature = "client")]
#[path = "world/world_entry_client.rs"]
pub(in crate::game) mod world_entry_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "blocks/world_fluids_tests_client.rs"]
pub(in crate::game) mod world_fluids_tests_client;
#[cfg(feature = "client")]
#[path = "world/world_ops_client.rs"]
pub(in crate::game) mod world_ops_client;

#[cfg(feature = "client")]
pub(in crate::game) struct ClientState {
    pub(in crate::game::client_side) darkness_blend: (f32, f32),
    /// `DebugEntryChunkGeneration`'s `lastPos` and `result`: F3's `Density` line is sampled
    /// again only when the feet block (or the dimension) changes.
    pub(in crate::game::client_side) debug_density: RefCell<debug_keys::DensityCache>,
    pub(in crate::game::client_side) pending_pack_reload: bool,
    /// HDR, Smooth Gradients, picked path and outcome at the last frame attempt, including a failed
    /// replacement (`follow_frame`).
    pub(in crate::game::client_side) frame_attempted: render_settings::FrameAttempt,
    /// When `resourcepacks/` was last looked at for the open pack screen, and what it held
    /// (`watch_pack_folder`).
    pub(in crate::game::client_side) pack_folder: Option<(
        std::time::Instant,
        Vec<(String, u64, Option<std::time::SystemTime>)>,
    )>,
    /// Its `LoadingOverlay`, up from the reload's start until it has faded
    /// out after it: no input reaches the game meanwhile.
    pub(in crate::game::client_side) reload_overlay: Option<packs::ReloadOverlay>,
    /// The link a chat click asks to open (`ConfirmLinkScreen`).
    pub(in crate::game) link_prompt: Option<String>,
    /// A server's Code of Conduct to accept (game/interface/code_of_conduct.rs).
    pub(in crate::game::client_side) code_of_conduct: Option<code_of_conduct::Prompt>,
    pub(in crate::game::client_side) world_renderer_ready: bool,
    pub(in crate::game::client_side) instance_groups:
        std::cell::RefCell<native_instance_groups::InstanceGroupStorage>,
    pub(in crate::game::client_side) standalone_geometry:
        native_block_entity_draw::StandaloneGeometry,
    /// `FogRenderer`'s evolving state.
    pub(in crate::game) fog: sunrise_client::fog::FogState,
    pub(in crate::game::client_side) screen_fx: sunrise_client::screen_effects::ScreenEffects,
    /// `LocalPlayer.itemActivation`: the totem flying up to the screen.
    pub(in crate::game::client_side) item_activation:
        sunrise_client::item_activation::ItemActivation,
    pub(in crate::game::client_side) weather_parameters: sunrise_client::weather::ParameterCache,
    pub(in crate::game::client_side) panorama_rotation: sunrise_client::gfx::panorama::Rotation,
    pub(in crate::game::client_side) lod_source_attempted: bool,
    pub(in crate::game::client_side) lod_observe: sunrise_client::fast_hash::FxHashSet<(i32, i32)>,
    /// The server level far terrain keeps the chunks of.
    pub(in crate::game::client_side) lod_level: Option<String>,
    /// The detail revision, centre and radius `Lod::update` last had covered for.
    pub(in crate::game::client_side) lod_covered_at: Option<(u64, (i32, i32), i32)>,
    /// Whether the view's columns have all arrived, and how new it is: large
    /// translucent pages wait for both before they shrink (`set_terrain_loading`).
    pub(in crate::game::client_side) view_arrival: sunrise_level::chunk_streamer::ViewArrival,
    pub(in crate::game::client_side) drop_spam_throttler: sunrise_client::menu::DropSpamThrottler,
    /// `yRotO` / `xRotO` of the local player: the view at the start of the tick,
    /// moved by `Entity.turn` and a boat's `clampRotation` as the view is
    /// (`LocalPlayer.getRopeHoldPosition` eases the held lead from it).
    pub(in crate::game::client_side) rope_rot_old: (f32, f32),
    pub(in crate::game::client_side) previous_eye: f32,
    /// A quick craft drag over inventory slots: (button, game slots so far).
    pub(in crate::game::client_side) inv_drag: Option<(i32, Vec<usize>)>,
    /// The mouse buttons whose press a container hook took: their release is the hooks' too.
    pub(in crate::game::client_side) container_buttons: HashSet<MouseButton>,
    /// This mouse event's button-up ended a container hook's press latch (`container_button_up`).
    pub(in crate::game::client_side) container_owned_up: bool,
    /// Physical mouse presses so far: a drag knows whether its press is still the latest.
    pub(in crate::game::client_side) physical_presses: u64,
    /// `AbstractContainerScreen`'s quick craft and skip-release state.
    pub(in crate::game::client_side) inv_clicks: sunrise_client::container_screen::Clicks,
    /// The Mouse Tweaks extra's drag and wheel (`tweaks`).
    pub(in crate::game::client_side) tweaks: extras::mouse_tweaks::Tweaks,
    /// The wheel and hovered slot of `BundleMouseActions`.
    pub(in crate::game::client_side) bundle_mouse: bundles::BundleMouse,
    /// The last inventory click, for the double click that gathers a stack.
    pub(in crate::game::client_side) last_inv_click:
        Option<(std::time::Instant, Option<usize>, bool)>,
    /// `lastQuickMoved`: what the last Shift click moved, which a Shift
    /// double click moves every stack of.
    pub(in crate::game::client_side) last_quick_moved: Option<ItemStack>,
    /// The other players' cracks (`LevelRenderer.destroyingBlocks`): breaker id to the block,
    /// stage (0..=9), last refresh time and stored additional display positions.
    pub(in crate::game::client_side) destroying: HashMap<i32, client_cracks::ClientCrack>,
    pub(in crate::game::client_side) mining_crack: Option<client_cracks::ClientCrack>,
    pub(in crate::game::client_side) keys: sunrise_client::held::HeldKeys,
    // Physical input is separate from remapped and toggled movement keys.
    pub(in crate::game) physical_keys: HashSet<KeyCode>,
    /// Keys whose first press a mod cancelled (`key_mapping_pre`): their repeats are never held.
    pub(in crate::game::client_side) rejected_keys: HashSet<KeyCode>,
    pub(in crate::game::client_side) physical_buttons: HashSet<MouseButton>,
    pub(in crate::game::client_side) drag_button: Option<MouseButton>,
    /// Control + left click as a right click (macOS).
    pub(in crate::game::client_side) right_click_emulation:
        sunrise_client_input::mouse::RightClickEmulation,
    pub(in crate::game) cursor: (f32, f32),
    /// The window pointer sample; frames and input project it at the current GUI size.
    pub(in crate::game::client_side) window_cursor: Option<sunrise_client::window::PointerSample>,
    pub(in crate::game::client_side) window_pointer_space:
        std::cell::OnceCell<sunrise_client::window::PointerSpace>,
    /// The cursor icon the window shows (`widgets::take_cursor` each frame).
    pub(in crate::game::client_side) cursor_icon: sunrise_client::widgets::Cursor,
    pub(in crate::game::client_side) captured: bool,
    /// Native grab mode used by winit. Linux window managers can drop the
    /// grab without a focus event, leaving `captured` true but stale.
    pub(in crate::game::client_side) capture_mode: Option<CursorGrabMode>,
    /// The window held the pointer when it lost focus: the system lets a
    /// grabbed pointer go with the focus, and a window that comes back still
    /// wanting it takes it again (`MouseHandler.grabMouse`, whose grab
    /// outlives the window's focus).
    pub(in crate::game::client_side) regrab_on_refocus: bool,
    /// Controller connection notices.
    pub(in crate::game::client_side) controller_notice:
        sunrise_client::controller_notice::NoticeSlot,
    pub(in crate::game) focused: bool,
    /// The visible window of a game an agent started, until a person clicks into it: its
    /// pointer grab stays inside the game as `--offscreen`'s does, so the person's pointer is
    /// never taken, hidden or moved, and their keys and mouse don't reach the game.
    pub(in crate::game::client_side) unclaimed_window: bool,
    /// The modifiers the person last held at it, applied at the claim (`held_modifiers`).
    pub(in crate::game::client_side) unclaimed_modifiers: Option<winit::event::Modifiers>,
    /// The window's last focus change during a scripted run, applied once the run ends.
    pub(in crate::game::client_side) focus_after_run: Option<bool>,
    /// Whether the window currently accepts input method events.
    pub(in crate::game::client_side) ime_allowed: bool,
    /// The character of the key held down, for the input method's echo of it.
    pub(in crate::game::client_side) held_key_text: ime::HeldKeyText,
    /// The candidate window's area last given to the window, in pixels.
    pub(in crate::game::client_side) ime_area: Option<[i32; 4]>,
    /// When the window lost focus, while it is still without
    /// (`Minecraft.lastActiveTime`: pausing waits 500 ms).
    pub(in crate::game::client_side) focus_lost: Option<Instant>,
    /// The keyboard's modifier state (`KeyEvent.modifiers`).
    pub(in crate::game::client_side) modifiers: winit::keyboard::ModifiersState,
    /// The window's fullscreen request and reported state.
    pub(in crate::game::client_side) fullscreen: sunrise_client::window::FullscreenState,
    /// MouseHandler's accumulated look and wheel state.
    pub(in crate::game::client_side) mouse: MouseLook,
    /// An agent moved the mouse (`mouse-move`) since the pointer was last
    /// grabbed or released: its motion turns the view like a grabbed pointer.
    pub(in crate::game::client_side) agent_pointer: bool,
    /// `Options.smoothCamera` (cinematic camera), not persisted.
    pub(in crate::game::client_side) smooth_camera: bool,
    /// Uniform `KeyState` (`isDown`, `clickCount`) for every vanilla mapping slot.
    pub(in crate::game) vanilla_mappings: sunrise_client::keymapping::VanillaMappings,
    /// A world screen with the pointer captured was open last frame.
    pub(in crate::game::client_side) was_playing: bool,
    pub(in crate::game::client_side) debug: bool,
    /// `sunrise-ctl gpu-passes`: time Sunrise's own passes on the GPU with F3 shut.
    pub(in crate::game::client_side) gpu_passes: bool,
    /// `sunrise-ctl gpu-passes detail`: also dispatch times and counters.
    pub(in crate::game::client_side) gpu_pass_detail: bool,
    pub(in crate::game) screen: Screen,
    /// What `window_resize` last reported: the physical size and the GUI settings behind the scaled size.
    pub(in crate::game::client_side) resize_state: Option<ResizeState>,
    /// Avoid an endless rebuild loop if the new device is lost as well.
    pub(in crate::game::client_side) last_device_recovery: Option<Instant>,
    /// CPU geometry storage retained across frames, including empty/title frames.
    pub(in crate::game::client_side) entity_vertices: Vec<Vertex>,
    /// Sunrise Extras' dynamic lights this frame (`dynamic_lights.rs`), and how brightly each
    /// item glows (built the first time the extra is on).
    pub(in crate::game::client_side) dynamic_lights: Vec<[f32; 4]>,
    pub(in crate::game::client_side) luminance:
        std::cell::OnceCell<sunrise_client::dynamic_light::Luminance>,
    /// Name tags drawn over everything (`RenderPipelines.TEXT_SEE_THROUGH`).
    pub(in crate::game::client_side) see_through_vertices: Vec<Vertex>,
    /// The first-person player's model, which only a native pack's shadow pass draws.
    pub(in crate::game::client_side) shadow_only_vertices: std::cell::RefCell<Vec<Vertex>>,
    /// Runs of `entity_vertices` by entity, for a shader pack's per-entity draws; recorded only
    /// while `pack_ids` is set (game/interface/shader_packs.rs).
    pub(in crate::game::client_side) entity_spans:
        std::cell::RefCell<Vec<sunrise_client::gfx::pack_draw::EntitySpan>>,
    /// The runs of `entity_geometry`'s ordered pass, kept to reuse their storage.
    pub(in crate::game::client_side) entity_runs: Vec<entity::DrawRun>,
    /// Which entity this frame's entity vertices belong to (`Frame::entity_motion`).
    pub(in crate::game::client_side) entity_motion: std::cell::RefCell<Vec<(u32, u64)>>,
    /// The GPU-posed vertex count at each of `entity_motion`'s marks (`Frame::entity_motion_posed`).
    pub(in crate::game::client_side) entity_motion_posed: std::cell::RefCell<Vec<u32>>,
    pub(in crate::game::client_side) skin_frame:
        std::cell::RefCell<sunrise_client::gfx::entity_skin::SkinFrame>,
    /// The glowing entities' silhouettes this frame (game/entities/glowing.rs).
    pub(in crate::game::client_side) glowing_vertices: std::cell::RefCell<Vec<Vertex>>,
    /// Client mods' sounds (`play_local_sound`): those waiting for the next audio update
    /// (`tick_sound_engine`), the stops and fades asked of them, and their handles.
    pub(in crate::game::client_side) mod_sounds: native_local_sounds::ModSounds,
    /// A client mod asked to clear the particles (`clear_particles`), done before
    /// the next frame or particle tick (`apply_mod_particle_clear`).
    pub(in crate::game::client_side) mod_clear_particles: std::cell::Cell<bool>,
    /// The shader pack's entity and block entity ids, while one draws entities.
    pub(in crate::game::client_side) pack_ids: Option<shader_packs::PackIds>,
    pub(in crate::game::client_side) started: Instant,
    /// `SpectatorGui`: the menu a spectator opens with the hotbar keys.
    pub(in crate::game::client_side) spectator_gui: sunrise_client::spectator_menu::SpectatorGui,
    /// FramerateLimitTracker.latestInputTime.
    pub(in crate::game::client_side) last_input: Instant,
    pub(in crate::game::client_side) frames: u64,
    /// The glint clock and options, read once per frame (game/interface/item_draw.rs).
    pub(in crate::game::client_side) glint_frame: sunrise_client::item_render::GlintFrame,
    /// Mod block entities' kept geometry (game/modding/native_block_entity_draw.rs).
    pub(in crate::game::client_side) kept_block_entities:
        std::cell::RefCell<native_block_entity_draw::KeptGeometry>,
    /// Client mods' dynamic textures in the GUI atlas's pool (game/modding/native_dynamic_textures.rs).
    pub(in crate::game::client_side) dynamic_textures:
        sunrise_client::hud::dynamic_textures::DynamicTextures,
    /// Glyphs from `native_draw_text` (`&self`): uploaded to the font atlas with the frame.
    pub(in crate::game::client_side) text_patches:
        std::cell::RefCell<Vec<sunrise_client::font::Patch>>,
    /// Prepared block-entity copies for this frame, in entity-stream order.
    pub(in crate::game::client_side) prepared_model_draws:
        std::cell::RefCell<Vec<sunrise_client::gfx::PreparedModelDraw>>,
    /// Scratch for the copies a frame draws through the terrain materials.
    pub(in crate::game::client_side) model_copy_order: Vec<u32>,
    pub(in crate::game::client_side) world_geometry:
        Option<sunrise_client::world_geometry::WorldGeometry>,
    /// `DebugRenderer.lastDebugEntriesVersion`: none before the first view of a world.
    pub(in crate::game::client_side) debug_renderers_version: Option<u64>,
    /// Next undrawn mod layer in HUD `gui_layers::order`; `None` while hidden or outside its pass.
    pub(in crate::game::client_side) hud_layer_next: Option<usize>,
    /// The mod block entities' view distances, asked of the parts once (`native_block_entity_draw`).
    pub(in crate::game::client_side) mod_view_distances:
        std::cell::OnceCell<native_block_entity_draw::ViewDistances>,
    /// What drawing a block entity asks of its block, by state id (`block_entity_reach`):
    /// resolved from the block names once, not for every block entity in every frame.
    pub(in crate::game::client_side) block_entity_reach: std::cell::OnceCell<Box<[(Reach, bool)]>>,
    /// `item_level` for one frame, game time, day time and dimension.
    pub(in crate::game::client_side) item_level_cache: std::cell::RefCell<
        Option<(
            (u64, i64, i64, &'static str),
            sunrise_assets::item_model::LevelState,
        )>,
    >,
    /// Frames drawn with the player in the world, for `--world-frames`.
    pub(in crate::game::client_side) world_frames: u64,
    pub(in crate::game::client_side) fps: f32,
    pub(in crate::game::client_side) fps_frames: u64,
    pub(in crate::game::client_side) fps_at: Instant,
    /// `Hud.tickCount`, advanced on each unpaused client tick, including in menus.
    pub(in crate::game::client_side) gui_ticks: i32,
    /// The sprite animation clock (`TextureManager.tick`): counts only the ticks the level
    /// runs normally, so `/tick freeze` holds it and `/tick step` moves it on.
    pub(in crate::game::client_side) texture_ticks: u64,
    pub(in crate::game::client_side) health_hud: hud_layout::HealthHud,
    pub(in crate::game) message: Option<(String, Instant)>,
    pub(in crate::game::client_side) music: sunrise_client::music::MusicManager,
    /// The tracks native mods play over the music (`native_music_layers`).
    pub(in crate::game::client_side) music_layers: sunrise_client::music::MusicLayers,
    /// `ToastManager.nowPlayingToast`: there while Music Toast is not Never.
    pub(in crate::game::client_side) now_playing: Option<sunrise_client::toasts::NowPlayingToast>,
    /// The Music Toast option the toast was last set up for.
    pub(in crate::game::client_side) music_toast_state: u8,
    pub(in crate::game::client_side) music_notes: sunrise_client::toasts::MusicNotes,
    pub(in crate::game::client_side) ambience: sunrise_client::ambience::Ambience,
    /// The sound engine was told the game paused (pauseAllExcept).
    pub(in crate::game::client_side) sound_paused: bool,
    /// Minecarts whose `MinecartSoundInstance` is playing.
    pub(in crate::game::client_side) cart_sounds: HashSet<u32>,
    /// The minecart the riding sounds were started for (`LocalPlayer.startRiding`).
    pub(in crate::game::client_side) riding_cart_sounds: Option<u32>,
    /// The happy ghast its riding sound was started for (`LocalPlayer.startRiding`).
    pub(in crate::game::client_side) riding_sound: Option<u32>,
    /// `LocalPlayer.wasFallFlying` for the elytra's flying sound.
    pub(in crate::game::client_side) was_fall_flying: bool,
    /// Bees whose buzz is playing, and whether it is the aggressive one.
    pub(in crate::game::client_side) bee_sounds: HashMap<u32, bool>,
    /// Songs playing from jukeboxes (tracked sound handles).
    pub(in crate::game::client_side) jukebox_sounds: HashMap<(i32, i32, i32), u64>,
    /// Remote guardian eye targets reused between ticks.
    pub(in crate::game::client_side) guardian_look_scratch: Vec<(usize, [f64; 3])>,
    pub(in crate::game::client_side) explicit_particle_quads:
        sunrise_client::particles::ParticleQuads,
    /// Sign text being edited (`SignEditScreen`).
    pub(in crate::game) sign_edit: Option<sign_edit::SignEditor>,
    /// An open book, book and quill or lectern screen.
    pub(in crate::game) book: Option<books::BookScreen>,
    /// The last click on the book screen, for double clicks.
    pub(in crate::game::client_side) book_last_click: Option<std::time::Instant>,
    pub(in crate::game::client_side) station_ui: Option<stations_client::StationUi>,
    /// LevelLoadingScreen.smoothedProgress while the player is not placed yet.
    pub(in crate::game::client_side) loading_progress: f32,
    /// When `loading_progress` last eased (the server thread's load, 50 ms a step).
    pub(in crate::game::client_side) loading_progress_at: Option<std::time::Instant>,
    /// A received packet started or ended a piston's move: `poll_remote`
    /// syncs the moving pistons once its packets are handled, not once per packet.
    pub(in crate::game::client_side) moving_pistons_stale: bool,
    /// Camera.oldFovModifier / fovModifier.
    /// `Camera` (eye height and field-of-view easing, view placement).
    pub(in crate::game::client_side) view: sunrise_client::camera::Camera,
    /// `FramerateLimiter` and its clock.
    pub(in crate::game::client_side) limiter: sunrise_client::window::FramerateLimiter,
    pub(in crate::game::client_side) frame_clock: sunrise_client::window::SystemClock,
    /// Hud.scopeScale (spyglass overlay size).
    pub(in crate::game::client_side) scope_scale: f32,
    /// Duration of the last frame in ticks (GUI easing).
    pub(in crate::game::client_side) frame_ticks: f32,
    /// `ItemStack.popTime` of the nine hotbar stacks, then the off hand: set to 5
    /// when a slot gains items, counted down each tick.
    pub(in crate::game::client_side) hotbar_pop: [u8; 10],
    pub(in crate::game::client_side) titles: titles::Titles,
    /// `Hud.toolHighlightTimer`: the selected item name.
    pub(in crate::game::client_side) tool_highlight: sunrise_client::hud_overlays::ToolHighlight,
    /// `Hud.autosaveIndicatorValue`.
    pub(in crate::game::client_side) saving_indicator:
        sunrise_client::hud_overlays::SavingIndicator,
    /// The first-person equip heights and the items the hands show (with
    /// the slot each came from).
    pub(in crate::game::client_side) heights: sunrise_client::first_person::HandHeights,
    /// The hands' lag behind the view (`LocalPlayer.xBob`, `yBob`); a new
    /// LocalPlayer (joining, respawning, changing dimension) starts at 0.
    pub(in crate::game::client_side) sway: sunrise_client::first_person::HandSway,
    pub(in crate::game::client_side) visible_hands: [(usize, Option<ItemStack>); 2],
    /// Whether the main and off hand's slots were set to a new stack that
    /// the hand doesn't show yet (`Game::hand_stack_replaced`).
    pub(in crate::game::client_side) hands_replaced: [bool; 2],
    pub(in crate::game::client_side) hand_strength: sunrise_client::attack_strength::AttackStrength,
    pub(in crate::game::client_side) look: look::LookCache,
    pub(in crate::game::client_side) tutorial: tutorial::Tutorial,
    pub(in crate::game) toast_manager: toast_manager::ToastManager,
    /// The open screen's narration (`Screen`'s narration state and timing).
    pub(in crate::game::client_side) narration: sunrise_client::screen_narrator::ScreenNarrator,
    /// Where the options screen returns: true = title, false = pause menu.
    pub(in crate::game::client_side) options_from_title: bool,
    /// `ClientAvatarState`: walked distance and view bob.
    pub(in crate::game::client_side) avatar: sunrise_client::camera::AvatarBob,
    /// OptiFine custom sky layers' eased brightness (`custom_sky.rs`).
    pub(in crate::game::client_side) custom_sky: custom_sky::CustomSkyView,
    /// `ClientLevel.rainSoundTime`: ticks of rain since the last rain sound.
    pub(in crate::game::client_side) rain_sound_time: i32,
    /// Bolts that struck here and sky flashes they made, in total (`status`).
    pub(in crate::game::client_side) lightning_seen: [u32; 2],
    /// The open WinScreen (game/interface/win.rs).
    pub(in crate::game::client_side) win: Option<win::Win>,
    pub(in crate::game::client_side) demo_intro: Option<demo_intro::DemoIntro>,
    /// Falling models by state and selected-model address list; cleared on pack/atlas replacement.
    pub(in crate::game::client_side) falling_vertices:
        RefCell<HashMap<BlockStateId, Vec<(Vec<usize>, Vec<Vertex>)>>>,
    /// Blocks the server shows for a moment (`ClientboundAddTransientBlockPacket`).
    pub(in crate::game::client_side) transient_blocks: transient_blocks::TransientBlocks,
    pub(in crate::game::client_side) frame_model: frames::FrameModel,
    pub(in crate::game::client_side) glow_frame_model: frames::FrameModel,
    /// The `map=true` frame variants.
    pub(in crate::game::client_side) map_frame_model: frames::FrameModel,
    pub(in crate::game::client_side) glow_map_frame_model: frames::FrameModel,
    /// `WaypointStyleManager`.
    pub(in crate::game::client_side) waypoint_styles: sunrise_client::locator_bar::WaypointStyles,
    /// `ClientWaypointManager`: the locator bar's waypoints.
    pub(in crate::game::client_side) waypoints: sunrise_client::locator_bar::ClientWaypoints,
    /// The camera the last frame drew from (position, yaw, pitch, fov), for the locator bar.
    pub(in crate::game::client_side) locator_camera: Option<([f64; 3], f32, f32, f32)>,
    pub(in crate::game::client_side) native_camera_frame: Option<mod_api::Camera>,
    /// The yaw and pitch a native mod's camera drew the last frame from (`native_camera_pose`),
    /// none when it drew from vanilla's.
    pub(in crate::game::client_side) posed_rotation: Option<(f32, f32)>,
    /// `LocalPlayer.postEffects`: the chains the server has the screen run
    /// (`ClientboundPostEffectsPacket`), in order.
    pub(in crate::game::client_side) post_effects: Vec<String>,
    /// Reused per-frame `requestedPostEffects` storage.
    pub(in crate::game::client_side) requested_post_effects_buf: Vec<String>,
    /// The game time (ticks and partial) of the last frame, for `PolyDeltaTime`.
    pub(in crate::game::client_side) polytone_post_ticks: f64,
    /// The last expression context built (what the packs' expressions read), kept across frames
    /// so its strings are rewritten in place instead of allocated each frame.
    pub(in crate::game::client_side) expression_frame: sunrise_client::environment::Frame,
    /// `GameRenderer.spectatedEntityPostEffect` (`checkEntityPostEffect`).
    pub(in crate::game) spectated_post_effect: Option<&'static str>,
    /// `GameRenderer.spectatedEntityEffectActive` (`toggleSpectatorPostEffect`).
    pub(in crate::game::client_side) spectated_post_effect_active: bool,
    /// `LocalPlayer.experienceDisplayStartTick`: (last progress, seen a change, tick).
    pub(in crate::game::client_side) experience_display: (f32, bool, u64),
    pub(in crate::game::client_side) boat_models: boats::BoatModels,
    /// EndCrystalModel's baked layer.
    pub(in crate::game::client_side) crystal_model: Option<sunrise_client::mobs::Part>,
    /// CushionModel's baked layer.
    pub(in crate::game::client_side) cushion_model: Option<sunrise_client::mobs::Part>,
    /// Models drawn only once such an entity shows up (lead knots, wind charges...).
    pub(in crate::game::client_side) entity_models: EntityModels,
    pub(in crate::game::client_side) dragon_model: Option<sunrise_client::mobs::Part>,
    /// Map textures waiting for the next HUD batch to upload them.
    pub(in crate::game::client_side) map_patches: Vec<sunrise_client::font::Patch>,
    pub(in crate::game::client_side) name_tag_layouts: names::NameTagLayouts,
    pub(in crate::game::client_side) see_through_collector: text_displays::SeeThroughCollector,
    pub(in crate::game::client_side) translucent_display_collector:
        text_displays::TranslucentDisplayCollector,
    pub(in crate::game::client_side) display_visible_scratch: Vec<(i32, f64)>,
    pub(in crate::game::client_side) block_display_parts_scratch: Vec<u32>,
    pub(in crate::game::client_side) display_inline_profiles_scratch:
        Vec<std::sync::Arc<serde_json::Value>>,
    pub(in crate::game::client_side) interactions:
        std::collections::HashMap<i32, interactions::RemoteInteraction>,
    /// The fence knots a server tracks to this client, by entity id, at their
    /// block (`LeashFenceKnotEntity`).
    pub(in crate::game::client_side) remote_knots: HashMap<u32, [i32; 3]>,
    pub(in crate::game::client_side) text_display_vertices: usize,
    pub(in crate::game::client_side) skin_slots: sunrise_client::skins::SkinSlots,
    pub(in crate::game::client_side) skin_profiles: RefCell<sunrise_client::skins::Resolver>,
    pub(in crate::game::client_side) chat_ui: chat::ChatUiState,
    pub(in crate::game::client_side) map_slots: super::atlas_slots::AtlasSlots<i32>,
    /// Runtime trim slots and the last completed preparation for this atlas.
    pub(in crate::game::client_side) trim_slots: sunrise_client::trims::TrimSlots,
    pub(in crate::game::client_side) cart_models: minecarts::CartModels,
    /// The horse inventory (game/interface/mount_menu.rs).
    pub(in crate::game::client_side) mount_menu: mount_menu::MountMenu,
    /// The other players' hooks a server tracks to this client.
    pub(in crate::game::client_side) remote_hooks: Vec<fishing::RemoteHook>,
    pub(in crate::game::client_side) remote_clouds:
        Vec<sunrise_client_gameplay::remote::projectiles::RemoteCloud>,
    pub(in crate::game::client_side) tick_hit: std::cell::Cell<Option<Option<Hit>>>,
    pub(in crate::game::client_side) tick_mob: Option<Option<(usize, f64, [f64; 3])>>,
    pub(in crate::game::client_side) predicted_equipment_modifiers: Vec<(String, String)>,
    pub(in crate::game::client_side) trident_model: Option<sunrise_client::mobs::Part>,
    pub(in crate::game::client_side) arrow_model: Option<sunrise_client::mobs::Part>,
    pub(in crate::game::client_side) bee_stinger_model: Option<sunrise_client::mobs::Part>,
    /// The creative search projection whose stacks the mods' `item_name` parts last named.
    pub(in crate::game::client_side) creative_search_named: Option<u64>,
    /// `CreativeModeInventoryScreen.ignoreTextInput`: the chat key opened the
    /// search tab, so the letter it carries is not typed into the box.
    pub(in crate::game::client_side) creative_ignore_text: bool,
    /// The Agent Control setting the socket was last opened or closed for (`agent_settings`).
    pub(in crate::game::client_side) agent_control_on: bool,
    /// When an agent's last command came.
    pub(in crate::game::client_side) agent_last: Option<Instant>,
    pub(in crate::game::client_side) frame_stats: sunrise_client::control::FrameStats,
    /// This thread's and all threads' allocation counts at the last rendered frame.
    pub(in crate::game::client_side) alloc_mark: Option<(
        sunrise_threads::alloc_count::Counts,
        sunrise_threads::alloc_count::Counts,
    )>,
    /// Settings and HUD state: `game/interface/extras.rs`.
    pub(in crate::game::client_side) extras: extras::ExtrasState,
    /// The PvP & HUD options' state (`game/entities/pvp.rs`).
    pub(in crate::game::client_side) pvp: pvp::PvpState,
    /// The item panel beside container screens (`game/modding/native_panel.rs`).
    pub(in crate::game::client_side) panel: native_panel::Panel,
    /// Sunrise's own shader pack, while one is picked (`game/modding/native_shaders.rs`).
    pub(in crate::game::client_side) native_shaders: native_shaders::NativeShaders,
    /// Native client mods' key mappings (game/modding/native_key_mappings.rs).
    pub(in crate::game::client_side) mod_keys: sunrise_client::keymapping::ModKeys,
    /// The mod whose own screen is open (`Screen::Mod`).
    pub(in crate::game::client_side) native_mod_screen: Option<String>,
    /// The camera section (column, height) `pending_uploads` is sorted for, farthest first:
    /// while it stays, the list needs only its new meshes merged in, not a sort every frame.
    pub(in crate::game::client_side) pending_uploads_order: Option<((i32, i32), i32)>,
    pub(in crate::game::client_side) upload_pace: upload_pace::UploadPace,
    /// This frame's view frustum, for culling entities and block entities.
    pub(in crate::game::client_side) view_frustum: Option<sunrise_client::gfx::JoinedFrusta>,
    /// `SUNRISE_TICK_SCRIPT`'s scenario (game/world/tick_script.rs).
    pub(in crate::game::client_side) tick_script: tick_script::TickScript,
    /// The last water plane `water_plane` found, kept while none is in reach.
    pub(in crate::game::client_side) water_plane_memo: std::cell::Cell<Option<f32>>,
    /// The camera's block and the second `water_plane` last scanned at.
    pub(in crate::game::client_side) water_plane_scan: std::cell::Cell<Option<([i32; 3], i64)>>,
    /// The server this window connected to last over the network: kept while something may
    /// connect there again (game/network/remote/connection.rs).
    pub(in crate::game::client_side) last_connection: Option<sunrise_hooks::connection::Server>,
    /// A connection over the network is open: started, not yet closed (one `Hooks::left` or
    /// `Hooks::ended` each).
    pub(in crate::game::client_side) connection_open: bool,
    /// The crates' answer to the last connection's end, until the Disconnected screen shows it.
    pub(in crate::game::client_side) pending_retry: Option<sunrise_hooks::connection::Retry>,
    /// A connection the Disconnected screen makes on its next frame, as that answer asked.
    pub(in crate::game::client_side) retry_due: Option<sunrise_hooks::connection::Server>,
    /// What the window's title last showed: its session part and the language (window_title.rs).
    pub(in crate::game::client_side) window_title: Option<(Option<&'static str>, u64)>,
    pub(in crate::game::client_side) studio: studio::Studio,
    /// The Social Interactions screen and the players whose chat is hidden (game/network/social.rs).
    pub(in crate::game::client_side) social: social::Social,
    /// Chat Settings opened from RestrictionsScreen: Done returns to World Options.
    pub(in crate::game::client_side) options_to_world_options: bool,
    /// `GameTestBlockHighlightRenderer.markers`: block, text, removal time.
    pub(in crate::game::client_side) test_highlights:
        HashMap<(i32, i32, i32), (String, std::time::Instant)>,
    /// KeyboardHandler's debug key state (game/interface/debug_keys.rs), the physical
    /// keys held, and the debug renderers F3+B and F3+G switch on.
    pub(in crate::game::client_side) debug_keys: sunrise_client::debug_keys::DebugKeys,
    /// The F3 overlay's charts (F3+2, F3+3).
    pub(in crate::game::client_side) debug_charts: sunrise_client::debug_chart::DebugCharts,
    /// `DebugOptionsScreen` while it is open (game/interface/debug_options.rs).
    pub(in crate::game::client_side) debug_options: Option<debug_options::DebugOptionsUi>,
    /// `DebugEntryMemory`'s readings (game/interface/debug_lines.rs).
    pub(in crate::game::client_side) memory_meter:
        std::cell::RefCell<sunrise_client::debug_overlay::MemoryMeter>,
    /// The F3 overlay's vertices between rebuilds (`draw_debug_overlay_under`).
    pub(in crate::game::client_side) debug_paint:
        RefCell<sunrise_client::debug_overlay::PaintCache>,
    /// The F3 overlay's last reading of the refresh rate, and when it was taken.
    pub(in crate::game::client_side) debug_refresh: std::cell::Cell<Option<(Instant, Option<u32>)>>,
    /// `Minecraft.gpuUtilization`: the last timed frame's GPU time over its
    /// CPU time, in percent.
    pub(in crate::game::client_side) gpu_utilization: f64,
    /// GPU Culling's Auto mode: what it has decided from the frames' CPU and GPU times.
    pub(in crate::game::client_side) gpu_culling_auto: sunrise_client::autotune::GpuCullingAuto,
    /// Dynamic Resolution: the scale it has the world at (`frame_autotune`).
    pub(in crate::game::client_side) dynamic_resolution:
        sunrise_client::dynamic_resolution::DynamicResolution,
    /// The mobs `mob_run_geometry` drew last frame (`lastEntityRenderStateCount`).
    pub(in crate::game::client_side) rendered_mobs: std::cell::Cell<usize>,
    /// Whether a native mod requested background blur for this frame (`apply_screen_blur`).
    pub(in crate::game::client_side) screen_blur_requested: std::cell::Cell<bool>,
    /// PauseScreen.showPauseMenu (false after F3+Escape).
    pub(in crate::game::client_side) pause_menu: bool,
    pub(in crate::game::client_side) pause_focus: Option<u8>,
    pub(in crate::game::client_side) death_focus: Option<u8>,
    /// The screen ([`Game::key_screen`]) the menu focus belongs to. Another
    /// screen in front drops it, as `Gui.setScreen` drops the focus of the
    /// screen it leaves (`Screen.clearFocus`).
    pub(in crate::game::client_side) focus_screen: u64,
    pub(in crate::game::client_side) gamemode_switcher:
        Option<sunrise_client::game_mode_switcher::GameModeSwitcher>,
    /// The game is handling a key, mouse or typed-text event, from the window or the
    /// control socket: the only time a mod reaches the clipboard (`with_user_input`).
    pub(in crate::game::client_side) user_input: std::cell::Cell<bool>,
    /// Select All in the append-only text fields.
    pub(in crate::game::client_side) text_selection: sunrise_client::clipboard::AppendSelection,
    /// The client loop consumes a mod's quit request outside the calling hook.
    pub(in crate::game::client_side) client_quit: std::cell::Cell<saving::ClientQuit>,
    /// The world was saved for the application ending (`save_for_exit`).
    pub(in crate::game) saved_for_exit: bool,
    /// The last world frame reached the swap chain. A window the compositor
    /// stops showing gets no frames, and the event loop must not spin on it.
    pub(in crate::game::client_side) frame_drawn: bool,
    /// The world list's icon in progress (`GameRenderer.tryTakeScreenshotIfNeeded`):
    /// `None` without a local world, or once the save has its icon.png.
    pub(in crate::game::client_side) world_icon: Option<WorldIcon>,
    pub(in crate::game::client_side) client_pick: native_hooks::ClientPick,
    /// `Hud.bossOverlay`: the bars this window is showing.
    pub(in crate::game::client_side) boss_overlay: sunrise_client::boss_overlay::BossOverlay,
    /// `WorldBorderRenderer` and its render state.
    pub(in crate::game::client_side) border_renderer: sunrise_client::border_render::BorderRenderer,
    /// The camera's 16-block section when the wall was last invalidated.
    pub(in crate::game::client_side) border_section: Option<[i32; 3]>,
    pub(in crate::game::client_side) border_state: sunrise_client::border_render::BorderRenderState,
    pub(in crate::game::client_side) subtitles: subtitle_hud::SubtitleState,
    pub(in crate::game::client_side) stats_ui: stats_screen::StatsUi,
    /// The Server Links screens' state (game/network/server_links.rs).
    pub(in crate::game::client_side) links_ui: server_links::LinksUi,
    pub(in crate::game::client_side) camera_type: sunrise_client::camera::CameraType,
    pub(in crate::game::client_side) requested_camera_type:
        std::cell::Cell<Option<sunrise_client::camera::CameraType>>,
    /// The local player as a model (third person, the inventory).
    pub(in crate::game::client_side) player_model: Option<Mob>,

    pub(in crate::game::client_side) pending_entry: ClientOnly<Option<PendingWorldEntry>>,
    /// A pack reload staged on a worker until `drive_pack_reload` applies it.
    pub(in crate::game::client_side) pack_reload: ClientOnly<Option<packs::PackReload>>,
    /// A server's resource packs (game/world/server_packs.rs).
    pub(in crate::game::client_side) server_packs: ClientOnly<server_packs::ServerPackState>,
    /// The window's inbox for server notices, kept across world disconnects.
    pub(in crate::game::client_side) client_reports:
        ClientOnly<Arc<sunrise_client::chunk_io::Reports>>,
    /// A command the player didn't type, waiting for their answer
    /// (game/network/remote_chat.rs).
    pub(in crate::game::client_side) command_prompt: ClientOnly<Option<remote_chat::CommandPrompt>>,
    pub(in crate::game::client_side) dialogs: ClientOnly<dialogs::DialogState>,
    /// This game's resource packs (`reload_packs` replaces them).
    pub(in crate::game::client_side) pending_initial_models:
        ClientOnly<Option<Box<initial_client_models::PendingClientStart>>>,
    pub(in crate::game::client_side) stitching_block_atlas: ClientOnly<Option<Arc<RenderAssets>>>,
    pub(in crate::game::client_side) lod: ClientOnly<Option<sunrise_client::lod::Lod>>,
    /// The server's BlueMap far terrain draws from, when the player named one.
    pub(in crate::game::client_side) lod_blue_map:
        ClientOnly<Option<Arc<sunrise_client::bluemap::BlueMap>>>,
    /// The monitor whose video modes `settings.video_modes` holds.
    pub(in crate::game::client_side) video_modes: ClientOnly<sunrise_client::window::VideoModeList>,
    pub(in crate::game::client_side) screen_state: ClientOnly<screen_build::ScreenState>,
    pub(in crate::game::client_side) window: ClientShared<Option<Arc<Window>>>,
    pub(in crate::game::client_side) renderer: ClientShared<Option<Box<Renderer>>>,
    /// Mob models posed on the GPU (`Frame::skin`, `mobs::skin`): the models stored there,
    /// and this frame's draws.
    pub(in crate::game::client_side) skin_models:
        ClientOnly<std::cell::RefCell<sunrise_client::mobs::skin::SkinModels>>,
    /// The level random a block part's `animate_tick` holds as `event.random`, while it runs:
    /// the game's own draws during the call go through it (`with_lent_level_random`).
    pub(in crate::game::client_side) lent_level_random:
        ClientOnly<Option<std::rc::Rc<std::cell::RefCell<mod_api::LegacyRandomSource>>>>,
    /// Controllers (Sunrise's own).
    pub(in crate::game::client_side) pad: ClientShared<pad::Pads>,
    /// Mod screens' living scenes, per mod and clock id (game/modding/native_scene_live.rs).
    pub(in crate::game::client_side) live_scenes:
        ClientOnly<std::cell::RefCell<native_scene_live::LiveScenes>>,
    /// `DebugRenderer.renderers`: active mod renderers and their owners, resolved at refresh.
    pub(in crate::game::client_side) debug_renderers:
        ClientOnly<Vec<sunrise_client::debug_entries::ActiveDebugRenderer>>,
    pub(in crate::game::client_side) audio: ClientShared<Audio>,
    pub(in crate::game::client_side) particles:
        ClientShared<sunrise_client::particles::ParticleEngine>,
    /// `ItemPickupParticle`s: taken items, orbs and arrows flying to their collector.
    pub(in crate::game::client_side) pickups: ClientOnly<pickups::Pickups>,
    /// `CreateWorldScreen.openCreateWorldScreen`'s data read, on a worker,
    /// while the title says `createWorld.preparing`.
    pub(in crate::game::client_side) creation_job:
        ClientOnly<Option<std::thread::JoinHandle<std::result::Result<(), String>>>>,
    pub(in crate::game::client_side) title: ClientShared<Option<Title>>,
    pub(in crate::game::client_side) armor_stand_assets: ClientOnly<armor_stands::ArmorStandAssets>,
    pub(in crate::game::client_side) text_displays:
        ClientOnly<indexmap::IndexMap<i32, text_displays::RemoteTextDisplay>>,
    /// Player skin downloads, and which skin each atlas skin slot holds
    /// (least recently shown first).
    pub(in crate::game::client_side) skin_downloads:
        ClientOnly<RefCell<sunrise_client::skins::Downloader>>,
    /// The recipe book of the open screen (game/items/recipe_book.rs).
    pub(in crate::game::client_side) recipe_ui: ClientOnly<recipe_book::RecipeUi>,
    /// Monster spawners' display mobs (`game/mobs/spawner.rs`).
    pub(in crate::game::client_side) spawners: ClientOnly<spawner::SpawnerState>,
    /// The mod screen of a menu open on this client (`native_screen`).
    pub(in crate::game::client_side) mod_screen: ClientOnly<Option<native_hooks::OpenNativeScreen>>,
    pub(in crate::game::client_side) creative_screen: ClientOnly<Creative>,
    /// Control replies waiting for a rendered frame or a benchmark deadline.
    pub(in crate::game::client_side) pending_screenshot:
        ClientOnly<Option<std::sync::mpsc::Sender<String>>>,
    /// `screenshot-sequence` while it runs.
    pub(in crate::game::client_side) screenshot_sequence:
        ClientOnly<Option<screenshot_sequence::Sequence>>,
    /// `render-capture DIR`: replied once the renderer wrote the scene.
    pub(in crate::game::client_side) pending_render_capture:
        ClientOnly<Option<std::sync::mpsc::Sender<String>>>,
    pub(in crate::game::client_side) pending_bench:
        ClientOnly<Option<(Instant, std::sync::mpsc::Sender<String>, bool)>>,
    /// The in-game browser's worker, separate from the cloneable reload inputs.
    pub(in crate::game::client_side) mod_jobs: ClientOnly<mod_browser::Jobs>,
    /// Main-hand input captured by a client mod, independent of living-item use.
    pub(in crate::game::client_side) client_item_use:
        ClientShared<Option<native_claimed_items::ActiveClientItemUse>>,
    /// Meshed sections waiting for an upload budget (Sunrise settings).
    pub(in crate::game::client_side) pending_uploads:
        ClientOnly<Vec<sunrise_client::streaming::MeshUpload>>,
    /// Empty between updates. Reuses storage for merging two sorted runs.
    pub(in crate::game::client_side) pending_uploads_merge:
        ClientOnly<Vec<sunrise_client::streaming::MeshUpload>>,
    pub(in crate::game::client_side) occlusion: ClientOnly<entity_culling::EntityOcclusion>,
    pub(in crate::game::client_side) async_particle_culler:
        ClientOnly<RefCell<particle_culling::ParticleAsyncState>>,
    /// A scripted flight from the control socket (`fly`).
    pub(in crate::game::client_side) flight: ClientOnly<Option<control::Flight>>,
    /// Widget commands waiting for the frame after a click (game/runtime/ui_control.rs).
    pub(in crate::game::client_side) ui_wait: ClientOnly<ui_control::UiWait>,
    /// Who caused each input event: a person, an agent or the game (game/runtime/input_trace.rs).
    pub(in crate::game::client_side) input_trace: ClientOnly<input_trace::InputTrace>,
    /// A repro recording, export or replay (game/runtime/repro.rs).
    pub(in crate::game::client_side) repro: ClientOnly<repro::Repro>,
    /// Client work survives a disconnect; packets and credits stay with their physical listener.
    pub(in crate::game::client_side) client_tasks: ClientOnly<sunrise_client::net::ClientTasks>,
    pub(in crate::game::client_side) pending_client_tasks:
        ClientOnly<std::collections::VecDeque<sunrise_client::net::ClientTask>>,
    pub(in crate::game::client_side) world_options: ClientOnly<Option<lan_screen::WorldOptions>>,
    pub(in crate::game::client_side) command_block_screen:
        ClientOnly<Option<command_blocks::CommandBlockScreen>>,
    pub(in crate::game::client_side) block_editor:
        ClientOnly<Option<block_editor::BlockEditScreen>>,
    /// Startup declarations shared by F3 options, the overlay and renderer selection.
    pub(in crate::game::client_side) registered_debug_entries:
        ClientOnly<std::sync::Arc<[sunrise_client::debug_entries::RegisteredDebugEntry]>>,
    /// `Minecraft.debugEntries`: what the overlay and the debug renderers
    /// show (`debug-profile.json`, F3+F6).
    pub(in crate::game::client_side) debug_entries:
        ClientOnly<sunrise_client::debug_entries::DebugEntries>,
    /// The system clipboard (`KeyboardHandler.clipboardManager`).
    pub(in crate::game::client_side) clipboard:
        ClientOnly<sunrise_client::clipboard::SystemClipboard>,
    pub(in crate::game::client_side) pending_save: ClientOnly<Option<saving::ClosingSave>>,
    /// `Hud.tabList`: the player list's header, footer and heart states.
    pub(in crate::game::client_side) tab_list: ClientOnly<sunrise_client::tab_list::TabList>,
    pub(in crate::game::client_side) adv_ui: ClientOnly<advancements_screen::AdvUi>,
}

#[cfg(feature = "client")]
pub(in crate::game) struct ClientCpuResources {
    pub(in crate::game) frame_model: frames::FrameModel,
    pub(in crate::game) glow_frame_model: frames::FrameModel,
    pub(in crate::game) map_frame_model: frames::FrameModel,
    pub(in crate::game) glow_map_frame_model: frames::FrameModel,
    pub(in crate::game) waypoint_styles: sunrise_client::locator_bar::WaypointStyles,
    pub(in crate::game) boat_models: boats::BoatModels,
    pub(in crate::game) cart_models: minecarts::CartModels,
    pub(in crate::game) crystal_model: Option<sunrise_client::mobs::Part>,
    pub(in crate::game) cushion_model: Option<sunrise_client::mobs::Part>,
    pub(in crate::game) dragon_model: Option<sunrise_client::mobs::Part>,
    pub(in crate::game) arrow_model: Option<sunrise_client::mobs::Part>,
    pub(in crate::game) bee_stinger_model: Option<sunrise_client::mobs::Part>,
    pub(in crate::game) trident_model: Option<sunrise_client::mobs::Part>,
    pub(in crate::game) entity_models: EntityModels,
}

#[cfg(feature = "client")]
pub(in crate::game) struct ClientConstructEffects {
    pub(in crate::game) fog: sunrise_client::fog::FogState,
    pub(in crate::game) vanilla_mappings: sunrise_client::keymapping::VanillaMappings,
    pub(in crate::game) health_hud: hud_layout::HealthHud,
    pub(in crate::game) music: sunrise_client::music::MusicManager,
    pub(in crate::game) ambience: sunrise_client::ambience::Ambience,
    pub(in crate::game) frame_clock: sunrise_client::window::SystemClock,
    pub(in crate::game) narration: sunrise_client::screen_narrator::ScreenNarrator,
    pub(in crate::game) tick_script: tick_script::TickScript,
    pub(in crate::game) social: social::Social,
    pub(in crate::game) skin_profiles: RefCell<sunrise_client::skins::Resolver>,
    pub(in crate::game) chat_ui: chat::ChatUiState,
}

#[cfg(feature = "client")]
impl ClientState {
    // Kept out-of-line so its field temporaries do not bloat `ClientGame::construct`'s unoptimised frame.
    #[inline(never)]
    #[allow(clippy::too_many_arguments, clippy::boxed_local)]
    pub(in crate::game) fn build(
        now: Instant,
        music_toast: u8,
        tutorial: tutorial::Tutorial,
        standalone_geometry: native_block_entity_draw::StandaloneGeometry,
        prepared: Box<ClientCpuResources>,
        effects: Box<ClientConstructEffects>,
        extra: Box<super::ClientSidePrep>,
        wc: Box<super::WrappedClientInit>,
        wr: Box<super::WrappedRendererInit>,
    ) -> Box<Self> {
        Box::new(Self {
            debug_density: RefCell::default(),
            pending_pack_reload: false,
            frame_attempted: render_settings::FrameAttempt::default(),
            pack_folder: None,
            reload_overlay: None,
            link_prompt: None,
            code_of_conduct: None,
            world_renderer_ready: false,
            instance_groups: Default::default(),
            standalone_geometry,
            fog: effects.fog,
            screen_fx: Default::default(),
            item_activation: Default::default(),
            weather_parameters: Default::default(),
            panorama_rotation: Default::default(),
            lod_source_attempted: false,
            lod_observe: Default::default(),
            lod_level: None,
            lod_covered_at: None,
            view_arrival: Default::default(),
            drop_spam_throttler: Default::default(),
            rope_rot_old: (0.0, 10.0),
            previous_eye: 1.62,
            inv_drag: None,
            container_buttons: HashSet::new(),
            container_owned_up: false,
            physical_presses: 0,
            tweaks: Default::default(),
            bundle_mouse: Default::default(),
            inv_clicks: Default::default(),
            last_inv_click: None,
            last_quick_moved: None,
            destroying: HashMap::new(),
            mining_crack: None,
            keys: Default::default(),
            physical_keys: HashSet::new(),
            rejected_keys: HashSet::new(),
            physical_buttons: HashSet::new(),
            drag_button: None,
            right_click_emulation: Default::default(),
            cursor: (0.0, 0.0),
            window_cursor: None,
            window_pointer_space: std::cell::OnceCell::new(),
            cursor_icon: sunrise_client::widgets::Cursor::Arrow,
            captured: false,
            capture_mode: None,
            regrab_on_refocus: false,
            controller_notice: Default::default(),
            focused: true,
            unclaimed_window: false,
            unclaimed_modifiers: None,
            focus_after_run: None,
            ime_allowed: false,
            held_key_text: Default::default(),
            ime_area: None,
            focus_lost: None,
            modifiers: winit::keyboard::ModifiersState::empty(),
            fullscreen: sunrise_client::window::FullscreenState::new(false),
            mouse: MouseLook::default(),
            agent_pointer: false,
            smooth_camera: false,
            vanilla_mappings: effects.vanilla_mappings,
            was_playing: false,
            debug: false,
            gpu_passes: false,
            gpu_pass_detail: false,
            screen: Screen::Playing,
            resize_state: None,
            last_device_recovery: None,
            entity_vertices: Vec::new(),
            dynamic_lights: Vec::new(),
            luminance: Default::default(),
            see_through_vertices: Vec::new(),
            shadow_only_vertices: Default::default(),
            entity_spans: Default::default(),
            entity_runs: Vec::new(),
            entity_motion: Default::default(),
            entity_motion_posed: Default::default(),
            skin_frame: Default::default(),
            glowing_vertices: Default::default(),
            mod_sounds: Default::default(),
            mod_clear_particles: Default::default(),
            pack_ids: None,
            health_hud: effects.health_hud,
            started: now,
            spectator_gui: Default::default(),
            last_input: now,
            frames: 0,
            glint_frame: Default::default(),
            kept_block_entities: Default::default(),
            dynamic_textures: Default::default(),
            text_patches: Default::default(),
            prepared_model_draws: Default::default(),
            model_copy_order: Vec::new(),
            world_geometry: Some(Default::default()),
            debug_renderers_version: None,
            hud_layer_next: None,
            mod_view_distances: Default::default(),
            block_entity_reach: Default::default(),
            item_level_cache: Default::default(),
            world_frames: 0,
            fps: 0.0,
            fps_frames: 0,
            fps_at: now,
            gui_ticks: 0,
            texture_ticks: 0,
            message: None,
            music: effects.music,
            music_layers: Default::default(),
            // ToastManager.initializeMusicToast: Pause and Pause and Toast make the toast.
            now_playing: (music_toast != 0).then(Default::default),
            music_toast_state: music_toast,
            music_notes: Default::default(),
            ambience: effects.ambience,
            sound_paused: false,
            cart_sounds: HashSet::new(),
            riding_cart_sounds: None,
            riding_sound: None,
            was_fall_flying: false,
            bee_sounds: HashMap::new(),
            jukebox_sounds: HashMap::new(),
            guardian_look_scratch: Vec::new(),
            explicit_particle_quads: Default::default(),
            sign_edit: None,
            book: None,
            book_last_click: None,
            station_ui: None,
            loading_progress: 0.0,
            loading_progress_at: None,
            moving_pistons_stale: false,
            view: Default::default(),
            limiter: sunrise_client::window::FramerateLimiter::new(0),
            frame_clock: effects.frame_clock,
            scope_scale: 0.5,
            frame_ticks: 0.0,
            hotbar_pop: [0; 10],
            titles: Default::default(),
            tool_highlight: Default::default(),
            saving_indicator: Default::default(),
            heights: Default::default(),
            sway: Default::default(),
            visible_hands: [(0, None), (0, None)],
            hands_replaced: [false; 2],
            hand_strength: Default::default(),
            look: Default::default(),
            tutorial,
            toast_manager: Default::default(),
            narration: effects.narration,
            options_from_title: false,
            avatar: Default::default(),
            custom_sky: Default::default(),
            rain_sound_time: 0,
            lightning_seen: [0; 2],
            win: None,
            demo_intro: None,
            falling_vertices: Default::default(),
            transient_blocks: Default::default(),
            frame_model: prepared.frame_model,
            glow_frame_model: prepared.glow_frame_model,
            map_frame_model: prepared.map_frame_model,
            glow_map_frame_model: prepared.glow_map_frame_model,
            waypoint_styles: prepared.waypoint_styles,
            waypoints: Default::default(),
            locator_camera: None,
            native_camera_frame: None,
            posed_rotation: None,
            post_effects: Vec::new(),
            requested_post_effects_buf: Vec::new(),
            polytone_post_ticks: 0.0,
            expression_frame: Default::default(),
            spectated_post_effect: None,
            spectated_post_effect_active: false,
            experience_display: Default::default(),
            boat_models: prepared.boat_models,
            crystal_model: prepared.crystal_model,
            cushion_model: prepared.cushion_model,
            entity_models: prepared.entity_models,
            dragon_model: prepared.dragon_model,
            map_patches: Vec::new(),
            name_tag_layouts: Default::default(),
            see_through_collector: Default::default(),
            translucent_display_collector: Default::default(),
            display_visible_scratch: Vec::new(),
            block_display_parts_scratch: Vec::new(),
            display_inline_profiles_scratch: Vec::new(),
            interactions: Default::default(),
            remote_knots: Default::default(),
            text_display_vertices: 0,
            skin_slots: sunrise_client::skins::SkinSlots::new(sunrise_client::hud::SKIN_SLOTS),
            skin_profiles: effects.skin_profiles,
            chat_ui: effects.chat_ui,
            map_slots: super::atlas_slots::AtlasSlots::new(sunrise_client::hud::MAP_SLOTS),
            trim_slots: Default::default(),
            cart_models: prepared.cart_models,
            mount_menu: Default::default(),
            remote_hooks: Vec::new(),
            remote_clouds: Vec::new(),
            tick_hit: Default::default(),
            tick_mob: None,
            predicted_equipment_modifiers: Vec::new(),
            trident_model: prepared.trident_model,
            arrow_model: prepared.arrow_model,
            bee_stinger_model: prepared.bee_stinger_model,
            creative_search_named: None,
            creative_ignore_text: false,
            agent_control_on: false,
            agent_last: None,
            frame_stats: Default::default(),
            alloc_mark: None,
            tick_script: effects.tick_script,
            view_frustum: None,
            pending_uploads_order: None,
            upload_pace: Default::default(),
            extras: Default::default(),
            pvp: Default::default(),
            panel: Default::default(),
            native_shaders: Default::default(),
            mod_keys: Default::default(),
            native_mod_screen: None,
            water_plane_memo: Default::default(),
            water_plane_scan: Default::default(),
            last_connection: None,
            connection_open: false,
            pending_retry: None,
            retry_due: None,
            window_title: None,
            studio: Default::default(),
            options_to_world_options: false,
            test_highlights: HashMap::new(),
            social: effects.social,
            debug_keys: Default::default(),
            debug_charts: Default::default(),
            debug_options: None,
            memory_meter: Default::default(),
            debug_paint: Default::default(),
            debug_refresh: Default::default(),
            gpu_utilization: 0.0,
            gpu_culling_auto: sunrise_client::autotune::GpuCullingAuto::new(),
            dynamic_resolution: Default::default(),
            rendered_mobs: Default::default(),
            screen_blur_requested: Default::default(),
            pause_menu: true,
            pause_focus: None,
            death_focus: None,
            focus_screen: 0,
            gamemode_switcher: None,
            user_input: Default::default(),
            text_selection: Default::default(),
            client_quit: std::cell::Cell::new(saving::ClientQuit::Running),
            saved_for_exit: false,
            frame_drawn: true,
            world_icon: None,
            client_pick: Default::default(),
            boss_overlay: Default::default(),
            border_renderer: Default::default(),
            border_section: None,
            border_state: Default::default(),
            subtitles: Default::default(),
            stats_ui: Default::default(),
            links_ui: Default::default(),
            camera_type: Default::default(),
            requested_camera_type: Default::default(),
            darkness_blend: (0.0, 0.0),
            player_model: None,
            pending_entry: wc.pending_entry,
            pack_reload: wc.pack_reload,
            server_packs: wc.server_packs,
            client_reports: wc.client_reports,
            command_prompt: wc.command_prompt,
            dialogs: wc.dialogs,
            pending_initial_models: wc.pending_initial_models,
            stitching_block_atlas: wc.stitching_block_atlas,
            lod: wc.lod,
            lod_blue_map: wc.lod_blue_map,
            video_modes: wc.video_modes,
            screen_state: wc.screen_state,
            window: wc.window,
            renderer: wr.renderer,
            skin_models: wc.skin_models,
            lent_level_random: wc.lent_level_random,
            pad: wc.pad,
            live_scenes: wc.live_scenes,
            debug_renderers: wc.debug_renderers,
            audio: extra.audio,
            particles: extra.particles,
            pickups: wc.pickups,
            creation_job: wc.creation_job,
            title: wc.title,
            armor_stand_assets: extra.armor_stand_assets,
            text_displays: wc.text_displays,
            skin_downloads: extra.skin_downloads,
            recipe_ui: wc.recipe_ui,
            spawners: wc.spawners,
            mod_screen: wc.mod_screen,
            creative_screen: extra.creative_screen,
            pending_screenshot: wc.pending_screenshot,
            screenshot_sequence: wc.screenshot_sequence,
            pending_render_capture: wc.pending_render_capture,
            pending_bench: wc.pending_bench,
            flight: wc.flight,
            ui_wait: wc.ui_wait,
            input_trace: wc.input_trace,
            repro: wc.repro,
            occlusion: wc.occlusion,
            async_particle_culler: wc.async_particle_culler,
            pending_uploads: wc.pending_uploads,
            pending_uploads_merge: wc.pending_uploads_merge,
            mod_jobs: wc.mod_jobs,
            client_item_use: wc.client_item_use,
            client_tasks: wc.client_tasks,
            pending_client_tasks: wc.pending_client_tasks,
            world_options: wc.world_options,
            command_block_screen: wc.command_block_screen,
            block_editor: wc.block_editor,
            registered_debug_entries: extra.registered_debug_entries,
            debug_entries: extra.debug_entries,
            clipboard: wc.clipboard,
            pending_save: wc.pending_save,
            tab_list: wc.tab_list,
            adv_ui: wc.adv_ui,
        })
    }
}

#[cfg(feature = "client")]
#[allow(dead_code)]
#[repr(transparent)]
pub(in crate::game) struct ClientOnly<T>(pub(in crate::game::client_side) T);

#[cfg(feature = "client")]
#[allow(dead_code)]
impl<T> ClientOnly<T> {
    #[inline(always)]
    pub(in crate::game) fn new(value: T) -> Self {
        Self(value)
    }
}

#[cfg(feature = "client")]
#[allow(dead_code)]
#[repr(transparent)]
pub(in crate::game) struct ClientShared<T>(pub(in crate::game) T);

#[cfg(feature = "client")]
#[allow(dead_code)]
impl<T> ClientShared<T> {
    #[inline(always)]
    pub(in crate::game) fn new(value: T) -> Self {
        Self(value)
    }
}

#[cfg(feature = "client")]
impl crate::game::Side for ClientState {
    const IS_SERVER: bool = false;
    fn host_side<'a, 'g>(game: &'g mut crate::game::Game<'a, Self>) -> mod_api::HostSide<'g> {
        mod_api::HostSide::Client(game)
    }
    fn as_host<'a, 'g>(game: &'g crate::game::Game<'a, Self>) -> &'g dyn mod_api::Host {
        game
    }
    fn as_block_states_backend<'a, 'g>(
        game: &'g crate::game::Game<'a, Self>,
    ) -> &'g dyn mod_api::prelude::BlockStatesBackend {
        game
    }
    fn as_config_context<'a, 'g>(
        game: &'g crate::game::Game<'a, Self>,
    ) -> &'g dyn mod_api::ConfigContext {
        game
    }
    fn as_reload_context<'a, 'g>(
        game: &'g crate::game::Game<'a, Self>,
    ) -> &'g dyn mod_api::ReloadContext {
        game
    }
    fn as_pack_discovery_context_mut<'a, 'g>(
        game: &'g mut crate::game::Game<'a, Self>,
    ) -> &'g mut dyn mod_api::PackDiscoveryContext {
        game
    }
    fn as_client_host_mut<'a, 'g>(
        game: &'g mut crate::game::Game<'a, Self>,
    ) -> Option<&'g mut dyn mod_api::ClientHost> {
        Some(game)
    }
    fn as_client_text_host<'a, 'g>(
        game: &'g crate::game::Game<'a, Self>,
    ) -> Option<&'g dyn mod_api::prelude::ClientTextClientHost> {
        Some(game)
    }
    fn attachment_store<'a, 'g>(
        game: &'g mut crate::game::Game<'a, Self>,
    ) -> &'g mut dyn mod_api::AttachmentStore {
        game
    }
    fn game_side_tag_items<'a>(
        &self,
        remote: &'a Option<crate::game::remote::Remote>,
        _res: &'a crate::game::Resources,
    ) -> Option<&'a sunrise_client::items::Items> {
        remote.as_ref().and_then(|remote| remote.tags.as_ref())
    }
}
