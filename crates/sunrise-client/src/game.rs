//! The playable game loop: streaming world, player simulation at 20 ticks/s,
//! block interaction, inventory and HUD.
#[allow(unused_imports)]
#[cfg(test)]
use key_codes::drop_held_behind_dialog;
#[allow(unused_imports)]
use key_codes::{
    follow_named_keymap, gameplay_has_keyboard, glfw_button, glfw_modifiers, keyboard_button,
};
#[allow(unused_imports)]
use loading_screen::{
    App, Loading, leave_fullscreen_after, preflight_resources, run_event_loop,
    unclean_start_recovery,
};
#[allow(unused_imports)]
use motion_ids::{motion_at, motion_block_entity};
#[allow(unused_imports)]
use resource_load::{
    Progress, StartupLoadContext, launch_world, load_resources_for_mods, load_resources_sync,
    named_scope, synchronous_startup,
};
#[allow(unused_imports)]
#[cfg(test)]
use resource_load::{load_resources, load_resources_skip_lang, load_resources_uncached};
/// The content tables the world plays with, as field-level borrows: the
/// world's data packs' over the game's own. The `Game::items`/`recipes`/
/// `stations` methods read the same tables, but taking `&self` blocks a
/// `&mut self.…` in the same scope; these expand to a match on the fields,
/// which borrows stay disjoint from.
macro_rules! world_items {
    ($s:ident) => {
        match (&$s.server_items, $crate::game::Side::world_tables(&$s.side)) {
            (Some(items), _) => &**items,
            (None, Some(t)) => &t.items,
            (None, None) => $s.res.items(),
        }
    };
}
#[allow(unused_macros)]
macro_rules! server_world_items {
    ($s:ident) => {
        match (&$s.server_items, $s.side.world_tables.0.as_ref()) {
            (Some(items), _) => &**items,
            (None, Some(t)) => &t.items,
            (None, None) => $s.res.items(),
        }
    };
}
macro_rules! world_recipes {
    ($s:ident) => {
        match $crate::game::Side::world_tables(&$s.side) {
            Some(t) => &t.recipes,
            None => $s.res.recipes(),
        }
    };
}
#[allow(unused_macros)]
macro_rules! server_world_recipes {
    ($s:ident) => {
        match $s.side.world_tables.0.as_ref() {
            Some(t) => &t.recipes,
            None => $s.res.recipes(),
        }
    };
}
macro_rules! world_stations {
    ($s:ident) => {
        match $crate::game::Side::world_tables(&$s.side) {
            Some(t) => &t.stations,
            None => $s.res.stations(),
        }
    };
}
#[allow(unused_macros)]
macro_rules! server_world_stations {
    ($s:ident) => {
        match $s.side.world_tables.0.as_ref() {
            Some(t) => &t.stations,
            None => $s.res.stations(),
        }
    };
}
/// The biome table the world plays with (`Game::biomes`), as a field-level
/// borrow: the world's packs' over the game's own.
macro_rules! world_biomes {
    ($s:expr) => {
        match &$s.world_biomes {
            Some(b) => b,
            None => $s.res.biomes(),
        }
    };
}
/// The acting player's stacks as field-level borrows, in vanilla `EquipmentSlot` order.
macro_rules! acting_equipment {
    ($s:expr) => {{
        use sunrise_items::equipment::{BODY, SADDLE};
        [
            $s.inventory[$s.selected].as_ref(),
            $s.offhand.as_ref(),
            $s.armor[3].as_ref(),
            $s.armor[2].as_ref(),
            $s.armor[1].as_ref(),
            $s.armor[0].as_ref(),
            $s.equipment.get(BODY),
            $s.equipment.get(SADDLE),
        ]
    }};
}
/// This side's level data for a tick as a field-level borrow, so it stands beside the tick's own
/// borrows of the game. Build it before taking a field mutably.
macro_rules! game_side {
    ($s:expr) => {
        $crate::game::native_surface::GameSide {
            dim: $s.dim,
            level: $s.native_lifecycle.epoch().and_then(|epoch| {
                let key = $s.dim.key();
                let id = mod_api::__private::dimension_id(key)?;
                Some(mod_api::LevelRef::with_generation(
                    epoch,
                    key,
                    $crate::game::Side::levels_generation(&$s.side, id),
                ))
            }),
            facts: $s.res.facts(),
            biomes: world_biomes!($s),
            behaviours: $s.res.behaviours(),
            features: if $s.options.execution_role.is_logical_server() {
                $s.level.enabled_features.as_arc()
            } else {
                match &$s.remote {
                    Some(remote) => &remote.features,
                    None => $crate::game::tables::default_client_features(),
                }
            },
            items: $s.res.items(),
            damage_types: &$s.health_resources.damage,
            player_equipment: acting_equipment!($s),
            hit_borrows: $crate::game::native_surface::HitBorrows {
                living_types: $s.res.living_type_keys(),
                attributes: &$s.motion.attributes,
                player_effects: &$s.potions.effects,
                player_uuid: $crate::game::Side::lan_acting_uuid(&$s.side),
                player_attachments: &$s.player_state.attachments,
                entity_attachments: &*$s.entity_attachments,
            },
            tag_items: $crate::game::Side::game_side_tag_items(&$s.side, &$s.remote, &$s.res),
        }
    };
    ($s:expr, $server:expr) => {{
        let (player_uuid, level, session_lock): (Option<u128>, Option<mod_api::LevelRef>, bool) =
            $server;
        $crate::game::native_surface::GameSide {
            dim: $s.dim,
            level,
            facts: $s.res.facts(),
            biomes: world_biomes!($s),
            behaviours: $s.res.behaviours(),
            features: if $s.options.execution_role.is_logical_server() {
                $s.level.enabled_features.as_arc()
            } else {
                match &$s.remote {
                    Some(remote) => &remote.features,
                    None => $crate::game::tables::default_client_features(),
                }
            },
            items: $s.res.items(),
            damage_types: &$s.health_resources.damage,
            player_equipment: acting_equipment!($s),
            hit_borrows: $crate::game::native_surface::HitBorrows {
                living_types: $s.res.living_type_keys(),
                attributes: &$s.motion.attributes,
                player_effects: &$s.potions.effects,
                player_uuid,
                player_attachments: &$s.player_state.attachments,
                entity_attachments: &*$s.entity_attachments,
            },
            tag_items: if $s.options.execution_role.is_client() {
                $s.remote.as_ref().and_then(|remote| remote.tags.as_ref())
            } else {
                session_lock.then(|| $s.res.items())
            },
        }
    }};
}
/// Server-side `game_side!` using direct field borrows on `self.side` so sibling mutable borrows
/// of unrelated `ServerState` fields (e.g. `pending_sounds`, `ordered.lava_random`) stay disjoint.
#[allow(unused_macros)]
macro_rules! server_game_side {
    ($s:expr) => {{
        let level = $s.native_lifecycle.epoch().and_then(|epoch| {
            let key = $s.dim.key();
            let id = mod_api::__private::dimension_id(key)?;
            Some(mod_api::LevelRef::with_generation(
                epoch,
                key,
                $s.side.levels.0.runtime.generation_of(id),
            ))
        });
        let player_uuid = $s.side.lan_acting_uuid;
        let session_lock = $s.side.session_lock.0.is_some();
        game_side!($s, (player_uuid, level, session_lock))
    }};
}
/// The world's packs' content files, as a field-level borrow (see the macros).
fn pack_files(world_tables: &Option<WorldTables>) -> sunrise_world::loot::PackOverlay<'_> {
    match world_tables {
        Some(t) => sunrise_world::loot::PackOverlay {
            conditions: &t.conditions,
            files: &t.files,
            priority_layers: &t.priority_layers,
        },
        None => sunrise_world::loot::PackOverlay::NONE,
    }
}
// The modules of the game/<area>/ folders, by name: an area folder is not a module.
#[allow(unused_imports)]
pub(in crate::game) use client_side::activity;
#[allow(unused_imports)]
pub(in crate::game) use client_side::advancements_screen;
#[allow(unused_imports)]
pub(in crate::game) use client_side::agent;
#[allow(unused_imports)]
pub(in crate::game) use client_side::agent_settings;
#[allow(unused_imports)]
pub(in crate::game) use server_side::acting_state;
#[allow(unused_imports)]
pub(in crate::game) use server_side::advancements;
#[allow(unused_imports)]
pub(in crate::game) use server_side::ahead;
#[allow(unused_imports)]
pub(in crate::game) use server_side::server_effects;
#[cfg(test)]
#[path = "game/mobs/allay_jukebox_source_tests.rs"]
mod allay_jukebox_source_tests;
#[cfg(test)]
#[path = "game/mobs/allay_pickup_cooldown_tests.rs"]
mod allay_pickup_cooldown_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::allays;
#[allow(unused_imports)]
pub(crate) use server_side::api_values;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::armor_stand_refind_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::armor_stands;
#[path = "game/interface/atlas_slots.rs"]
mod atlas_slots;
#[allow(unused_imports)]
pub(in crate::game) use server_side::attachment_index;
#[cfg(test)]
#[path = "game/modding/attachment_instruction_bench.rs"]
mod attachment_instruction_bench;
#[allow(unused_imports)]
pub(in crate::game) use server_side::attachment_sync;
#[path = "game/world/autosave.rs"]
mod autosave;
#[cfg(test)]
#[path = "game/modding/baked_model_keys_tests.rs"]
mod baked_model_keys_tests;
#[path = "game/blocks/banners.rs"]
mod banners;
#[cfg(test)]
#[path = "game/modding/bare_declaration_names_tests.rs"]
mod bare_declaration_names_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::beacon;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::beacon_cull_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::block_breaking;
#[allow(unused_imports)]
pub(in crate::game) use client_side::block_editor;
#[allow(unused_imports)]
pub(in crate::game) use server_side::beds;
#[allow(unused_imports)]
pub(in crate::game) use server_side::beehives;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::block_drops_pops_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::block_edits;
#[allow(unused_imports)]
pub(in crate::game) use server_side::block_entity_anims;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::block_entity_part_state_tests;
#[cfg(test)]
#[path = "game/blocks/block_entity_reach_draw_tests.rs"]
pub(in crate::game) mod block_entity_reach_draw_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::block_entity_ticks;
#[path = "game/blocks/block_items.rs"]
mod block_items;
#[path = "game/modding/block_notifications.rs"]
mod block_notifications;
#[cfg(test)]
#[path = "game/blocks/block_part_tool_tests.rs"]
mod block_part_tool_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::block_particles;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::block_screen_effect_draw_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::block_ticks;
#[allow(unused_imports)]
pub(in crate::game) use server_side::block_use_completion;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::block_use_completion_tests;
#[path = "game/runtime/blockctl.rs"]
mod blockctl;
#[allow(unused_imports)]
pub(in crate::game) use server_side::boarding;
#[allow(unused_imports)]
pub(in crate::game) use server_side::boat_authority_fixture;
#[allow(unused_imports)]
pub(in crate::game) use server_side::boat_leads;
#[allow(unused_imports)]
pub(in crate::game) use server_side::boat_passengers;
#[cfg(test)]
#[path = "game/entities/boat_refind_tests.rs"]
mod boat_refind_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::boat_rider_fixture;
#[allow(unused_imports)]
pub(in crate::game) use client_side::books;
#[allow(unused_imports)]
pub(in crate::game) use client_side::bookshelves;
#[allow(unused_imports)]
pub(in crate::game) use server_side::boats;
#[allow(unused_imports)]
pub(in crate::game) use server_side::bossbars;
#[path = "game/mobs/breezes.rs"]
mod breezes;
#[allow(unused_imports)]
pub(in crate::game) use server_side::brewing_remainder;
#[path = "game/blocks/brush.rs"]
pub(in crate::game) mod brush;
#[allow(unused_imports)]
pub(in crate::game) use server_side::bucket_empty;
#[allow(unused_imports)]
pub(in crate::game) use server_side::buckets;
#[allow(unused_imports)]
pub(in crate::game) use server_side::building_players;
#[cfg(test)]
#[path = "game/items/bundle_use_tests.rs"]
mod bundle_use_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::bundles;
#[path = "game/items/cake_eligibility.rs"]
mod cake_eligibility;
#[allow(unused_imports)]
pub(in crate::game) use client_side::camera_hud;
#[cfg(test)]
#[path = "game/interface/camera_submersion_tests.rs"]
mod camera_submersion_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::campfire;
#[path = "game/blocks/campfire_douse.rs"]
pub(in crate::game) mod campfire_douse;
#[allow(unused_imports)]
pub(in crate::game) use server_side::carrier_transfers;
#[path = "game/entities/cart_rails.rs"]
mod cart_rails;
#[path = "game/blocks/cartography.rs"]
mod cartography;
#[allow(unused_imports)]
pub(in crate::game) use server_side::cat_spawner;
#[allow(unused_imports)]
pub(in crate::game) use server_side::cauldron_interactions;
#[path = "game/network/cauldron_prediction.rs"]
mod cauldron_prediction;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::cauldron_underwater_tests;
#[path = "game/blocks/cauldrons.rs"]
mod cauldrons;
#[allow(unused_imports)]
pub(in crate::game) use client_side::chat;
#[allow(unused_imports)]
pub(in crate::game) use server_side::chiseled_bookshelf;
#[path = "game/world/class_tags.rs"]
mod class_tags;
#[cfg(test)]
#[path = "game/world/class_tags_tests.rs"]
mod class_tags_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::client_ambient;
#[allow(unused_imports)]
pub(in crate::game) use client_side::client_attack_targets;
#[allow(unused_imports)]
pub(in crate::game) use client_side::client_cracks;
#[allow(unused_imports)]
pub(in crate::game) use client_side::client_executor;
#[allow(unused_imports)]
pub(in crate::game) use client_side::client_item_extensions;
#[allow(unused_imports)]
pub(in crate::game) use client_side::client_reload;
#[path = "game/client_side.rs"]
pub(in crate::game) mod client_side;
#[allow(unused_imports)]
pub(in crate::game) use client_side as client_state;
#[allow(unused_imports)]
pub(in crate::game) use client_side::client_tick_steps;
#[path = "game/items/clip_ray.rs"]
mod clip_ray;
#[allow(unused_imports)]
pub(in crate::game) use client_side::clipboard;
#[cfg(test)]
#[path = "game/blocks/clone_block_ticks_tests.rs"]
mod clone_block_ticks_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::clone_dimensions_tests;
#[path = "game/entities/closest_space.rs"]
mod closest_space;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::cloud_height_frame_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::code_of_conduct;
#[path = "game/entities/collision_source.rs"]
mod collision_source;
#[allow(unused_imports)]
pub(in crate::game) use client_side::command_blocks;
#[allow(unused_imports)]
pub(in crate::game) use client_side::command_minecarts;
#[allow(unused_imports)]
pub(in crate::game) use server_side::combat;
#[allow(unused_imports)]
pub(in crate::game) use server_side::command_harness;
#[path = "game/interface/books_common.rs"]
pub(in crate::game) mod books_common;
#[path = "game/interface/chat_common.rs"]
pub(in crate::game) mod chat_common;
#[path = "game/world/clocks.rs"]
pub(in crate::game) mod clocks;
#[path = "game/administration/command_blocks_common.rs"]
pub(in crate::game) mod command_blocks_common;
#[path = "game/interface/command_registries.rs"]
mod command_registries;
#[path = "game/administration/commands_common.rs"]
pub(in crate::game) mod commands_common;
#[path = "game/blocks/machines/crafter_common.rs"]
pub(in crate::game) mod crafter_common;
#[path = "game/world/integrated_common.rs"]
pub(in crate::game) mod integrated_common;
#[path = "game/runtime/memory_common.rs"]
pub(in crate::game) mod memory_common;
#[path = "game/items/stack_names.rs"]
pub(in crate::game) mod stack_names;
#[path = "game/world/world_entry_common.rs"]
pub(in crate::game) mod world_entry_common;
#[allow(unused_imports)]
pub(in crate::game) use server_side::commands;
#[allow(unused_imports)]
pub(in crate::game) use server_side::composter;
#[path = "game/world/condition_reload_state.rs"]
mod condition_reload_state;
#[allow(unused_imports)]
pub(in crate::game) use server_side::consume;
#[path = "game/modding/consume_effect_dispatch.rs"]
mod consume_effect_dispatch;
#[cfg(test)]
#[path = "game/modding/consume_effect_dispatch_tests.rs"]
mod consume_effect_dispatch_tests;
#[cfg(test)]
#[path = "game/interface/contained_fluid_tests.rs"]
mod contained_fluid_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::container_hooks;
#[allow(unused_imports)]
pub(in crate::game) use client_side::container_layout;
#[allow(unused_imports)]
pub(in crate::game) use client_side::control;
#[allow(unused_imports)]
pub(in crate::game) use server_side::container_loot;
#[cfg(test)]
#[path = "game/mobs/conversion_place_tests.rs"]
mod conversion_place_tests;
#[path = "game/mobs/copper_golems.rs"]
pub(in crate::game) mod copper_golems;
#[cfg(test)]
#[path = "game/runtime/core_tests.rs"]
mod core_tests;
#[cfg(test)]
#[path = "game/modding/crafting_recipe_kind_tests.rs"]
mod crafting_recipe_kind_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::creaking;
#[cfg(test)]
#[path = "game/interface/create_fluid_source_tests.rs"]
mod create_fluid_source_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::creative_hotbar;
#[path = "game/interface/creative_screen.rs"]
pub(in crate::game) mod creative_screen;
#[cfg(test)]
#[path = "game/items/creator_mod_id_tests.rs"]
mod creator_mod_id_tests;
#[path = "game/mobs/creatures/mod.rs"]
mod creatures;
#[cfg(test)]
#[path = "game/interface/crop_grow_tests.rs"]
mod crop_grow_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::crosshair_pick_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::crossbow;
#[cfg(test)]
#[path = "game/entities/cushion_refind_tests.rs"]
mod cushion_refind_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::custom_arm_pose;
#[allow(unused_imports)]
pub(in crate::game) use client_side::custom_sky;
#[allow(unused_imports)]
pub(crate) use server_side::cushions;
#[allow(unused_imports)]
pub(in crate::game) use server_side::custom_spawner_level;
#[allow(unused_imports)]
pub(in crate::game) use server_side::custom_spawners;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::damage_item_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::damage_resistant_tests;
#[path = "game/modding/data_map_files.rs"]
mod data_map_files;
#[path = "game/modding/data_map_json.rs"]
mod data_map_json;
#[allow(unused_imports)]
pub(in crate::game) use client_side::debug_keys;
#[path = "game/interface/debug_lines.rs"]
pub(in crate::game) mod debug_lines;
#[allow(unused_imports)]
pub(in crate::game) use client_side::debug_options;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::debug_renderer_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::debug_renderers;
#[allow(unused_imports)]
pub(in crate::game) use server_side::debug_stick;
#[allow(unused_imports)]
pub(in crate::game) use server_side::decided_action;
#[cfg(test)]
#[path = "game/modding/declared_vegetation_tests.rs"]
mod declared_vegetation_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::demo_intro;
#[path = "game/blocks/decorated_pot.rs"]
pub(in crate::game) mod decorated_pot;
#[allow(unused_imports)]
pub(crate) use server_side::dedicated;
#[allow(unused_imports)]
pub(in crate::game) use server_side::deflect;
#[path = "game/interface/developer_launch.rs"]
mod developer_launch;
#[allow(unused_imports)]
pub(in crate::game) use client_side::dialogs;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::disabled_block_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::dispense_bucket;
#[cfg(test)]
#[path = "game/blocks/dispense_container_tests.rs"]
mod dispense_container_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::dragon_boss_bar_name_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::draw_boundary;
#[allow(unused_imports)]
pub(in crate::game) use server_side::dispense_fixture;
#[allow(unused_imports)]
pub(in crate::game) use server_side::dispenser;
#[allow(unused_imports)]
pub(in crate::game) use server_side::dragon;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::dragon_fireball_motion_tests;
#[cfg(test)]
#[path = "game/items/dropped_by_player_tests.rs"]
mod dropped_by_player_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::dropped_item_setter;
#[path = "game/items/durability.rs"]
mod durability;
#[allow(unused_imports)]
pub(in crate::game) use server_side::dynamic_light_emission;
#[cfg(test)]
#[path = "game/world/dynamic_light_emission_tests.rs"]
mod dynamic_light_emission_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::dynamic_lights;
#[allow(unused_imports)]
pub(in crate::game) use server_side::early_game_rules;
#[cfg(test)]
#[path = "game/world/early_game_rules_tests.rs"]
mod early_game_rules_tests;
#[cfg(test)]
#[path = "game/blocks/empty_contents_tests.rs"]
mod empty_contents_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::enchant_power_beam_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::enchant_effects;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::enchant_pick_parts_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::end;
#[path = "game/world/end_common.rs"]
pub(in crate::game) mod end_common;
#[allow(unused_imports)]
pub(in crate::game) use server_side::enderman;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::enderman_hurt_teleport_tests;
#[cfg(test)]
#[path = "game/interface/enderman_placement_tests.rs"]
mod enderman_placement_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::entity_culling;
#[allow(unused_imports)]
pub(in crate::game) use server_side::entity;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::entity_die_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::entity_initialization;
#[cfg(test)]
#[path = "game/entities/entity_inside_tests.rs"]
mod entity_inside_tests;
#[cfg(test)]
#[path = "game/interface/entity_model_loading_tests.rs"]
mod entity_model_loading_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::entity_part_state;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::entity_part_state_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::entity_residency;
#[path = "game/modding/entity_sections.rs"]
mod entity_sections;
#[allow(unused_imports)]
pub(in crate::game) use client_side::entity_sounds;
#[allow(unused_imports)]
pub(in crate::game) use server_side::entity_sizes;
#[path = "game/modding/entity_type_rows.rs"]
mod entity_type_rows;
#[path = "game/items/equip_answers.rs"]
mod equip_answers;
#[allow(unused_imports)]
pub(in crate::game) use client_side::equipment_piece;
#[cfg(test)]
#[path = "game/mobs/equipment_tick_tests.rs"]
mod equipment_tick_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::event_loop;
#[allow(unused_imports)]
pub(in crate::game) use client_side::events;
#[path = "game/runtime/execution_role.rs"]
mod execution_role;
#[path = "game/modding/item_snapshot.rs"]
pub(in crate::game) mod item_snapshot;
#[path = "game/world/tables.rs"]
pub(in crate::game) mod tables;
#[allow(unused_imports)]
pub(in crate::game) use tables::level_suffocation_of;
#[path = "game/side_ops/mod.rs"]
pub(in crate::game) mod side_ops;
#[allow(unused_imports)]
pub(in crate::game) use client_side::game_client::WindowKeyInput;
#[allow(dead_code)]
pub(in crate::game) type ServerGame<'a> = Game<'a, server_side::ServerState>;
#[allow(dead_code)]
pub(in crate::game) type ClientGame<'a> = Game<'a, client_side::ClientState>;
#[allow(unused_imports)]
pub(in crate::game) use client_side::allays_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::composter_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::modapi_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::spawner_client;
#[allow(unused_imports)]
pub(in crate::game) use server_side::modapi_server;
#[cfg(test)]
#[path = "game/interface/experience_points_tests.rs"]
mod experience_points_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::extras;
#[allow(unused_imports)]
pub(in crate::game) use server_side::explosion;
#[allow(unused_imports)]
pub(in crate::game) use server_side::explosion_detonate;
#[allow(unused_imports)]
pub(in crate::game) use server_side::explosion_settings;
#[allow(unused_imports)]
pub(in crate::game) use server_side::extinguish;
#[cfg(test)]
#[path = "game/runtime/falling_air_tick_tests.rs"]
mod falling_air_tick_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::falling_blocks;
#[allow(unused_imports)]
pub(in crate::game) use server_side::falling_damage;
#[allow(unused_imports)]
pub(in crate::game) use server_side::falling_save;
#[allow(unused_imports)]
pub(in crate::game) use server_side::falling_tick;
#[cfg(test)]
#[path = "game/entities/falling_tick_tests.rs"]
mod falling_tick_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::far_terrain_edits;
#[allow(unused_imports)]
pub(in crate::game) use server_side::fangs;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::fence_leash_force_tests;
#[cfg(test)]
#[path = "game/blocks/fertile_part_tests.rs"]
mod fertile_part_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::finalize_spawn;
#[path = "game/entities/fireworks.rs"]
mod fireworks;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::first_tick_registration_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::fishing;
#[cfg(test)]
#[path = "game/interface/fluid_interaction_tests.rs"]
mod fluid_interaction_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::focused_text_field;
#[allow(unused_imports)]
pub(in crate::game) use client_side::fov;
#[allow(unused_imports)]
pub(in crate::game) use client_side::frame_geometry;
#[allow(unused_imports)]
pub(in crate::game) use client_side::friends;
#[allow(unused_imports)]
pub(in crate::game) use client_side::gamemode_switcher;
#[allow(unused_imports)]
pub(in crate::game) use client_side::general_draw;
#[allow(unused_imports)]
pub(in crate::game) use server_side::ghasts;
#[path = "game/modding/fluid_interactions.rs"]
pub(in crate::game) mod fluid_interactions;
#[allow(unused_imports)]
pub(in crate::game) use server_side::foxes;
#[allow(unused_imports)]
pub(in crate::game) use server_side::frames;
#[allow(unused_imports)]
pub(in crate::game) use server_side::functions;
#[allow(unused_imports)]
pub(in crate::game) use server_side::game_mode;
#[allow(unused_imports)]
pub(in crate::game) use server_side::game_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::general;
#[path = "game/items/glide_wear.rs"]
mod glide_wear;
#[cfg(test)]
#[path = "game/items/glide_wear_tests.rs"]
mod glide_wear_tests;
#[path = "game/modding/global_loot_modifiers.rs"]
mod global_loot_modifiers;
#[allow(unused_imports)]
pub(in crate::game) use client_side::glowing;
#[allow(unused_imports)]
pub(in crate::game) use client_side::growth;
#[allow(unused_imports)]
pub(in crate::game) use client_side::gui_clips;
#[allow(unused_imports)]
pub(in crate::game) use client_side::gui_entity;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::golem_defense_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::golems;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::grindstone_decisions_tests;
#[cfg(test)]
#[path = "game/modding/gui_gradient_tests.rs"]
mod gui_gradient_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::gui_layer_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::gui_paint_items;
#[cfg(test)]
#[path = "game/modding/gui_paint_items_tests.rs"]
mod gui_paint_items_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::gui_pose;
#[allow(unused_imports)]
pub(in crate::game) use client_side::hand;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::hand_part_tests;
#[path = "game/interface/game_mode_common.rs"]
pub(in crate::game) mod game_mode_common;
#[cfg(test)]
#[path = "game/entities/hanging_refind_tests.rs"]
mod hanging_refind_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::headless;
#[path = "game/items/held_items.rs"]
pub(in crate::game) mod held_items;
#[path = "game/modding/native_entity_refs_common.rs"]
pub(in crate::game) mod native_entity_refs_common;
#[path = "game/entities/pick_ray.rs"]
pub(in crate::game) mod pick_ray;
#[path = "game/player_identity.rs"]
pub(in crate::game) mod player_identity;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::highlight_tip_tests;
#[path = "game/interface/hit.rs"]
mod hit;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::hoglin_repeat_attack_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::hook_meddler_tests;
mod host;
#[allow(unused_imports)]
pub(in crate::game) use client_side::hud_editor;
#[allow(unused_imports)]
pub(in crate::game) use server_side::hot_paths;
#[path = "game/interface/hud_render.rs"]
mod hud_render;
#[allow(unused_imports)]
pub(in crate::game) use client_side::hud_room;
#[allow(unused_imports)]
pub(in crate::game) use client_side::humanoid_setup_anim;
#[allow(unused_imports)]
pub(in crate::game) use client_side::ime;
#[allow(unused_imports)]
pub(in crate::game) use client_side::initial_client_models;
#[allow(unused_imports)]
pub(in crate::game) use server_side::infiniburn;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::infiniburn_tests;
#[cfg(test)]
#[path = "game/modding/initial_client_models_tests.rs"]
mod initial_client_models_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::input_trace;
#[allow(unused_imports)]
pub(in crate::game) use client_side::interactions;
#[allow(unused_imports)]
pub(in crate::game) use client_side::inventory_input;
#[path = "game/interface/inventory_slots.rs"]
pub(in crate::game) mod inventory_slots;
#[allow(unused_imports)]
pub(in crate::game) use server_side::initial_spawn;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::inline_feature_parse_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::inline_loot_reference_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::integrated;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::interaction_data_tests;
#[path = "game/items/inventory_tick.rs"]
mod inventory_tick;
#[cfg(test)]
#[path = "game/mobs/invulnerable_time_tests.rs"]
mod invulnerable_time_tests;
#[path = "game/items/item_abilities.rs"]
mod item_abilities;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::item_abilities_tests;
#[path = "game/items/item_actions.rs"]
pub(in crate::game) mod item_actions;
#[cfg(test)]
#[path = "game/items/item_attack_tests.rs"]
mod item_attack_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::item_draw;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::item_contact_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::item_drop_state_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::item_durability;
#[allow(unused_imports)]
pub(in crate::game) use server_side::item_entities;
#[path = "game/items/item_entity_spawn.rs"]
pub(in crate::game) mod item_entity_spawn;
#[allow(unused_imports)]
pub(in crate::game) use server_side::item_hurt;
#[cfg(test)]
#[path = "game/items/item_interact_mob_tests.rs"]
mod item_interact_mob_tests;
#[cfg(test)]
#[path = "game/items/item_menu_parts_tests.rs"]
mod item_menu_parts_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::item_model_context;
#[cfg(test)]
#[path = "game/items/item_name_tests.rs"]
mod item_name_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::item_particle_send;
#[cfg(test)]
#[path = "game/modding/item_particle_send_tests.rs"]
mod item_particle_send_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::item_particles;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::item_pickup_shape_tests;
#[cfg(test)]
#[path = "game/interface/item_slot_command_tests.rs"]
mod item_slot_command_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::item_use;
#[path = "game/items/item_use_stacks.rs"]
mod item_use_stacks;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::item_void_tests;
#[path = "game/administration/jigsaw_blocks.rs"]
mod jigsaw_blocks;
#[allow(unused_imports)]
pub(in crate::game) use client_side::jigsaw_edit;
#[allow(unused_imports)]
pub(in crate::game) use client_side::jukebox;
#[allow(unused_imports)]
pub(in crate::game) use server_side::journal_level_write;
#[path = "game/interface/key_codes.rs"]
mod key_codes;
#[path = "game/interface/key_focus.rs"]
mod key_focus;
#[cfg(test)]
#[path = "game/interface/keymap_tests.rs"]
mod keymap_tests;
#[cfg(test)]
#[path = "game/entities/kill_die_cancel_tests.rs"]
mod kill_die_cancel_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::kill_die_vibration_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::knot_residency_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan;
#[path = "game/network/lan_actions/mod.rs"]
mod lan_actions;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_attributes;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_camera;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_chat;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_configuration_tasks;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_crafting;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_entities;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_far;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_menus;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_menus_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_mobs;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_mount_menu;
#[path = "game/administration/lan_op.rs"]
mod lan_op;
#[allow(unused_imports)]
pub(in crate::game) use client_side::lan_screen;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_particles;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_pickup;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_pvp;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_quiet;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_stations;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_trade_decisions;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_trade_decisions_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_trading;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_vehicles;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lan_waypoints;
#[cfg(test)]
#[path = "game/mobs/last_hurt_by_mob_tests.rs"]
mod last_hurt_by_mob_tests;
#[path = "game/runtime/launch_api.rs"]
mod launch_api;
#[path = "game/runtime/launch_options.rs"]
mod launch_options;
#[allow(unused_imports)]
pub(in crate::game) use client_side::launch_timeout;
#[allow(unused_imports)]
pub(in crate::game) use server_side::leads;
#[path = "game/blocks/level_access.rs"]
mod level_access;
#[path = "game/modding/level_control_hooks.rs"]
mod level_control_hooks;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::lightmap_expression_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::lightning;
#[allow(unused_imports)]
pub(in crate::game) use server_side::level_particles;
#[allow(unused_imports)]
pub(in crate::game) use server_side::level_sounds;
#[allow(unused_imports)]
pub(in crate::game) use server_side::level_tick;
#[allow(unused_imports)]
pub(in crate::game) use server_side::levels;
#[cfg(test)]
#[path = "game/entities/lightning_bolt_uniform_tests.rs"]
mod lightning_bolt_uniform_tests;
#[cfg(test)]
#[path = "game/entities/lightning_item_tests.rs"]
mod lightning_item_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::loading_screen;
#[allow(unused_imports)]
pub(in crate::game) use client_side::loading_window;
#[allow(unused_imports)]
pub(in crate::game) use server_side::living_attributes;
#[allow(unused_imports)]
pub(in crate::game) use server_side::llamas;
#[path = "game/modding/load_conditions.rs"]
pub(in crate::game) mod load_conditions;
#[path = "game/blocks/location_world.rs"]
mod location_world;
#[allow(unused_imports)]
pub(in crate::game) use client_side::locator_bar;
#[allow(unused_imports)]
pub(in crate::game) use client_side::look;
#[allow(unused_imports)]
pub(in crate::game) use client_side::loom;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::loot_kill_guest_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::loot_rolls;
#[path = "game/modding/loot_tags.rs"]
mod loot_tags;
#[path = "game/modding/loot_types.rs"]
pub(crate) mod loot_types;
#[allow(unused_imports)]
pub(in crate::game) use server_side::mace;
#[allow(unused_imports)]
pub(in crate::game) use server_side::machines;
#[cfg(test)]
#[path = "game/entities/mannequin_save_tests.rs"]
mod mannequin_save_tests;
#[path = "game/interface/menu_clicks.rs"]
pub(in crate::game) mod menu_clicks;
#[allow(unused_imports)]
pub(in crate::game) use server_side::maps;
#[allow(unused_imports)]
pub(in crate::game) use server_side::melee;
#[allow(unused_imports)]
pub(in crate::game) use server_side::memory;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::mending_pick_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::minecart_moves;
#[path = "game/entities/minecarts.rs"]
mod minecarts;
#[allow(unused_imports)]
pub(in crate::game) use client_side::mob_blocks;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::mining_component_tests;
#[cfg(test)]
#[path = "game/mobs/mob_discard_tests.rs"]
mod mob_discard_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::mob_gear;
#[cfg(test)]
#[path = "game/world/mob_hurt_cooldown_tests.rs"]
mod mob_hurt_cooldown_tests;
#[cfg(test)]
#[path = "game/mobs/mob_landing_support_tests.rs"]
mod mob_landing_support_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::mob_light;
#[allow(unused_imports)]
pub(in crate::game) use client_side::mob_sounds;
#[allow(unused_imports)]
pub(in crate::game) use client_side::mod_browser;
#[allow(unused_imports)]
pub(in crate::game) use server_side::mob_level;
#[allow(unused_imports)]
pub(in crate::game) use server_side::mob_looting;
#[allow(unused_imports)]
pub(in crate::game) use server_side::mod_api_host;
#[allow(unused_imports)]
pub(in crate::game) use server_side::mod_container_openers;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::mod_container_openers_tests;
#[path = "game/modding/mod_crafting_commands.rs"]
mod mod_crafting_commands;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::mod_menu_decisions_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::mod_raycast_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::mod_recipe_property_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::mod_toasts;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::model_baking_completed_tests;
#[path = "game/modding/mods/mod.rs"]
pub(crate) mod mods;
#[allow(unused_imports)]
pub(in crate::game) use client_side::motion_ids;
#[allow(unused_imports)]
pub(in crate::game) use client_side::mount_menu;
#[allow(unused_imports)]
pub(in crate::game) use server_side::mounts;
#[path = "game/entities/move_gate.rs"]
mod move_gate;
#[path = "game/blocks/moving_pistons.rs"]
pub(in crate::game) mod moving_pistons;
#[allow(unused_imports)]
pub(in crate::game) use client_side::music;
#[allow(unused_imports)]
pub(in crate::game) use client_side::names;
#[allow(unused_imports)]
pub(in crate::game) use client_side::narrator;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_audio_streams;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_biome_colors;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_ai_pause;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_biome_keys;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_biome_keys_tests;
#[path = "game/modding/native_biome_source.rs"]
mod native_biome_source;
#[cfg(test)]
#[path = "game/modding/native_biome_tag_tests.rs"]
mod native_biome_tag_tests;
#[cfg(test)]
#[path = "game/modding/native_block_class_tests.rs"]
mod native_block_class_tests;
#[allow(unused_imports)]
pub use client_side::native_block_entity_draw;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_block_entity_item_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_block_entity_refs;
#[path = "game/modding/native_block_entity_transform.rs"]
mod native_block_entity_transform;
#[cfg(test)]
#[path = "game/modding/native_block_entity_transform_tests.rs"]
mod native_block_entity_transform_tests;
#[path = "game/modding/native_block_extra_models.rs"]
mod native_block_extra_models;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_block_sprites;
#[path = "game/modding/native_block_sounds.rs"]
pub(crate) mod native_block_sounds;
#[path = "game/modding/native_block_states.rs"]
mod native_block_states;
#[path = "game/modding/native_block_tables.rs"]
mod native_block_tables;
#[cfg(test)]
#[path = "game/modding/native_block_use_criteria_tests.rs"]
mod native_block_use_criteria_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_book_apply;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_book_backings;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_book_catalogue;
#[path = "game/modding/native_book_displays.rs"]
mod native_book_displays;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_book_grid;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_book_invalidation;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_book_net;
#[path = "game/modding/native_book_placement.rs"]
mod native_book_placement;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_book_receive;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_book_scope;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_book_settings;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_book_transport;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_boss_bar_refs;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_bots;
#[cfg(test)]
#[path = "game/modding/native_capability_cache_tests.rs"]
mod native_capability_cache_tests;
#[path = "game/modding/native_cauldron_contents.rs"]
mod native_cauldron_contents;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_chat_recipients;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_chat_recipients_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_chunk_areas;
#[path = "game/modding/native_chunk_generator.rs"]
mod native_chunk_generator;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_chunk_queries;
#[cfg(test)]
#[path = "game/modding/native_chunk_relight_tests.rs"]
mod native_chunk_relight_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_claimed_items;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_client_particles;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_client_particles_fixture;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_client_screens;
#[allow(unused_imports)]
pub use client_side::native_client_services;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_client_session;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_chunk_save;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_colliders;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_colliders_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_collision_exclusions;
#[path = "game/modding/native_collision_shape.rs"]
pub(crate) mod native_collision_shape;
#[cfg(test)]
#[path = "game/modding/native_command_completion_tests.rs"]
mod native_command_completion_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_command_reads;
#[path = "game/modding/native_components.rs"]
pub(in crate::game) mod native_components;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_config_files;
#[path = "game/modding/native_configs.rs"]
mod native_configs;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_connection_groups;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_console;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_console_tests;
#[path = "game/modding/native_content_items.rs"]
mod native_content_items;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_custom_geometry;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_criteria;
#[path = "game/modding/native_custom_stats.rs"]
mod native_custom_stats;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_damage_tests;
#[path = "game/modding/native_data_bound.rs"]
mod native_data_bound;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_data_maps;
#[path = "game/modding/native_datafix.rs"]
mod native_datafix;
#[cfg(test)]
#[path = "game/modding/native_datafix_tests.rs"]
mod native_datafix_tests;
#[path = "game/modding/native_datapack_refs.rs"]
mod native_datapack_refs;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_death_weapon;
#[path = "game/modding/native_dimensions.rs"]
mod native_dimensions;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_disconnect;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_disconnect_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_dynamic_textures;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_display_refs;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_dragon_refs;
#[cfg(test)]
#[path = "game/modding/native_dynamic_textures_tests.rs"]
mod native_dynamic_textures_tests;
#[path = "game/modding/native_effect_category.rs"]
mod native_effect_category;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_entity_layers;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entities;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_commands;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_data;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_extinguish;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_extinguish_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_index_parity_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_invulnerable;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_kind;
#[cfg(test)]
#[path = "game/modding/native_entity_layers_tests.rs"]
mod native_entity_layers_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_motion;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_move_correction_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_move_player_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_move_sweep_tests;
#[cfg(test)]
#[path = "game/modding/native_entity_move_tests.rs"]
mod native_entity_move_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_entity_renderers;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_moves;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_physics;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_refs;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_remove;
#[cfg(test)]
#[path = "game/modding/native_entity_renderers_tests.rs"]
mod native_entity_renderers_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_riding;
#[path = "game/modding/native_entity_selectors.rs"]
pub(in crate::game) mod native_entity_selectors;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_sounds;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_entity_state;
#[cfg(test)]
#[path = "game/modding/native_experience_orb_renderers_tests.rs"]
mod native_experience_orb_renderers_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_explosion_destroy;
#[cfg(test)]
#[path = "game/modding/native_explosion_destroy_tests.rs"]
mod native_explosion_destroy_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_explosion_resistance;
#[cfg(test)]
#[path = "game/modding/native_explosion_resistance_tests.rs"]
mod native_explosion_resistance_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_explosions;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_explosions_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_fake_disconnect_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_falling;
#[path = "game/modding/native_feature_types.rs"]
mod native_feature_types;
#[cfg(test)]
#[path = "game/modding/native_fire_odds_tests.rs"]
mod native_fire_odds_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_fluid_render;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_fishing_hook_refs;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_game_event_context;
#[path = "game/modding/native_game_event_keys.rs"]
mod native_game_event_keys;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_game_event_keys_tests;
#[path = "game/modding/native_game_rules_read.rs"]
mod native_game_rules_read;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_general_renderers_tests;
#[path = "game/modding/native_give_stack.rs"]
mod native_give_stack;
#[cfg(test)]
#[path = "game/modding/native_give_stack_tests.rs"]
mod native_give_stack_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_goal_ticks;
#[path = "game/modding/native_ground.rs"]
mod native_ground;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_guest_turn_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_hanging;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_hanging_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_harvest_queries;
#[cfg(test)]
#[path = "game/modding/native_held_out_tests.rs"]
mod native_held_out_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_hit_attempt_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_hit_causer_tests;
#[path = "game/modding/native_hit_effects.rs"]
mod native_hit_effects;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_hit_sounds_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_hit_view_tests;
mod native_hooks;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_hotbar_slot;
#[cfg(test)]
#[path = "game/modding/native_hover_name_tests.rs"]
mod native_hover_name_tests;
#[cfg(test)]
#[path = "game/modding/native_humanoid_setup_anim_tests.rs"]
mod native_humanoid_setup_anim_tests;
#[cfg(test)]
#[path = "game/modding/native_ignited_by_lava_tests.rs"]
mod native_ignited_by_lava_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_instance_groups;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_inventories;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_invulnerability_check_tests;
#[cfg(test)]
#[path = "game/modding/native_is_fire_source_tests.rs"]
mod native_is_fire_source_tests;
#[cfg(test)]
#[path = "game/modding/native_is_piglin_currency_tests.rs"]
mod native_is_piglin_currency_tests;
#[path = "game/modding/native_item_bundle.rs"]
mod native_item_bundle;
#[cfg(test)]
#[path = "game/modding/native_item_bundle_tests.rs"]
mod native_item_bundle_tests;
#[path = "game/modding/native_item_container.rs"]
mod native_item_container;
#[path = "game/modding/native_item_entity_refs.rs"]
mod native_item_entity_refs;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_item_frame_renderers_tests;
#[path = "game/modding/native_item_menu_parts.rs"]
pub(crate) mod native_item_menu_parts;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_item_particles;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_item_places;
#[path = "game/modding/native_item_registry.rs"]
mod native_item_registry;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_key_mappings;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_key_mappings_tests;
#[path = "game/modding/native_knockback_credit.rs"]
pub(in crate::game) mod native_knockback_credit;
#[cfg(test)]
#[path = "game/modding/native_knockback_credit_site_tests.rs"]
mod native_knockback_credit_site_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_knockback_tests;
#[cfg(test)]
#[path = "game/modding/native_level_block_states_tests.rs"]
mod native_level_block_states_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_level_notify_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_level_border;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_level_effects;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_level_events;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_level_items;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_level_placement_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_level_players;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_level_refs;
#[path = "game/modding/native_level_signals.rs"]
pub(in crate::game) mod native_level_signals;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_level_sound_except_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_level_spawning;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_level_ticking;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_level_weather;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_level_world;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_lifecycle;
#[cfg(test)]
#[path = "game/modding/native_live_holder_tests.rs"]
mod native_live_holder_tests;
#[path = "game/modding/native_living_absorption.rs"]
mod native_living_absorption;
#[path = "game/modding/native_living_blocking.rs"]
pub(in crate::game) mod native_living_blocking;
#[path = "game/modding/native_living_body.rs"]
pub(in crate::game) mod native_living_body;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_living_effects;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_living_flags;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_living_position;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_living_refs;
#[cfg(test)]
#[path = "game/modding/native_living_renderers_tests.rs"]
mod native_living_renderers_tests;
#[cfg(test)]
#[path = "game/modding/native_living_travel_sweep_tests.rs"]
mod native_living_travel_sweep_tests;
#[cfg(test)]
#[path = "game/modding/native_living_travel_tests.rs"]
mod native_living_travel_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_local_player_writes;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_local_sounds;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_living_writes;
#[cfg(test)]
#[path = "game/modding/native_makes_piglins_neutral_tests.rs"]
mod native_makes_piglins_neutral_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_mapping_presses;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_menu_refs;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_menu_slot_kinds;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_mob_draw;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_menu_inventory_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_menu_places;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_menus;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_merchant_refs;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_minecart_refs;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_mob_goals;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_mob_navigation;
#[cfg(test)]
#[path = "game/modding/native_mob_navigation_flags_tests.rs"]
mod native_mob_navigation_flags_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_model_3d;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_mob_refs;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_mob_wanted_position;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_mob_wanted_position_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_mobs;
#[path = "game/modding/native_motion.rs"]
mod native_motion;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_mounts;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_move_by_tests;
#[path = "game/modding/native_namespaced_id.rs"]
mod native_namespaced_id;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_net;
#[cfg(test)]
#[path = "game/modding/native_on_caught_fire_tests.rs"]
mod native_on_caught_fire_tests;
#[path = "game/modding/native_packet_builders.rs"]
mod native_packet_builders;
#[cfg(test)]
#[path = "game/modding/native_painting_renderers_tests.rs"]
mod native_painting_renderers_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_panel;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_panel_recipes;
#[cfg(test)]
#[path = "game/modding/native_panel_wheel_tests.rs"]
mod native_panel_wheel_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_part_tables;
#[cfg(test)]
#[path = "game/modding/native_part_tables_tests.rs"]
mod native_part_tables_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_passage_cross;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_passages;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_path_types_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_pending_children;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_place_feature;
#[cfg(test)]
#[path = "game/modding/native_place_feature_tests.rs"]
mod native_place_feature_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_placement_state_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_attack;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_break;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_cooldowns;
#[path = "game/modding/native_player_death.rs"]
mod native_player_death;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_destroyed_block_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_disconnect;
#[cfg(test)]
#[path = "game/modding/native_player_disconnect_tests.rs"]
mod native_player_disconnect_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_ender_chest;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_ender_chest_tests;
#[path = "game/modding/native_player_food.rs"]
mod native_player_food;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_player_inventory_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_inventory;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_level_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_modes;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_move_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_moves;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_persistent_data;
#[cfg(test)]
#[path = "game/modding/native_player_persistent_data_tests.rs"]
mod native_player_persistent_data_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_place;
#[cfg(test)]
#[path = "game/modding/native_player_place_tests.rs"]
mod native_player_place_tests;
#[path = "game/modding/native_player_reach.rs"]
mod native_player_reach;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_respawn;
#[cfg(test)]
#[path = "game/modding/native_player_respawn_tests.rs"]
mod native_player_respawn_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_spectating_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_stats;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_visibility;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_player_visibility_tests;
#[path = "game/modding/native_pool_elements.rs"]
mod native_pool_elements;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_recipes;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_portal_destination;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_profiles;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_projectile_launch;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_projectile_launch_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_projectile_refs;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_projectiles;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_quick_move;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_raid_refs;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_recipe_sync;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_recipe_sync_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_registry_entries;
#[cfg(test)]
#[path = "game/modding/native_registry_entries_tests.rs"]
mod native_registry_entries_tests;
#[cfg(test)]
#[path = "game/modding/native_registry_history_tests.rs"]
mod native_registry_history_tests;
#[path = "game/modding/native_registry_ids.rs"]
mod native_registry_ids;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_registry_ids_tests;
#[cfg(test)]
#[path = "game/modding/native_registry_processor_tests.rs"]
mod native_registry_processor_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_render_pair_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_render_visibility;
#[cfg(test)]
#[path = "game/modding/native_requires_tool_tests.rs"]
mod native_requires_tool_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_resources;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_ride_player;
#[path = "game/modding/native_rotate.rs"]
pub(crate) mod native_rotate;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_scene;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_scene_live;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_scene_live_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_section_visibility;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_scoreboard;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_scoreboard_teams;
#[path = "game/modding/native_server.rs"]
mod native_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_server_distances;
#[path = "game/modding/native_server_max_players.rs"]
mod native_server_max_players;
#[cfg(test)]
#[path = "game/modding/native_server_max_players_tests.rs"]
mod native_server_max_players_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_shaders;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_small_reads_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_shield_totem_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_singleplayer_owner;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_singleplayer_owner_tests;
#[cfg(test)]
#[path = "game/modding/native_smoke_source_tests.rs"]
mod native_smoke_source_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_sound_refs;
#[cfg(test)]
#[path = "game/modding/native_sound_seed_tests.rs"]
mod native_sound_seed_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_special_renderers;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_spectator_shader_tests;
#[path = "game/modding/native_state_shapes.rs"]
mod native_state_shapes;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_structure_placement;
#[path = "game/modding/native_structure_types.rs"]
mod native_structure_types;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_structures;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_success_broadcast_tests;
#[path = "game/modding/native_surface.rs"]
pub(crate) mod native_surface;
#[path = "game/modding/native_tags.rs"]
mod native_tags;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_terrain_model;
#[path = "game/modding/native_text_json.rs"]
mod native_text_json;
#[path = "game/modding/native_text_legacy.rs"]
mod native_text_legacy;
#[path = "game/modding/native_text_nbt.rs"]
mod native_text_nbt;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_tick_time;
#[path = "game/modding/native_ticks.rs"]
pub(in crate::game) mod native_ticks;
#[cfg(test)]
#[path = "game/modding/native_tnt_renderers_tests.rs"]
mod native_tnt_renderers_tests;
#[path = "game/modding/native_trapdoor_ladder.rs"]
pub(crate) mod native_trapdoor_ladder;
#[path = "game/modding/native_tree_grow.rs"]
pub(crate) mod native_tree_grow;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_viewed_levels;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_visuals;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_unspawned_mob;
#[path = "game/modding/native_world_columns.rs"]
mod native_world_columns;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_world_edit;
#[allow(unused_imports)]
pub(in crate::game) use server_side::native_world_read;
#[path = "game/modding/native_worldgen.rs"]
mod native_worldgen;
#[path = "game/modding/native_worldgen_parts.rs"]
pub(crate) mod native_worldgen_parts;
#[allow(unused_imports)]
pub(in crate::game) use client_side::nautili;
#[allow(unused_imports)]
pub(in crate::game) use server_side::natural_spawning;
#[cfg(test)]
#[path = "game/interface/neighbor_notify_tests.rs"]
mod neighbor_notify_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::net_harness;
#[allow(unused_imports)]
pub(in crate::game) use client_side::no_resources;
#[allow(unused_imports)]
pub(in crate::game) use client_side::options;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::options_close_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::neoforge_block_maps;
#[allow(unused_imports)]
pub(in crate::game) use server_side::neoforge_mob_maps;
#[allow(unused_imports)]
pub(in crate::game) use server_side::nether_portal;
#[allow(unused_imports)]
pub(in crate::game) use server_side::ominous_item_spawner;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::orb_data_tests;
#[cfg(test)]
#[path = "game/modding/orb_merge_tests.rs"]
mod orb_merge_tests;
#[cfg(test)]
#[path = "game/entities/orb_score_store_tests.rs"]
mod orb_score_store_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::pack_clouds_frame_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::pack_sources_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::packs;
#[allow(unused_imports)]
pub(in crate::game) use client_side::pad;
#[allow(unused_imports)]
pub(in crate::game) use client_side::pad_actions;
#[allow(unused_imports)]
pub(in crate::game) use client_side::pad_craft;
#[allow(unused_imports)]
pub(in crate::game) use client_side::pad_menu;
#[allow(unused_imports)]
pub(in crate::game) use server_side::ordered_tick;
#[allow(unused_imports)]
pub(in crate::game) use server_side::original_player;
#[allow(unused_imports)]
pub(in crate::game) use server_side::owner_link;
#[allow(unused_imports)]
pub(in crate::game) use server_side::pack_finders;
#[allow(unused_imports)]
pub(in crate::game) use server_side::paintings;
#[allow(unused_imports)]
pub(in crate::game) use server_side::pandas;
#[cfg(sunrise_parity_fixtures)]
#[path = "game/parity_fixtures.rs"]
pub mod parity_fixtures;
#[path = "game/modding/part_admission.rs"]
mod part_admission;
#[cfg(test)]
#[path = "game/modding/part_admission_tests.rs"]
mod part_admission_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::particle_culling;
#[path = "game/modding/payload_transport.rs"]
pub(in crate::game) mod payload_transport;
#[allow(unused_imports)]
pub(in crate::game) use client_side::perf_link;
#[allow(unused_imports)]
pub(in crate::game) use client_side::perspective;
#[allow(unused_imports)]
pub(in crate::game) use client_side::pick_accumulator;
#[allow(unused_imports)]
pub(in crate::game) use client_side::pick_entity;
#[allow(unused_imports)]
pub(in crate::game) use client_side::pickups;
#[allow(unused_imports)]
pub(in crate::game) use server_side::part_shearing;
#[allow(unused_imports)]
pub(in crate::game) use server_side::passengers;
#[allow(unused_imports)]
pub(in crate::game) use server_side::pending_births;
#[allow(unused_imports)]
pub(in crate::game) use server_side::perf_remote;
#[allow(unused_imports)]
pub(in crate::game) use server_side::persist;
#[allow(unused_imports)]
pub(in crate::game) use server_side::pick_hotbar;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::piglin_admire_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::piglin_conversion_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::piglins;
#[path = "game/blocks/piston_carry.rs"]
mod piston_carry;
#[allow(unused_imports)]
pub(in crate::game) use server_side::piston_carry_state;
#[path = "game/world/piston_nbt.rs"]
mod piston_nbt;
#[path = "game/blocks/piston_push.rs"]
mod piston_push;
#[allow(unused_imports)]
pub(in crate::game) use client_side::play_input;
#[allow(unused_imports)]
pub(in crate::game) use client_side::play_keys;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::place_feature_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::play_ping_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::player_destroy;
#[cfg(test)]
#[path = "game/items/player_destroy_item_tests.rs"]
mod player_destroy_item_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::player_destroy_tests;
#[path = "game/entities/player_equip.rs"]
pub(in crate::game) mod player_equip;
#[cfg(test)]
#[path = "game/modding/player_equipment_change_tests.rs"]
mod player_equipment_change_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::player_hit_sources;
#[cfg(test)]
#[path = "game/network/player_kill_credit_tests.rs"]
mod player_kill_credit_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::pointer_capture;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::polytone_frame_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::player_residency;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::player_residency_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::player_save_projection_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::player_spawn;
#[cfg(test)]
#[path = "game/world/portal_ticking_tests.rs"]
mod portal_ticking_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::potions;
#[path = "game/blocks/powder_snow.rs"]
pub(in crate::game) mod powder_snow;
#[allow(unused_imports)]
pub(in crate::game) use client_side::profile_capture;
#[allow(unused_imports)]
pub(in crate::game) use server_side::postprocess;
#[allow(unused_imports)]
pub(in crate::game) use server_side::potent_sulfur;
#[path = "game/items/primary_items.rs"]
pub(in crate::game) mod primary_items;
#[path = "game/interface/profile_lines.rs"]
mod profile_lines;
#[allow(unused_imports)]
pub(in crate::game) use server_side::projectile_block_impacts;
#[allow(unused_imports)]
pub(in crate::game) use server_side::projectile_spawned;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::projectile_spawned_tests;
#[path = "game/items/projectile_spawned_validation.rs"]
mod projectile_spawned_validation;
#[cfg(test)]
#[path = "game/modding/published_collision_mob_tests.rs"]
mod published_collision_mob_tests;
#[path = "game/modding/published_loops.rs"]
mod published_loops;
#[cfg(test)]
#[path = "game/modding/published_shape_game_tests.rs"]
mod published_shape_game_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::pvp;
#[allow(unused_imports)]
pub(in crate::game) use client_side::quick_actions;
#[allow(unused_imports)]
pub(in crate::game) use server_side::pumpkin;
#[allow(unused_imports)]
pub(in crate::game) use server_side::raids;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::rail_parts_tests;
#[cfg(test)]
#[path = "game/blocks/random_order_tests.rs"]
mod random_order_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::random_textures;
#[path = "game/network/raw_backlog.rs"]
mod raw_backlog;
#[allow(unused_imports)]
pub(in crate::game) use client_side::recipe_book;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::recipe_properties_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::recipe_book_variants_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::rectangular_recipe_remainder_tests;
#[path = "game/modding/registration_capability_coverage.rs"]
mod registration_capability_coverage;
#[path = "game/network/remote/mod.rs"]
mod remote;
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_block_writes;
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_camera;
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_chat;
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_chunks;
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_mob_data;
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_mob_particles;
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_mob_use;
#[cfg(test)]
#[path = "game/network/remote_mob_use_tests.rs"]
mod remote_mob_use_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_objects;
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_projectiles;
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_removal;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_save_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_stations;
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_tags;
#[allow(unused_imports)]
pub(in crate::game) use client_side::remote_vehicles;
#[allow(unused_imports)]
pub(in crate::game) use client_side::render_control;
#[allow(unused_imports)]
pub(in crate::game) use client_side::render_settings;
#[allow(unused_imports)]
pub(in crate::game) use client_side::render_tick;
#[allow(unused_imports)]
pub(in crate::game) use client_side::replay;
#[allow(unused_imports)]
pub(in crate::game) use client_side::repro;
#[path = "game/entities/residency_tick.rs"]
mod residency_tick;
#[path = "game/entities/resident_state.rs"]
mod resident_state;
#[path = "game/runtime/resource_load.rs"]
mod resource_load;
#[allow(unused_imports)]
pub(in crate::game) use client_side::resource_loader;
#[path = "game/runtime/resource_tables.rs"]
mod resource_tables;
#[allow(unused_imports)]
pub(in crate::game) use server_side::respawn_anchor;
#[allow(unused_imports)]
pub(in crate::game) use server_side::respawn_position;
#[cfg(test)]
#[path = "game/blocks/respawn_position_tests.rs"]
mod respawn_position_tests;
#[cfg(test)]
#[path = "game/modding/rule_replacements.rs"]
mod rule_replacements;
#[allow(unused_imports)]
pub(in crate::game) use server_side::runtime_levels;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::runtime_levels_tests;
#[cfg(test)]
#[path = "game/runtime/runtime_reset_tests.rs"]
mod runtime_reset_tests;
#[cfg(test)]
#[path = "game/world/save_format_tests.rs"]
mod save_format_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::saves_control;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::save_load_pass_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::save_pass_tests;
#[path = "game/world/saving.rs"]
mod saving;
#[path = "game/modding/scheduled_tasks.rs"]
mod scheduled_tasks;
#[path = "game/interface/screen_backgrounds.rs"]
pub(in crate::game) mod screen_backgrounds;
#[allow(unused_imports)]
pub(in crate::game) use client_side::screen_build;
#[allow(unused_imports)]
pub(in crate::game) use client_side::screen_build_fixture;
#[allow(unused_imports)]
pub(in crate::game) use server_side::scoreboard;
#[path = "game/interface/screen_info.rs"]
mod screen_info;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::screen_render_phase_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::screen_room;
#[allow(unused_imports)]
pub(in crate::game) use client_side::screen_widgets;
#[allow(unused_imports)]
pub(in crate::game) use client_side::screens;
#[allow(unused_imports)]
pub(in crate::game) use client_side::screenshot_sequence;
#[allow(unused_imports)]
pub(in crate::game) use client_side::screenshot_writer;
#[allow(unused_imports)]
pub(in crate::game) use client_side::sculk;
#[allow(unused_imports)]
pub(in crate::game) use client_side::server_links;
#[allow(unused_imports)]
pub(in crate::game) use client_side::server_packs;
#[allow(unused_imports)]
pub(in crate::game) use server_side::sculk_fixture;
#[allow(unused_imports)]
pub(in crate::game) use server_side::sculk_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::seats;
#[allow(unused_imports)]
pub(in crate::game) use server_side::server_lifecycle;
#[allow(unused_imports)]
pub(in crate::game) use server_side::server_reload;
#[path = "game/server_side.rs"]
pub(in crate::game) mod server_side;
#[allow(unused_imports)]
pub(in crate::game) use server_side as server_state;
#[path = "game/blocks/block_entity_anims_common.rs"]
pub(in crate::game) mod block_entity_anims_common;
#[path = "game/world/light_emission.rs"]
pub(in crate::game) mod light_emission;
#[allow(unused_imports)]
pub(in crate::game) use server_side::server_travel;
#[cfg(test)]
#[path = "game/administration/setblock_order_tests.rs"]
mod setblock_order_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::shader_pack_switch_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::shader_packs;
#[allow(unused_imports)]
pub(in crate::game) use client_side::shelf;
#[allow(unused_imports)]
pub(in crate::game) use client_side::sign_edit;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::shear_refind_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::shield;
#[allow(unused_imports)]
pub(in crate::game) use server_side::shoulders;
#[allow(unused_imports)]
pub(in crate::game) use server_side::slot_changes;
#[cfg(test)]
#[path = "game/modding/slot_pickup_tests.rs"]
mod slot_pickup_tests;
#[path = "game/modding/slot_rules.rs"]
mod slot_rules;
#[cfg(test)]
#[path = "game/modding/slot_rules_tests.rs"]
mod slot_rules_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::slot_take_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::slot_takes;
#[path = "game/interface/smithing_icons.rs"]
mod smithing_icons;
#[allow(unused_imports)]
pub(in crate::game) use client_side::sniffers;
#[allow(unused_imports)]
pub(in crate::game) use client_side::social;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::spawn_save_tests;
#[cfg(test)]
#[path = "game/mobs/spawn_yaw_tests.rs"]
mod spawn_yaw_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::spawner_client_tick_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::spectator;
#[allow(unused_imports)]
pub(in crate::game) use server_side::spawner;
#[path = "game/items/spear.rs"]
pub(in crate::game) mod spear;
#[allow(unused_imports)]
pub(in crate::game) use client_side::stack_attribute_tooltip;
#[path = "game/modding/stacked_on_other.rs"]
pub(in crate::game) mod stacked_on_other;
#[allow(unused_imports)]
pub(in crate::game) use server_side::spear_players;
#[allow(unused_imports)]
pub(in crate::game) use server_side::special_attack_fixture;
#[allow(unused_imports)]
pub(in crate::game) use server_side::special_attacks;
#[allow(unused_imports)]
pub(in crate::game) use server_side::spill;
#[allow(unused_imports)]
pub(in crate::game) use server_side::splash_water;
#[cfg(test)]
#[path = "game/modding/stacked_on_other_tests.rs"]
mod stacked_on_other_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::stand_break_loot_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::stand_command_hurt_tests;
#[cfg(test)]
#[path = "game/interface/stand_equipment_change_tests.rs"]
mod stand_equipment_change_tests;
#[cfg(test)]
#[path = "game/entities/stand_low_health_tests.rs"]
mod stand_low_health_tests;
#[cfg(test)]
#[path = "game/modding/start_riding_refusal_tests.rs"]
mod start_riding_refusal_tests;
#[cfg(test)]
#[path = "game/runtime/startup_cpu_tests.rs"]
mod startup_cpu_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::startup_menu;
#[path = "game/interface/startup_options.rs"]
mod startup_options;
#[path = "game/runtime/startup_progress.rs"]
pub(crate) mod startup_progress;
#[cfg(test)]
#[path = "game/interface/state_at_viewpoint_tests.rs"]
mod state_at_viewpoint_tests;
#[cfg(feature = "state-digest")]
#[allow(unused_imports)]
pub(in crate::game) use server_side::state_digest;
#[path = "game/modding/state_resolver.rs"]
mod state_resolver;
#[cfg(test)]
#[path = "game/runtime/stateful_block_entity_tests.rs"]
mod stateful_block_entity_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::station_control;
#[allow(unused_imports)]
pub(in crate::game) use client_side::stations;
#[allow(unused_imports)]
pub(in crate::game) use client_side::stats_screen;
#[allow(unused_imports)]
pub(in crate::game) use client_side::storage;
#[allow(unused_imports)]
pub(in crate::game) use client_side::structure_edit;
#[allow(unused_imports)]
pub(in crate::game) use server_side::stats;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::storage_alias_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::structure_blocks;
#[path = "game/administration/structures.rs"]
mod structures;
#[allow(unused_imports)]
pub(in crate::game) use client_side::studio;
#[allow(unused_imports)]
pub(in crate::game) use client_side::subtitle_hud;
#[allow(unused_imports)]
pub(in crate::game) use client_side::subtitles;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::sulfur_bucket_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::sulfur_cubes;
#[cfg(test)]
#[path = "game/mobs/sulfur_prime_phase_tests.rs"]
mod sulfur_prime_phase_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::sync_background_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::test_block_edit;
#[allow(unused_imports)]
pub(in crate::game) use client_side::test_instance_edit;
#[allow(unused_imports)]
pub(in crate::game) use client_side::text_displays;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::text_expression_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::tick_script;
#[allow(unused_imports)]
pub(in crate::game) use client_side::titles;
#[allow(unused_imports)]
pub(in crate::game) use server_side::table_enchant;
#[allow(unused_imports)]
pub(in crate::game) use server_side::test_blocks;
#[path = "game/administration/test_entities.rs"]
pub(in crate::game) mod test_entities;
#[path = "game/interface/toast_manager.rs"]
pub(in crate::game) mod toast_manager;
#[allow(unused_imports)]
pub(in crate::game) use server_side::test_environments;
#[allow(unused_imports)]
pub(in crate::game) use server_side::test_instances;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::test_pos_guest_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::tnt;
#[path = "game/blocks/tool_modification.rs"]
mod tool_modification;
#[allow(unused_imports)]
pub(in crate::game) use client_side::advancements_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::block_entity_anims_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::boats_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::cauldron_prediction_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::commands_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::creatures_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::dragon_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::end_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::entity_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::entity_sizes_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::fangs_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::fireworks_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::fishing_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::game_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::game_mode_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::integrated_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::level_particles_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::machines_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::memory_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_book_net_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_book_placement_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_book_settings_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_entity_refs_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_hooks_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::native_living_effects_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::ominous_item_spawner_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::paintings_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::player_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::saving_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::spear_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::swing_packet;
#[allow(unused_imports)]
pub(in crate::game) use client_side::thrown_item_draw;
#[allow(unused_imports)]
pub(in crate::game) use client_side::tnt_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::totem;
#[allow(unused_imports)]
pub(in crate::game) use client_side::transactions_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::transient_blocks;
#[allow(unused_imports)]
pub(in crate::game) use client_side::trial_visuals;
#[allow(unused_imports)]
pub(in crate::game) use client_side::trident_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::tutorial;
#[allow(unused_imports)]
pub(in crate::game) use client_side::using_item_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::world_view_client;
#[allow(unused_imports)]
pub(in crate::game) use client_side::zombie_villagers_client;
#[allow(unused_imports)]
pub(in crate::game) use server_side::books_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::chat_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::command_blocks_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::command_minecarts_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::commands_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::creatures_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::game_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::hand_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::lightning_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::player_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::sign_edit_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::spawner_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::trading;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::transaction_entities_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::transactions_server;
#[allow(unused_imports)]
pub(in crate::game) use server_side::world_update_server;
#[path = "game/runtime/transactions.rs"]
pub(in crate::game) mod transactions;
#[allow(unused_imports)]
pub(in crate::game) use server_side::trial_spawner;
#[allow(unused_imports)]
pub(in crate::game) use server_side::trident;
#[path = "game/modding/typed_payloads.rs"]
mod typed_payloads;
#[allow(unused_imports)]
pub(in crate::game) use client_side::ui_control;
#[cfg(test)]
#[path = "game/runtime/ui_core_shader_tests.rs"]
mod ui_core_shader_tests;
#[path = "game/interface/upload_pace.rs"]
mod upload_pace;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::use_same_item_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::viewed_worlds;
#[allow(unused_imports)]
pub(in crate::game) use server_side::use_hand;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::use_stop_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::using_item;
#[allow(unused_imports)]
pub(in crate::game) use server_side::vault;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::vehicle_removal_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::vehicle_teleport;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::vibration_source_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::vibration_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::view_pass;
#[path = "game/network/villager_client.rs"]
mod villager_client;
#[allow(unused_imports)]
pub(in crate::game) use server_side::villager_professions;
#[allow(unused_imports)]
pub(in crate::game) use server_side::villagers;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::visibility_unwind_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::wake_time;
#[allow(unused_imports)]
pub(in crate::game) use server_side::wandering_trader;
#[path = "game/blocks/waxing.rs"]
mod waxing;
#[cfg(test)]
#[path = "game/blocks/waxing_tests.rs"]
mod waxing_tests;
#[path = "game/world/weather.rs"]
mod weather;
#[allow(unused_imports)]
pub(in crate::game) use client_side::win;
#[allow(unused_imports)]
pub(in crate::game) use client_side::window_handler;
#[allow(unused_imports)]
pub(in crate::game) use client_side::window_title;
#[allow(unused_imports)]
pub(in crate::game) use server_side::wind;
#[allow(unused_imports)]
pub(in crate::game) use server_side::wither;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::wither_star_tests;
#[path = "game/world/world_entry.rs"]
pub(in crate::game) mod world_entry;
#[cfg(test)]
#[path = "game/interface/world_fluid_tests.rs"]
mod world_fluid_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::world_name_tag_tests;
#[allow(unused_imports)]
pub(in crate::game) use client_side::world_rest;
#[allow(unused_imports)]
pub(in crate::game) use client_side::world_session;
#[allow(unused_imports)]
pub(in crate::game) use client_side::world_update;
#[allow(unused_imports)]
pub(in crate::game) use server_side::world_fluids;
#[allow(unused_imports)]
pub(in crate::game) use server_side::world_loading;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::world_test_def_tests;
#[path = "game/items/worn_boots.rs"]
mod worn_boots;
pub use hot_paths::{HotPathsMods, hot_paths_next_block_entity, note_hot_paths_listener_call};
#[allow(unused_imports)]
pub(in crate::game) use server_side::xp_orb_targeting;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use server_side::xp_repair_ratio_tests;
#[allow(unused_imports)]
pub(in crate::game) use server_side::zombie_reinforcements;
#[allow(unused_imports)]
pub(in crate::game) use server_side::zombie_villagers;
#[path = "game/modding/api_values_common.rs"]
pub(in crate::game) mod api_values_common;
#[path = "game/blocks/beacon_common.rs"]
pub(in crate::game) mod beacon_common;
#[path = "game/mobs/beehives_common.rs"]
pub(in crate::game) mod beehives_common;
#[path = "game/blocks/block_edits_common.rs"]
pub(in crate::game) mod block_edits_common;
#[path = "game/blocks/block_props.rs"]
pub(in crate::game) mod block_props;
#[path = "game/entities/boats_common.rs"]
pub(in crate::game) mod boats_common;
#[path = "game/blocks/bookshelves_common.rs"]
pub(in crate::game) mod bookshelves_common;
#[path = "game/mobs/dragon_common.rs"]
pub(in crate::game) mod dragon_common;
#[path = "game/entities/entity_common.rs"]
pub(in crate::game) mod entity_common;
#[path = "game/modding/entity_initialization_common.rs"]
pub(in crate::game) mod entity_initialization_common;
#[path = "game/modding/entity_sizes_common.rs"]
pub(in crate::game) mod entity_sizes_common;
#[path = "game/mobs/fangs_common.rs"]
pub(in crate::game) mod fangs_common;
#[path = "game/blocks/jukebox_common.rs"]
pub(in crate::game) mod jukebox_common;
#[path = "game/modding/mod_api_host_common.rs"]
pub(in crate::game) mod mod_api_host_common;
#[path = "game/mobs/mounts_common.rs"]
pub(in crate::game) mod mounts_common;
#[path = "game/modding/native_bots_common.rs"]
pub(in crate::game) mod native_bots_common;
#[path = "game/modding/native_harvest_queries_common.rs"]
pub(in crate::game) mod native_harvest_queries_common;
#[path = "game/modding/native_lifecycle_common.rs"]
pub(in crate::game) mod native_lifecycle_common;
#[path = "game/modding/native_menu_refs_common.rs"]
pub(in crate::game) mod native_menu_refs_common;
#[path = "game/mobs/ominous_item_spawner/common.rs"]
pub(in crate::game) mod ominous_item_spawner_common;
#[path = "game/modding/pack_finders/common.rs"]
pub(in crate::game) mod pack_finders_common;
#[path = "game/entities/passengers_common.rs"]
pub(in crate::game) mod passengers_common;
#[path = "game/entities/pick_entity_common.rs"]
pub(in crate::game) mod pick_entity_common;
#[path = "game/mobs/piglins_common.rs"]
pub(in crate::game) mod piglins_common;
#[path = "game/blocks/potent_sulfur_common.rs"]
pub(in crate::game) mod potent_sulfur_common;
#[path = "game/blocks/shelf_common.rs"]
pub(in crate::game) mod shelf_common;
#[path = "game/blocks/stations_common.rs"]
pub(in crate::game) mod stations_common;
#[path = "game/world/storage_common.rs"]
pub(in crate::game) mod storage_common;
#[allow(unused_imports)]
use sunrise_client_gameplay::frame_autotune;
#[allow(unused_imports)]
use sunrise_client_gameplay::occlusion_culling;
#[allow(unused_imports)]
use sunrise_gameplay::lan::idle as lan_idle;
#[allow(unused_imports)]
use sunrise_gameplay::{interaction_shapes, item_areas, mob_index, tick_profile};

#[allow(unused_imports)]
use client_state::ClientState;
#[allow(unused_imports)]
use event_loop::EventLoopCtl;
#[allow(unused_imports)]
use hand::interact_location;
pub(crate) use hit::Hit;
#[allow(unused_imports)]
use loading_window::{
    build_renderer, loading_frame, loading_renderer, prepare_renderer_textures, start_client_game,
};
#[allow(unused_imports)]
use resource_loader::{
    EntityModels, GameCpuResources, Loaded, LoaderMsg, PendingWorldEntry, WorldRestLoader,
    run_loading, write_report,
};
#[allow(unused_imports)]
use server_side::loading_server::start_dedicated_game;
#[allow(unused_imports)]
use server_state::ServerState;
#[allow(unused_imports)]
use sunrise_client::random_source;
#[allow(unused_imports)]
use sunrise_client_render::block_entity_reach::Reach;
// `Action` here is the title screen's buttons (`title::Action`).
#[allow(unused_imports)]
use self::api_values::TryIntoApi;
pub(crate) use api_values::{block_jump_factor, block_speed_factor, is_ladder};
pub(crate) use launch_options::Options;
pub use native_block_entity_draw::BlockEntityDraw;
pub use native_configs::ConfigFile;
pub use native_data_maps::DataMapType;
pub use native_hooks::{PhantomSpawnResult, Shake, SleepProblem};
pub use native_mob_draw::{MobRender, PlayerRender};
#[allow(unused_imports)]
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
        mpsc::{Receiver, TryRecvError},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
#[allow(unused_imports)]
use sunrise_client::actions::Action as Mapping;
#[allow(unused_imports)]
use sunrise_client::held::Source;
#[allow(unused_imports)]
use sunrise_client::spans::{startup, time as span};
#[allow(unused_imports)]
use sunrise_client::{
    Result,
    assets::{Assets, RenderAssets},
    audio::Audio,
    biome::Biomes,
    block_entity_render::{
        self, BlockEntityRenderer, CacheOptions, Scene as BlockEntityScene, StaticCacheFrameStats,
        StaticSectionSnapshot,
    },
    blocks::{BlockFacts, Fluid},
    clock::FixedStep,
    collisions,
    containers::{BlockEntity, Providers},
    crafting::Recipes,
    creative::{Creative, Names},
    entities::{Experience, ExperienceOrb, ItemEntity},
    environment::Environment,
    first_person::{Hand, SwingAnimation},
    furnace::FurnaceCooking,
    gfx::{Frame, Renderer},
    graphics::EntityCullingMode,
    hud::{HudAtlas, HudBatch, rgba},
    hud_layout, hud_overlays,
    items::{ItemStack, Items},
    keymapping::{KeyAction, SNEAK, SPRINT},
    light::LightTables,
    mesher::{MeshTables, Vertex},
    mobs::{Mob, MobType, MobTypes},
    models,
    motion::{MotionResources, PlayerMotion},
    mouse::{LookOptions, MouseLook},
    particles::Particle,
    save::{Level, Persistent, SaveDir},
    settings::Settings,
    spans,
    sprint::Keys,
    streaming::{Streaming, WorkerContext},
    testgen::TestTerrain,
    title::{Action, Title},
    vanillagen::VanillaTerrain,
    world::World,
};
#[allow(unused_imports)]
use sunrise_core::{
    biome::{BiomeZoom, obfuscate_seed},
    geometry::{Aabb, Direction},
    position::BlockPos,
};
#[allow(unused_imports)]
use sunrise_core::{geometry::Vec3, random::RandomSource};
#[allow(unused_imports)]
use sunrise_mod_package::selection::{Argument, LaunchArguments};
#[allow(unused_imports)]
use winit::{
    application::ApplicationHandler,
    error::EventLoopError,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};
#[allow(unused_imports)]
use {
    sunrise_core::dimension::Dimension as LevelDimension,
    sunrise_world::{
        block_collision::BlockCollisionRegistry,
        collision_context::{CollisionContext, CollisionEntity, CollisionItem},
        entity_move::MoverType,
        entity_obstruction::{EntityIdentity, EntityObstructionWorld, ObstructionEntity},
        fluid::FluidState,
        level::{
            Behaviours, BucketContent, EntityQuery, Landing, LevelAccess, LevelEvent, PlaceContext,
            PlacementContext, PlayerContext, PlayerEffect, RandomTickMaskUpdate, Ticks,
            flags as level_flags,
        },
        registry::BlockStateId,
    },
};

/// A frame's finished meshes up to this many upload at once, unqueued.
const AUTO_UPLOADS_AT_ONCE: usize = 8;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Playing,
    GameModeSwitcher,
    Paused,
    Options,
    Chat,
    Creative,
    Inventory,
    Dead,
    /// The Statistics screen.
    Stats,
    /// The Advancements screen.
    Advancements,
    /// WorldOptionsScreen (game/network/lan_screen.rs).
    WorldOptions,
    /// Play with Friends, from the pause menu's Friends button
    /// (`friends_screen`, game/network/friends.rs).
    Friends,
    /// `SocialInteractionsScreen` (`social_screen`, game/network/social.rs).
    Social,
    /// CommandBlockEditScreen (game/administration/command_blocks.rs).
    CommandBlock,
    /// A game master block's edit screen (game/interface/block_editor.rs).
    BlockEditor,
    /// ServerLinksDialogScreen: a server's links (game/network/server_links.rs).
    ServerLinks,
    /// ConfirmLinkScreen for a server link (game/network/server_links.rs).
    ConfirmLink,
    /// Parentless PopupScreen opened by the server's demo event.
    DemoIntro,
    /// DialogScreen.WarningScreen: the note about custom screens.
    DialogWarning,
    /// WinScreen: the end poem and the credits (game/interface/win.rs).
    Win,
    /// DebugOptionsScreen, F3+F6 (game/interface/debug_options.rs).
    DebugOptions,
    /// A screen of a native mod's own (`mod_screen_open`): it draws and
    /// takes its input through the screen hooks.
    Mod,
    /// PvP & HUD's widget editor (game/interface/hud_editor.rs).
    HudEditor,
}
impl Screen {
    /// The id native mods see for this screen (`ScreenView.id`, the
    /// `screen_render_post` hook): one per variant, and a hook's contract, so
    /// it stays the same from release to release. A new variant must add one.
    fn id(self) -> &'static str {
        match self {
            Screen::Playing => "playing",
            Screen::GameModeSwitcher => "game_mode_switcher",
            Screen::Paused => "paused",
            Screen::Options => "options",
            Screen::Chat => "chat",
            Screen::Creative => "creative",
            Screen::Inventory => "inventory",
            Screen::Dead => "dead",
            Screen::Stats => "stats",
            Screen::Advancements => "advancements",
            Screen::WorldOptions => "world_options",
            Screen::Friends => "friends",
            Screen::Social => "social",
            Screen::CommandBlock => "command_block",
            Screen::BlockEditor => "block_editor",
            Screen::ServerLinks => "server_links",
            Screen::ConfirmLink => "confirm_link",
            Screen::DemoIntro => "demo_intro",
            Screen::DialogWarning => "dialog_warning",
            Screen::Win => "win",
            Screen::DebugOptions => "debug_options",
            Screen::Mod => "mod",
            Screen::HudEditor => "hud_editor",
        }
    }
}
/// The vanilla `state` a `transformer` table entry offers to `ability`: hoe and
/// shovel tables have one action each; the axe's strip, scrape and wax-off lists
/// are one untagged list in `BlockTransformers.bootstrap`, so its particle names it.
fn vanilla_tool_state(
    transformer: &str,
    particle: &str,
    ability: mod_api::ToolAbility,
    state: BlockStateId,
) -> Option<BlockStateId> {
    use mod_api::ToolAbility::*;
    let action = match transformer {
        "minecraft:axe" => match particle {
            "scrape" => AxeScrape,
            "wax_off" => AxeWaxOff,
            _ => AxeStrip,
        },
        "minecraft:shovel" => ShovelFlatten,
        "minecraft:hoe" => HoeTill,
        _ => return None,
    };
    (action == ability).then_some(state)
}

struct Mining {
    pos: [i32; 3],
    progress: f32,
    /// The latest destroy-speed query, reused for server crack feedback.
    per_tick: f32,
    ticks: u32,
    /// The stage's display-only answer, dropped with this mining state.
    crack: Option<client_cracks::ClientCrack>,
}
impl Mining {
    fn stage(&self) -> Option<u8> {
        (self.progress > 0.0).then(|| ((self.progress * 10.0) as u8).min(9))
    }
}
#[derive(Clone, Copy)]
struct TransformContext<'a> {
    world: &'a World,
    player: Option<u32>,
    hand: mod_api::Hand,
    blocking: bool,
    simulated: bool,
}

/// What a falling block's tick did (`FallingBlockEntity.tick`).
enum FallStep {
    Falling,
    Gone,
    /// Gave up falling: drops itself as an item here.
    Dropped(Vec3),
    /// Hit the ground in this block.
    Landed([i32; 3]),
}
/// A guest's right click comes as one packet per hand and part
/// (`Minecraft.startUseItem`): `ServerGamePacketListenerImpl.handleUseItemOn`
/// does the block's and `ItemStack.useOn`'s part, `handleUseItem` the
/// item's own (`Item.use`). Running both for either would use the item twice.
#[derive(Clone, Copy)]
enum GuestUse {
    OnBlock {
        hand: Hand,
        block_fell_through: bool,
    },
    Item(Hand),
    /// `handleInteract`: `Player.interactOn` with the hand, nothing else (the
    /// client sends UseItem itself when the entity passed), on the entity the
    /// packet named: `ServerGamePacketListenerImpl.handleInteract` resolves
    /// `packet.entityId()` once and range checks it, so the click never picks
    /// an entity again from the view.
    Entity(Hand, InteractTarget),
}
impl GuestUse {
    fn on_block(hand: Hand) -> Self {
        Self::OnBlock {
            hand,
            block_fell_through: false,
        }
    }
}
/// The entity an interact packet names and where its client clicked it (the
/// packet's location, measured from the entity's position): what
/// `ServerGamePacketListenerImpl.handleInteract` takes from
/// `ServerboundInteractPacket` and hands to `Player.interactOn`, which passes
/// the point on to `interactAt`.
#[derive(Clone, Copy)]
struct InteractTarget {
    id: i32,
    location: [f64; 3],
}

/// The client world as seen by the vanilla block-behaviour library.
pub(in crate::game) struct GameLevel<'w> {
    cake_probe: bool,
    cake_requested: std::cell::Cell<bool>,
    cake_answer: Option<bool>,
    api_level: Option<mod_api::LevelRef>,
    part_state_index: Option<
        &'w std::sync::Arc<
            std::sync::Mutex<
                sunrise_mod_runtime::attachments::index::AttachmentIndex<mod_api::AttachmentHolder>,
            >,
        >,
    >,
    attachment_query_scopes: &'w sunrise_mod_runtime::attachments::AttachmentQueryScopes,
    positions: piston_carry_state::PositionState<'w>,
    border: &'w sunrise_core::geometry::border::BorderGeometry,
    world: &'w mut World,
    command_block_transition: Option<command_blocks::CommandBlockTransition>,
    redstone_experiments: bool,
    facts: &'w BlockFacts,
    /// The block-state facts a part reads through `LevelHost::block_states`.
    tables: native_block_tables::BlockTables<'w>,
    collision: &'w BlockCollisionRegistry<'w>,
    events: &'w mut Vec<LevelEvent>,
    dimension: LevelDimension,
    darken: i32,
    /// The environment of the level's dimension (`SUN_ANGLE`).
    environment: &'w Environment,
    raining: bool,
    thundering: bool,
    /// The world's enabled feature flags as this side knows them (`LevelReader.enabledFeatures`).
    enabled_features: &'w std::sync::Arc<[String]>,
    player_box: Option<([f64; 3], [f64; 3])>,
    /// The LAN guests in this level who trigger blocks (not spectators).
    guest_boxes: Vec<([f64; 3], [f64; 3])>,
    /// Other players that block placement during a guest turn, including live
    /// ancestor bodies; None outside one, where `guest_boxes` are those players.
    building_player_boxes: Option<Vec<([f64; 3], [f64; 3])>>,
    player_collision: Option<PlayerCollision>,
    /// Minecart boxes (detector rails).
    cart_boxes: Vec<([f64; 3], [f64; 3])>,
    /// The item frames a comparator reads through a conductor.
    item_frames: &'w [sunrise_client::entities::ItemFrame],
    /// The minecarts, in `cart_boxes` order, for a detector rail's comparator.
    carts: &'w mut sunrise_client::join_seq::JoinList<minecarts::Minecart, false>,
    /// Arrow and thrown trident boxes (wooden buttons).
    arrow_boxes: Vec<([f64; 3], [f64; 3])>,
    /// Every entity on the walk; see `entity::for_each_entity`.
    other_entities: entity::EntityLists<'w>,
    /// End crystals, which aren't on the walk yet.
    crystals: &'w [dragon::Crystal],
    /// The fishing hooks here, which aren't on the walk either. None ignores
    /// block triggers, so each counts on a plate or tripwire. The flag says
    /// whether it also reaches `entityInside` (`applyEffectsFromBlocks`): not
    /// a hook on an entity, whose tick skips that step.
    trigger_hooks: Vec<(Aabb, bool)>,
    /// The lightning bolts here: none ignores block triggers
    /// (`Entity.isIgnoringBlockTriggers`).
    trigger_extras: Vec<Aabb>,
    biomes: &'w Biomes,
    /// `BiomeManager`'s zoom over this level's seed, for `biome_name`.
    biome_zoom: sunrise_core::biome::BiomeZoom,
    seed: i64,
    behaviours: &'w Behaviours,
    items: &'w Items,
    /// Cooking recipes' experience, which a removed furnace pays out.
    recipes: &'w sunrise_client::crafting::Recipes,
    /// Container loot tables (`unpackLootTable`).
    loot: Option<std::sync::Arc<sunrise_world::loot::LootData>>,
    loot_storage: std::sync::Arc<sunrise_world::loot::copy::StorageData>,
    registry: &'static sunrise_world::registry::BlockRegistry,
    block_registries: &'w sunrise_client::command::args::Registries,
    rules: sunrise_world::level::GameRules,
    /// `Difficulty.getId()` (peaceful 0 .. hard 3).
    difficulty: i32,
    /// The feet of the players who keep fire spreading around them: the local
    /// player and the LAN guests in this level, spectators aside.
    fire_players: Vec<[f64; 3]>,
    /// Block loot random sequences (`ServerLevel.getRandomSequence`).
    rand_sequences: &'w mut sunrise_client::random_sequences::RandomSequences,
    pending: &'w mut spill::Pending,
    spill: &'w mut spill::Spill,
    /// Where the chunk tickets let blocks tick.
    tickets: sunrise_client::ticket_levels::Levels,
    random_tick_sections: &'w mut sunrise_client::random_tick_sections::RandomTickSections,
    /// Players with each chest open (a trapped chest's signal), also what a
    /// recount of the openers replaces (`ContainerOpenersCounter.recheckOpeners`).
    lid_openers: &'w mut HashMap<(i32, i32, i32), i32>,
    /// The containers players have open at this moment, by position: the local
    /// player's and the guests' (`getEntitiesWithContainerOpen`). A double
    /// chest is in it as both halves.
    open_containers: &'w [(i32, i32, i32)],
    /// The chests copper golems have open, a double chest as both halves. They
    /// keep their own count (`chest_openers`), so they stay out of `open_containers`.
    golem_chests: &'w [(i32, i32, i32)],
    /// `Level.isClientSide`: a client's copy of the level (a guest of a LAN
    /// world, a window on its server thread, a dedicated server's client).
    client_side: bool,
    /// Only the integrated or dedicated server dispatches native observations.
    native_authoritative: bool,
    native_player: Option<u128>,
    /// The acting player's entity id, beside `native_player`.
    native_player_entity: Option<i32>,
    /// Direct liquid part writes may not bypass an open Game transaction.
    liquid_transaction_open: bool,
    /// `Level.isDebug`: `setBlock` changes nothing.
    debug: bool,
    native_sounds: &'w mut Vec<creatures::NativeSound>,
    loop_context: &'w sunrise_world::native_load_context::LoopContext,
    native_particles: &'w mut Vec<crate::lan::server_effects::ServerQueuedParticle>,
    sound_seeds: Option<&'w sunrise_client::lan::hub::Hub>,
    drop_entity: Option<mod_api::EntityRef>,
    /// `LootContextParams.INTERACTING_ENTITY` of a block-interaction drop: the
    /// player whose click `use_without_item` runs for.
    interacting: Option<sunrise_world::loot::LootEntity>,
    /// What a location predicate of such a drop reads beyond the level's own
    /// fields (`blocks::location_world`).
    location: location_world::LocationRefs<'w>,
    /// The block a mod's machine breaks and the tool it breaks it with
    /// (`native_machine_destroy_block`): `Block.dropResources` rolls that
    /// block's loot with the tool instead of with none. Neighbours that pop
    /// off because of the break drop as with no tool, as in vanilla.
    drop_tool: Option<((i32, i32, i32), Option<ItemStack>)>,
    /// Only this destroyed block has an entity as its loot `this` parameter.
    /// Neighbour/default/machine drops keep the no-entity context.
    drop_breaker_at: Option<BlockPos>,
    /// Where `Block.dropResources` puts the drops instead of popping them,
    /// while a mod's machine breaks a block (`native_machine_destroy_block`).
    drop_into: Option<Vec<ItemStack>>,
}
pub(in crate::game) struct PlayerCollision {
    y: f64,
    descending: bool,
    fall_distance: f64,
    main_hand_item: Arc<str>,
    main_hand_count: i32,
    leather_boots: bool,
}
/// The boxes of the entities that block building, as obstruction candidates.
struct BuildingObstruction(Vec<Aabb>);
impl EntityObstructionWorld for BuildingObstruction {
    type Error = std::convert::Infallible;
    fn obstruction_candidates(
        &mut self,
        _excluded: Option<EntityIdentity>,
        bounds: Aabb,
    ) -> std::result::Result<Vec<ObstructionEntity>, Self::Error> {
        Ok(self
            .0
            .iter()
            .filter(|b| b.intersects(bounds))
            .enumerate()
            .map(|(i, &bounds)| ObstructionEntity {
                identity: EntityIdentity(i as u64),
                root_vehicle: EntityIdentity(i as u64),
                bounds,
                removed: false,
                spectator: false,
                blocks_building: true,
            })
            .collect())
    }
}
/// Whether a block's successful use is `InteractionResult.SUCCESS_SERVER`,
/// which a client does not predict: `useWithoutItem` of a bed, a light
/// block, a charged respawn anchor or a sign. Every other block use the
/// level answers with SUCCESS is `SUCCESS` (`SwingSource.PREDICTED`).
fn server_only_block_use(
    block: &sunrise_items::blocks::StateFacts,
    branch: sunrise_world::level::UseBranch,
) -> bool {
    branch == sunrise_world::level::UseBranch::WithoutItem
        && [
            "AbstractBedBlock",
            "LightBlock",
            "RespawnAnchorBlock",
            "SignBlock",
        ]
        .iter()
        .any(|class| block.has_class(class))
}
impl GameLevel<'_> {
    /// `LilyPadBlock.entityInside`: the boxes of the boats that can break a pad.
    fn boat_trigger_boxes(&self) -> impl Iterator<Item = Aabb> + '_ {
        self.entities().boats.iter().map(boats::Boat::bounds)
    }
    /// How many entities in `area` press plates and tripwires, or with
    /// `LIVING` only the living ones; a constant so a list of other kinds
    /// compiles to no loop at all.
    fn count_triggers<const LIVING: bool>(&self, area: Aabb) -> usize {
        use entity::{Entity as _, for_each_entity};
        let mut count = 0;
        for_each_entity!(self.entities(), entity => {
            if (!LIVING || entity.is_living())
                && entity.triggers_blocks()
                && area.intersects(entity.bounds())
            {
                count += 1;
            }
        });
        count
    }
    /// Every entity box that blocks building: the players who aren't
    /// spectators, the minecarts and the `Blockers` (a removed mob or a marker
    /// armor stand, whose box is empty, doesn't).
    fn building_blocker_boxes(&self) -> Vec<Aabb> {
        let boxed = |(lo, hi): &([f64; 3], [f64; 3])| {
            Aabb::new(
                Vec3::new(lo[0], lo[1], lo[2]),
                Vec3::new(hi[0], hi[1], hi[2]),
            )
        };
        let b = self.entities();
        self.player_box
            .iter()
            .chain(
                self.building_player_boxes
                    .as_ref()
                    .unwrap_or(&self.guest_boxes),
            )
            .map(boxed)
            .chain(b.mobs.iter().filter(|m| !m.removed).map(Mob::bounds))
            .chain(
                b.non_mob_building_blocker_boxes(self.cart_boxes.iter().map(boxed), self.crystals),
            )
            .collect()
    }
}

impl<'w> entity::EntityLists<'w> {
    /// Non-player, non-mob boxes with `Entity.blocksBuilding`. The block level
    /// supplies its minecart snapshot; the mob level reads the live cart boxes.
    fn non_mob_building_blocker_boxes(
        self,
        carts: impl Iterator<Item = Aabb> + 'w,
        crystals: &'w [dragon::Crystal],
    ) -> impl Iterator<Item = Aabb> + 'w {
        carts
            .chain(
                self.armor_stands
                    .iter()
                    .filter(|s| !s.marker)
                    .map(|s| s.bounds()),
            )
            .chain(self.boats.iter().map(boats::Boat::bounds))
            .chain(crystals.iter().map(dragon::Crystal::bounds))
            .chain(self.tnts.iter().map(tnt::PrimedTnt::bounds))
            .chain(
                self.falling
                    .iter()
                    .filter(|f| f.blocks_building)
                    .map(FallingBlock::bounds),
            )
    }
}
/// A landing's block game event, emitted after its world write is applied.
struct LandingVibration {
    event: &'static str,
    position: BlockPos,
    state: BlockStateId,
    from_entity: bool,
}

/// FallingBlockEntity carrying a block state.
#[derive(Clone, Debug)]
struct FallingBlock {
    attachment_lifetime: sunrise_mod_runtime::attachments::AttachmentLifetime,
    /// `Entity.yRot` and `xRot` (`/tp`, `/data get ... Rotation`).
    pub rotation: [f32; 2],
    /// When it joined its list (`sunrise_client::join_seq`); set by the list.
    pub(super) join: sunrise_client::join_seq::JoinSeq,
    /// Its entity id: from the entity counter on the server, the server's on a client.
    id: u32,
    state: BlockStateId,
    /// `FallingBlockEntity.fall` and `recreateFromPacket` set `blocksBuilding`;
    /// constructing an entity from NBT leaves the default false.
    blocks_building: bool,
    /// `DATA_START_POS`: spawn position used to select weighted block models.
    start_pos: BlockPos,
    position: Vec3,
    previous: Vec3,
    velocity: Vec3,
    /// `FallingBlockEntity.time`: signed, a save can start it below zero.
    age: i32,
    /// `Entity.isNoGravity`.
    no_gravity: bool,
    /// `FallingBlockEntity.tick` handles portals after moving, before landing.
    portal: sunrise_client::nether_portal::PortalState,
    /// `FallingBlockEntity.dropItem`.
    drop_item: bool,
    /// `Entity.fallDistance` (powder snow holds a block that fell far at 0.9).
    fall_distance: f64,
    /// `Entity.onGround`: whether its last move was stopped going down.
    ground: sunrise_world::ground_support::GroundState,
    /// `Entity.remainingFireTicks` (`Fire`): lava sets it, and it never counts
    /// down, as `FallingBlockEntity.tick` runs no `baseTick`.
    fire_ticks: i32,
    /// `Entity.syncVelocity`, set by `markHurt` (lava's hurt): a server sends
    /// its motion to the clients tracking it after its next update.
    sync_velocity: bool,
    /// `Entity.stuckSpeedMultiplier`: a cobweb it is in scales its next move.
    stuck: Vec3,
    damage: sunrise_client::falling_block::Damage,
    /// The UUID and compound it was loaded with, kept while it moves.
    saved: Option<Box<([i32; 4], sunrise_core::nbt::Tag)>>,
    /// What `entity_size` answered at its last size refresh (`EntityEvent.Size`): its box takes
    /// the answer while it holds for its vanilla size.
    pub(super) size_answer: Option<sunrise_client::pose::SizeAnswer>,
}
sunrise_mod_runtime::impl_attachment_lifetime!(FallingBlock);

impl FallingBlock {
    /// `EntityType.FALLING_BLOCK.getDimensions()`: width, height and eye height.
    const DIMENSIONS: [f32; 3] = [0.98, 0.98, 0.833_000_06];
    /// Its box (`falling_block::bounds`), or its `entity_size` answer's.
    fn bounds(&self) -> sunrise_core::geometry::Aabb {
        let vanilla = sunrise_client::falling_block::bounds(self.position);
        sunrise_client::pose::SizeAnswer::sized_box(
            self.size_answer,
            Self::DIMENSIONS,
            self.position,
            vanilla,
        )
    }
    /// Where `FallingBlockRenderer` draws the block's model corner, `alpha`
    /// into the tick: centred on the entity, then moved by `offset`, the
    /// block's random model offset (`ModelBlockRenderer.tesselateBlock`:
    /// `BlockState.getOffset` of `BlockPos.containing(x, maxY, z)`), so a
    /// pointed dripstone keeps the place it hung at while it falls.
    fn model_origin(
        &self,
        alpha: f64,
        offset: impl Fn(BlockStateId, i32, i32) -> Option<[f64; 3]>,
    ) -> glam::DVec3 {
        let [dx, dy, dz] = offset(
            self.state,
            self.position.x.floor() as i32,
            self.position.z.floor() as i32,
        )
        .unwrap_or([0.0; 3]);
        glam::DVec3::new(
            self.previous.x + (self.position.x - self.previous.x) * alpha - 0.5 + dx,
            self.previous.y + (self.position.y - self.previous.y) * alpha + dy,
            self.previous.z + (self.position.z - self.previous.z) * alpha - 0.5 + dz,
        )
    }
}

/// `FallingBlockRenderer.shouldRender` drops a falling block whose own state is
/// the state of the cell it is in (`level.getBlockState(entity.blockPosition())`):
/// a `NoGravity` block, or one that has just landed, sits inside an identical
/// block, and vanilla draws no model for it.
fn falling_block_visible(state: BlockStateId, cell: BlockStateId) -> bool {
    state != cell
}

/// The data loaded once at startup and never changed: blocks, items,
/// recipes, biomes, mobs, collision and motion. Read-only and `Send + Sync`,
/// so the client and an integrated server thread can share one copy.
pub struct Resources {
    core: WorldCore,
    rest: std::sync::OnceLock<WorldRest>,
}

struct WorldCore {
    item_data: Arc<sunrise_world::item_defaults::ItemData>,
    facts: BlockFacts,
    biomes: Arc<Biomes>,
    environment: Environment,
    collision: Arc<BlockCollisionRegistry<'static>>,
    motion: MotionResources,
    suffocation: sunrise_world::suffocation::SuffocationRegistry<'static>,
    spawn_registries: sunrise_client::mobs::finalize::SpawnRegistries,
    threads: usize,
}

struct WorldRest {
    items: Arc<Items>,
    recipes: Recipes,
    transformers: sunrise_client::block_transform::Transformers,
    mob_types: HashMap<String, Arc<MobType>>,
    /// The registry keys of the player, the armor stand and the dragon, resolved with these
    /// resources as each mob type's own key is ([`MobType::type_key`]).
    living_type_keys: sunrise_client::health::LivingTypeKeys,
    mob_data: sunrise_client::mobs::MobData,
    /// Complete pre-creation facts for the registered entity types.
    spawner_types: HashMap<String, sunrise_client::mobs::spawner::SpawnerTypeFacts>,
    providers: Providers,
    block_entity_render: BlockEntityRenderer,
    animate: sunrise_client::animate_tick::AnimateTables,
    fog: sunrise_client::fog::FogTables,
    music: sunrise_client::music::MusicTables,
    ambient: sunrise_client::ambience::AmbientTables,
    nether_environment: Environment,
    end_environment: Environment,
    behaviours: Arc<Behaviours>,
    /// The player model's right and left arms.
    player_arm: [Option<sunrise_client::mobs::Part>; 2],
    /// The slim model's right and left arms (3 pixels wide).
    player_arm_slim: [Option<sunrise_client::mobs::Part>; 2],
    player_type: Option<Arc<sunrise_client::mobs::MobType>>,
    /// `SpinAttackEffectModel` (Riptide's spinning boxes).
    spin_attack: Option<sunrise_client::mobs::Part>,
    stations: sunrise_client::stations::StationData,
}

/// The content tables a world's data packs change, loaded when the world
/// opens and on `/reload`: `ReloadableServerResources` loads a server's
/// recipes, loot tables and tags from the world's pack stack, over the
/// resource root's own. Worlds without packs use `Resources.rest` directly.
pub(in crate::game) struct WorldTables {
    conditions: sunrise_world::conditions::LoadConditions,
    pub(in crate::game) suffocation: sunrise_world::suffocation::SuffocationRegistry<'static>,
    /// The pack files the tables were built from (a native mod's among
    /// them), for the advancement tree reload and the loot data.
    files: Vec<(String, Vec<Vec<u8>>)>,
    priority_layers: Vec<sunrise_world::loot::RecipePriorityLayer>,
    pub(in crate::game) items: Arc<Items>,
    /// Registry definitions stay frozen at world open, even when tags reload.
    packed_registries: Arc<sunrise_client::lan::data::PackRegistryDefinitions>,
    pub(in crate::game) providers: Providers,
    loot: Arc<sunrise_world::loot::LootData>,
    loot_modifier_manager: Arc<mod_api::GlobalLootModifierManager>,
    pub(in crate::game) recipes: Recipes,
    pub(in crate::game) stations: sunrise_client::stations::StationData,
    reload_sources: Option<Arc<sunrise_client::crafting::ResolvedSources>>,
    /// The values of the data map types native mods registered (`native_data_map_types`).
    data_maps: native_data_maps::DataMaps,
    /// Pack tags used by advancement predicates and built-in data maps
    /// (`TagLoader.loadTagsForExistingRegistries`). Damage tags live in the catalog.
    pack_tags: HashMap<&'static str, HashMap<String, HashSet<String>>>,
}

struct PreparedParts {
    block_states: Arc<api_values::block_state::Index>,
    tags: HashMap<String, HashSet<mod_api::BlockKey>>,
    parts: Option<mod_api::Parts>,
    cauldrons: Arc<cauldron_interactions::Table>,
}
#[allow(unused_imports)]
use condition_reload_state::PreparedWorldReload;

impl WorldTables {
    /// The registries [`WorldTables::pack_tags`] holds.
    const PACK_TAG_REGISTRIES: [&'static str; 2] = ["worldgen/biome", "worldgen/structure"];
}

/// The equipment slot of `armor[index]`: head, chest, legs, feet.
/// `doHurtEquipment`'s `durabilityDamage` before the cast to int: `max(1, damage / 4)`.
pub(super) fn durability_damage(damage: f32) -> f32 {
    (damage / 4.0).max(1.0)
}
/// The durability damage a mod left for `slot` (`mod_api::ArmorHurt::new_damage`, cast to int
/// as vanilla casts it); an entry it removed keeps vanilla's `fallback`.
pub(super) fn answered_wear(
    entries: &[mod_api::ArmorHurt],
    slot: sunrise_core::equipment_slot::EquipmentSlot,
    fallback: u32,
) -> u32 {
    entries
        .iter()
        .find(|e| e.slot == slot)
        .map_or(fallback, |e| e.new_damage as u32)
}

pub(super) fn armor_slot(index: usize) -> sunrise_core::equipment_slot::EquipmentSlot {
    use sunrise_core::equipment_slot::EquipmentSlot;
    match index {
        0 => EquipmentSlot::Head,
        1 => EquipmentSlot::Chest,
        2 => EquipmentSlot::Legs,
        _ => EquipmentSlot::Feet,
    }
}

pub use resource_tables::UiResources;

/// What a resource pack reload replaces (`Minecraft.reloadResourcePacks`):
/// block models and sprites with their meshing tables, item models,
/// translations and the textures read on the CPU. Each game holds its own;
/// cloning shares the loaded data.
#[derive(Clone)]
pub struct PackResources {
    source: sunrise_assets::resource_source::ResourceSource,
    data_source: sunrise_assets::resource_source::ResourceSource,
    data_changes: sunrise_assets::resource_source::ResourceSource,
    additional_data: Arc<[Arc<sunrise_client::datapack::DataPack>]>,
    mod_layers: Arc<[sunrise_assets::resource_source::ResourceSource]>,
    assets: Arc<Assets>,
    /// `Blocks.WATER.defaultBlockState()` for screen-effect extraction.
    water_overlay_state: mod_api::BlockState,
    block_registries: Arc<sunrise_client::command::args::Registries>,
    render: Arc<RenderAssets>,
    context: Arc<WorkerContext>,
    layer_names: Arc<HashMap<u32, String>>,
    /// Item definitions, baked item models and special renderers.
    item_renderer: Arc<sunrise_client::item_render::ItemRenderer>,
    names: Arc<Names>,
    /// clouds.png: (width, height, filled cells).
    clouds: Arc<(u32, u32, Vec<bool>)>,
    /// misc/forcefield.png: (width, height, RGBA) for the world border wall.
    forcefield: Arc<(u32, u32, Vec<u8>)>,
    /// Custom sky layers (OptiFine's `optifine/sky`) with their textures; none without them.
    custom_sky: Arc<dyn sunrise_client::custom_sky::SkyLayers>,
    /// The overworld's, Nether's and End's attribute stacks with the packs' Polytone biome and
    /// dimension modifiers (#11486), when a pack has some: what the frame draws.
    environments: Option<Arc<[Environment; 3]>>,
    /// The packs' Polytone custom particles and block emitters (#11486), when a pack has some.
    polytone_particles: Option<Arc<sunrise_client::particles::polytone::Registry>>,
    /// The packs' Polytone post chains (#11486), when a pack has some.
    polytone_post: Option<Arc<sunrise_client::polytone_post::PolytonePost>>,
    /// The packs' Polytone custom colours (`polytone/colors.json`).
    polytone_colors: Arc<sunrise_client::pack_palette::PackColors>,
}

/// A quick craft drag as the screen draws it: each dragged slot's count and
/// whether the slot caps it (None: hidden), and what stays in hand.
struct DragView {
    slots: HashMap<usize, Option<(u32, bool)>>,
    left: Option<u32>,
}
fn equipment_modifier_applies(group: &str, slot: &str) -> bool {
    group == slot
        || group == "any"
        || (group == "armor" && slot != "mainhand" && slot != "offhand")
        || (group == "hand" && (slot == "mainhand" || slot == "offhand"))
}
/// Ctrl+Q reads the physical keyboard, independently of sprint key mappings.
/// A chunk whose ticking future can complete (`ChunkMap.prepareTickingChunk`):
/// lit, with its neighbours loaded, as `postprocess_chunks` waits for.
fn ticking_chunk(world: &World, x: i32, z: i32) -> bool {
    world.chunks.is_ticking_ready((x, z))
}
/// `TicketStorage.canTicketExpire` for a `PORTAL` ticket: its chunk's holder is
/// ready for saving once the futures of its level are done, the chunk lit and
/// its neighbours loaded out to one chunk at a ticking level
/// (`prepareTickingChunk`) and two at an entity-ticking one
/// (`prepareEntityTickingChunk`). The ticket's own level stands for the
/// holder's: the area it waits for is the one its own streaming loads.
fn portal_ticket_can_expire(
    world: &World,
    ticket: &sunrise_client::ticket_levels::PortalTicket,
) -> bool {
    use sunrise_client::ticket_levels::{BLOCK_TICKING, ENTITY_TICKING};
    let r = match ticket.level {
        l if l <= i32::from(ENTITY_TICKING) => 2,
        l if l == i32::from(BLOCK_TICKING) => 1,
        _ => 0,
    };
    let (x, z) = ticket.chunk;
    world.chunks.get(&(x, z)).is_some_and(|c| c.lit)
        && (-r..=r).all(|dz| (-r..=r).all(|dx| world.chunks.contains_key(&(x + dx, z + dz))))
}
/// Key events carry the scancode's US-layout code, which ignores XKB options like
/// `caps:swapescape`. For the keys whose meaning the keymap may remap, take the code
/// from the keysym the keymap resolved instead. This includes function keys on
/// remapped media rows; ordinary character bindings still use their physical key.
fn follow_keymap(event: &mut WindowEvent) {
    if let WindowEvent::KeyboardInput { event, .. } = event {
        follow_named_keymap(&event.logical_key, &mut event.physical_key);
    }
}

fn select_creative_tab(
    creative: &mut Creative,
    clicks: &mut sunrise_client::container_screen::Clicks,
    tab: usize,
) {
    creative.select(tab);
    *clicks = Default::default();
    // Tabs are outside the panel; their release must not drop a carried stack.
    clicks.skip_release();
}

/// The icon the Select World list shows for the world in play
/// (`GameRenderer.tryTakeScreenshotIfNeeded`): its path in the save, and when
/// the last frame grab was asked for.
struct WorldIcon {
    path: PathBuf,
    last_attempt: Option<Instant>,
}

#[derive(Debug, PartialEq)]
struct QueuedLevelEvent {
    kind: i32,
    pos: [i32; 3],
    data: i32,
    source: Option<sunrise_world::level::LevelEventPlayer>,
}

/// Physical and scaled GUI dimensions last reported by `window_resize`.
#[derive(Clone, Copy, PartialEq)]
struct ResizeState {
    size: winit::dpi::PhysicalSize<u32>,
    gui_size: [u32; 2],
}

impl ResizeState {
    /// Records `self` as the last report and says whether it differs from the previous one.
    fn differs_from(self, last: &mut Option<Self>) -> bool {
        last.replace(self) != Some(self)
    }
}

/// Temporary block-contact collections rebuilt on every server tick. Keeping
/// their backing storage on `Game` avoids reallocating them while the world is
/// ticking; the wrapper around `tick_blocks` restores the scratch even when a
/// phase returns an error.
#[derive(Default)]
struct TickScratch {
    ticking: Vec<(i32, i32)>,
    inside: Vec<BlockPos>,
    standing: Vec<(BlockPos, bool, f64)>,
    boat_cells: Vec<BlockPos>,
    touched: Vec<BlockPos>,
    sensing: Vec<BlockPos>,
    rest: Vec<BlockPos>,
    later: Vec<BlockPos>,
}


#[allow(dead_code)]
pub(in crate::game) trait Side: Sized + crate::game::side_ops::blocks::SideOps + crate::game::side_ops::events::SideOps + crate::game::side_ops::items::SideOps + crate::game::side_ops::lan::SideOps + crate::game::side_ops::mobs_creatures::SideOps + crate::game::side_ops::mobs_rest::SideOps + crate::game::side_ops::modapi::SideOps + crate::game::side_ops::modapi_hooks::SideOps + crate::game::side_ops::modapi_modding::SideOps + crate::game::side_ops::player::SideOps + crate::game::side_ops::presentation::SideOps + crate::game::side_ops::tables::SideOps + crate::game::side_ops::ui::SideOps + crate::game::side_ops::world::SideOps {
    const IS_SERVER: bool;
    fn is_server() -> bool { Self::IS_SERVER }
    fn host_side<'a, 'g>(game: &'g mut Game<'a, Self>) -> mod_api::HostSide<'g>;
    fn as_host<'a, 'g>(game: &'g Game<'a, Self>) -> &'g dyn mod_api::Host;
    fn as_block_states_backend<'a, 'g>(
        game: &'g Game<'a, Self>,
    ) -> &'g dyn mod_api::prelude::BlockStatesBackend;
    fn as_config_context<'a, 'g>(game: &'g Game<'a, Self>) -> &'g dyn mod_api::ConfigContext;
    fn as_reload_context<'a, 'g>(game: &'g Game<'a, Self>) -> &'g dyn mod_api::ReloadContext;
    fn as_pack_discovery_context_mut<'a, 'g>(
        game: &'g mut Game<'a, Self>,
    ) -> &'g mut dyn mod_api::PackDiscoveryContext;
    fn as_server_host_mut<'a, 'g>(_game: &'g mut Game<'a, Self>) -> Option<&'g mut dyn mod_api::ServerHost> { None }
    fn as_client_host_mut<'a, 'g>(_game: &'g mut Game<'a, Self>) -> Option<&'g mut dyn mod_api::ClientHost> { None }
    fn as_client_text_host<'a, 'g>(_game: &'g Game<'a, Self>) -> Option<&'g dyn mod_api::prelude::ClientTextClientHost> { None }
    fn attachment_store<'a, 'g>(game: &'g mut Game<'a, Self>) -> &'g mut dyn mod_api::AttachmentStore;
    fn world_tables(&self) -> Option<&WorldTables> { None }
    fn lan_acting_uuid(&self) -> Option<u128> { None }
    fn levels_generation(&self, _id: &str) -> u32 { 0 }
    fn session_lock_active(&self) -> bool { false }
    fn level_ref(&self, _dim: Dim, _epoch: Option<mod_api::WorldEpoch>) -> Option<mod_api::LevelRef> { None }
    fn game_side_tag_items<'a>(&self, _remote: &'a Option<remote::Remote>, _res: &'a Resources) -> Option<&'a Items> { None }
    fn levels(&self) -> Option<&levels::Levels> { None }
    fn others(&self) -> Option<&sunrise_client::join_seq::ResidentList<commands::OtherEntity>> { None }
    fn held_out(&self) -> &[u32] { &[] }
    fn fns(&self) -> Option<&functions::FnState> { None }
    fn cmds(&self) -> Option<&commands::extra::CommandExtras> { None }
    fn lan_acting(&self) -> Option<u32> { None }
    fn lan_acting_entity(&self) -> Option<u32> { None }
    fn lan_acting_fake(&self) -> bool { false }
    fn lan_guest_swaps(&self) -> &[u32] { &[] }
    fn acting_abilities(&self) -> Option<sunrise_client::lan::abilities::PlayerAbilities> { None }
    fn native_command_filter(&self) -> bool { false }
    fn riding_order(&self) -> i64 { 0 }
    fn vehicle_mobs(&self) -> Option<&crate::game::server_side::boat_passengers::VehicleMobs> { None }
    fn lan_acting_name(&self) -> Option<&str> { None }
    fn server_tick_deadline(&self) -> Option<Instant> { None }
    fn locate_resident_owner<'a>(
        _game: &Game<'a, Self>,
        _at: sunrise_client::join_seq::residency::table::Location,
    ) -> Option<sunrise_client::join_seq::residency::table::SourceEntity<(commands::Target, bool)>> {
        None
    }
}

struct Game<'a, S> {
    /// Shared with an integrated server; `collision` and `motion` borrow from it.
    res: Arc<Resources>,
    rest_loader: Option<WorldRestLoader>,
    /// Single player's server while it loads its world (`Minecraft.doWorldLoad`).
    integrated_starting: Option<integrated::Starting>,
    collision: &'a BlockCollisionRegistry<'a>,
    motion: &'a MotionResources,
    /// This world's frozen damage definitions and reloadable tag snapshot.
    pub(super) health_resources: sunrise_client::health::HealthResources,
    damage_tag_generation: u64,
    pack: PackResources,
    /// The title and menu bundle (GUI atlas, font and panorama).
    gui: UiResources,
    world: World,
    /// The biome table the world plays with: the game's own with the world's
    /// packs over it (`Biomes::with_packs`), `None` before a world opens.
    world_biomes: Option<Arc<Biomes>>,
    /// `world_biomes` as the biome files made it, before `native_biome_colors`.
    world_biomes_base: Option<Arc<Biomes>>,
    streaming: Streaming,
    options: Options,
    player: Option<PlayerMotion>,
    spawn: Option<Vec3>,
    clock: FixedStep,
    previous_position: Vec3,
    eye: f32,
    yaw: f32,
    pitch: f32,
    inventory: Vec<Option<ItemStack>>,
    selected: usize,
    carried: Option<ItemStack>,
    craft_grid: Vec<Option<ItemStack>>,
    craft_size: usize,
    /// Open container block entity, if any.
    container: Option<(i32, i32, i32)>,
    /// Open workstation menu (enchanting table, anvil, ...).
    station: Option<stations_common::Station>,
    /// Status effects and thrown potions (game/items/potions.rs).
    potions: potions::PotionState,
    scratch_slot: Option<ItemStack>,
    mining: Option<Mining>,
    use_delay: u32,
    creative: bool,
    /// The game mode; `creative` mirrors `mode == Creative`.
    mode: sunrise_client::command::args::GameMode,
    /// Minecraft.pause: a single-player screen that pauses the game is open.
    pause: bool,
    previous: Instant,
    ticks: u64,
    /// What ends the game with an error; kept as it is, so main can tell a debug crash.
    error: Option<Box<dyn std::error::Error>>,
    level: Level,
    /// The side, registries and mod settings of this game's resource generation, which
    /// its menus lend to the hooks they fire inside a loop; rebuilt by `refresh_loop_context`.
    loop_context: Arc<sunrise_world::native_load_context::LoopContext>,
    /// This thread's authority over the game's tick-owned state (`parallel::TickThread`): a
    /// `Game` never leaves the thread that built it.
    tick: sunrise_client::parallel::TickThread,
    setting_overrides: std::cell::RefCell<std::collections::BTreeMap<(String, String), String>>,
    /// The native damage hooks every level of this game lends to its hits.
    damage_hooks: sunrise_client::health::DamageHooks,
    dropped: sunrise_client::join_seq::JoinList<ItemEntity>,
    mobs: sunrise_client::join_seq::JoinList<Mob>,
    /// `mobs` by network id, for the packets that name one (`game/mobs/mob_index.rs`).
    mob_index: mob_index::MobIndex,
    creatures: creatures::Creatures,
    /// Client-ticked block-entity animation state (`game/blocks/block_entity_anims.rs`).
    be_anims: sunrise_client::block_entity_anim::BlockEntityAnims,
    /// Item being used (eaten): hotbar slot and ticks used.
    using: Option<(usize, u32)>,
    /// `getUseDuration` at the use's start, as a mod left it (`game/items/item_use.rs`).
    use_duration: i32,
    /// What that use began with (`useItem`) and whether a mod set its ticks left
    /// (`game/items/item_use.rs`).
    use_start: using_item::UseStart,
    /// Armor slots: head, chest, legs, feet.
    armor: [Option<ItemStack>; 4],
    /// The off hand (Inventory slot 40, saved as slot 150; menu slot 504 here).
    offhand: Option<ItemStack>,
    /// The body and saddle slots (`EquipmentSlot.BODY`, `SADDLE`; Inventory
    /// slots 41 and 42, saved as 151 and 152). The hands and armor stay in
    /// the fields above. Nothing in the game reads these yet; they are kept
    /// for mods and saves.
    equipment: sunrise_items::equipment::Equipment,
    /// The hand an item use acts with (`InteractionHand`): the off hand only
    /// for its turn in `use_item` and while its item is being used.
    hand: Hand,
    /// `Player.getCooldowns()`: the `use_cooldown` component's entries, what
    /// `itemDecorations` shades a slot by. A server's players have their own
    /// (`GuestPlay::cooldowns`), swapped in for their turn.
    cooldowns: sunrise_client::cooldowns::Cooldowns,
    /// This game's world is autosaving this tick (`MinecraftServer.isSaving`).
    autosaving: bool,
    experience: Experience,
    orbs: sunrise_client::join_seq::JoinList<ExperienceOrb>,
    /// The arm swing (`LivingEntity.SwingState`) and how many swings started.
    swing: sunrise_client::first_person::SwingState,
    swings_started: u32,
    /// `Minecraft`'s attack handling, attack clicks since the last tick and
    /// the attack strength tickers the hand reads.
    clicks: sunrise_client::first_person::Clicks<Option<ItemStack>>,
    settings: Settings,
    flash: u32,
    /// The head of the bed while sleeping; the visual timer also survives waking.
    sleeping: Option<[i32; 3]>,
    sleep_timer: u32,
    dim: Dim,
    server_items: Option<Arc<Items>>,
    /// The world is superflat (`ServerLevel.isFlat`, for every dimension).
    world_flat: bool,
    /// Nether portals: the player's, items' and mobs' (game/blocks/nether_portal.rs).
    portals: nether_portal::Portals,
    end: end::EndState,
    block_ticks: Ticks,
    level_events: Vec<LevelEvent>,
    falling: sunrise_client::join_seq::JoinList<FallingBlock, false>,
    paintings: sunrise_client::join_seq::JoinList<sunrise_client::entities::Painting>,
    painting_variants: paintings::PaintingVariants,
    item_frames: sunrise_client::join_seq::JoinList<sunrise_client::entities::ItemFrame>,
    armor_stands: sunrise_client::join_seq::JoinList<sunrise_client::entities::ArmorStand>,
    /// `Cushion` decoration entities (game/entities/cushions.rs).
    cushions: sunrise_client::join_seq::JoinList<sunrise_client::entities::Cushion>,
    boats: sunrise_client::join_seq::JoinList<boats::Boat, false>,
    carts: sunrise_client::join_seq::JoinList<minecarts::Minecart, false>,
    bolts: sunrise_client::join_seq::ResidentList<lightning::Bolt>,
    maps: maps::Maps,
    dragon: native_entity_refs::dragon_parts_table::DragonSlot,
    crystals: sunrise_client::join_seq::JoinList<dragon::Crystal>,
    breath_clouds: sunrise_client::join_seq::ResidentList<dragon::Breath>,
    /// `ProfileResolver.Cached`: `Dynamic` profiles (name- or id-only heads
    /// and mannequins) resolve through the services in the background.
    skin_profiles: RefCell<sunrise_client::skins::Resolver>,
    /// The boat the player rides.
    riding: Option<u32>,
    /// The mount the last `position_rider` put the player on, to tell the tick
    /// a seat is taken (`Entity.moveTo`, no camera sweep) from the ones that
    /// follow it (`positionRider`, interpolated as usual).
    posed_rider: Option<u32>,
    /// Block entity data of broken blocks waiting for their dropped item.
    pending: spill::Pending,
    /// The player's fishing hook.
    hook: sunrise_client::join_seq::ResidentSlot<fishing::Hook>,
    tridents: sunrise_client::join_seq::JoinList<trident::ThrownTrident>,
    rockets: sunrise_client::join_seq::JoinList<fireworks::Rocket>,
    /// Ominous trial spawners' item spawners.
    ominous_spawners:
        sunrise_client::join_seq::ResidentList<ominous_item_spawner::OminousItemSpawner>,
    tnts: sunrise_client::join_seq::JoinList<tnt::PrimedTnt>,
    /// The open `host.transaction`s, innermost last (`transactions.rs`).
    transactions: transactions::OpenTransactions,
    /// Block-entity positions retained between ticks for client effects.
    spawner_tick_positions: Vec<sunrise_client::block_entities::Pos>,
    trial_spawner_tick_positions: Vec<sunrise_client::block_entities::Pos>,
    /// The `[[native.loop]]` slots' state, per published loop (`published_loops.rs`).
    loop_states: published_loops::LoopStates,
    item_features: sunrise_items::creative_catalog::ItemFeatureRequirements,
    creative_contents: native_hooks::CreativeCache,
    entity_initialization: entity_initialization::EntityInitialization,
    kinetic_feedback: HashMap<i32, i64>,
    control: Option<sunrise_client::control::Server>,
    /// This play session's frame and tick times, for opt-in performance statistics.
    perf: sunrise_client::telemetry::Session,
    /// Sunrise mods (`game/modding/mods/mod.rs`), none running unless turned on.
    mods: mods::ModsState,
    /// Every item and recipe, for mods (`game/modding/native_recipes.rs`), made once.
    recipe_tables: native_recipes::RecipeTables,
    /// The native lifecycle bookkeeping.
    native_lifecycle: native_lifecycle::NativeLifecycle,
    resident_entities: resident_state::ResidentEntities,
    /// Native mods' config files this game loaded (game/modding/native_configs.rs).
    native_configs: native_configs::NativeConfigs,
    /// Private custom-book connection and placement state.
    native_net: native_net::NativeNet,
    /// The running agent action from the control socket (game/runtime/agent/mod.rs).
    agent: agent::Agent,
    /// What happened, for `events` and `wait-for` (game/runtime/events.rs).
    events: events::Events,
    /// The health history and client captures (game/profile_capture.rs).
    profile: profile_capture::Profile,
    /// The overworld's last `sky_darken` and its inputs (time, rain, thunder).
    sky_darken_memo: std::cell::Cell<Option<((Dim, i64, u32, u32), i32)>>,
    persist: persist::EntityMemory,
    entity_attachments: native_colliders::EntityMaps,
    collider_placements: native_colliders::Placements,
    collider_histories: native_colliders::Histories,
    /// Typed per-level membership follows dimensions, not the temporary world swap.
    attachment_indexes: attachment_index::Indexes,
    /// Connected to a server: the world comes from the network (game/network/remote/mod.rs).
    remote: Option<remote::Remote>,
    /// The current connection came from Realms (retained for reconnect).
    realm_session: bool,
    /// The current connection came from the LAN list (`ServerData.isLan`).
    lan_session: bool,
    /// Replay playback (game/replay.rs), and Sunrise Studio's editor on it (game/studio.rs).
    pub(super) replay: replay::Replay,
    /// This world is open to the network (game/lan.rs).
    lan: Option<lan::LanHost>,
    /// Single player's server thread, which `remote` is connected to.
    integrated: Option<integrated::Integrated>,
    /// This game is that server thread's (game/world/integrated.rs).
    serving: Option<integrated::Serving>,
    /// The open world is shared behind an invite code (game/network/friends.rs).
    #[cfg(feature = "friends")]
    friends: Option<friends::Sharing>,
    /// The passages mods registered in each level, and a client's copy (`native_passages.rs`).
    passages: native_passages::Passages,
    /// MultiPlayerGameMode.previousLocalPlayerMode (F3+N goes back to it).
    previous_mode: Option<sunrise_client::command::args::GameMode>,
    /// `Player.entityInteractionRange` and `blockInteractionRange`, refreshed
    /// each tick from the attribute (a server's reach modifier, creative's
    /// +2): the pick helpers' `&self` reach.
    entity_reach: f64,
    block_reach: f64,
    /// Live attachments and original snapshots retained during player replacement.
    player_state: original_player::PlayerState,
    ui: chat::UiState,
    mount: mounts::MountState,
    stats: stats::Stats,
    score: scoreboard::ScoreState,
    border: sunrise_client::worldborder::WorldBorder,
    adv: advancements::AdvState,
    fangs: fangs::FangState,
    pub(in crate::game) side: S,
}
/// The open world's dimensions past vanilla's three. One per process, as a window
/// and its server thread name the same dimensions.
static EXTRA_DIMS: std::sync::RwLock<Vec<mod_api::DimensionKey>> =
    std::sync::RwLock::new(Vec::new());

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Dim {
    Overworld,
    Nether,
    End,
    /// A dimension past vanilla's three, from a data pack or a mod.
    Extra(mod_api::DimensionKey),
}
impl Dim {
    /// The level as block behaviour and the mods name it.
    fn level(self) -> LevelDimension {
        match self {
            Self::Overworld => LevelDimension::Overworld,
            Self::Nether => LevelDimension::Nether,
            Self::End => LevelDimension::End,
            Self::Extra(_) => LevelDimension::of_level(
                sunrise_core::LevelKey::parse(self.id())
                    .unwrap_or(sunrise_core::LevelKey::OVERWORLD),
            ),
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::Overworld => "minecraft:overworld",
            Self::Nether => "minecraft:the_nether",
            Self::End => "minecraft:the_end",
            Self::Extra(key) => {
                mod_api::__private::dimension_id(key).expect("the game's table made the key")
            }
        }
    }
    fn from_id(id: &str) -> Self {
        match id {
            "minecraft:the_nether" => Self::Nether,
            "minecraft:the_end" => Self::End,
            _ => Self::extra(id).unwrap_or(Self::Overworld),
        }
    }
    /// `Level.getSeaLevel` of the level: the sea level its noise settings give a mod's level, else
    /// the built-in level's.
    fn sea_level(self) -> i32 {
        Self::sea_level_of_id(self.id())
    }
    /// [`Self::sea_level`] of the level `id` names.
    fn sea_level_of_id(id: &str) -> i32 {
        sunrise_core::LevelKey::parse(id).map_or(63, sunrise_world::level::sea_level_of)
    }
    /// The open world's dimension `id` past vanilla's three.
    fn extra(id: &str) -> Option<Self> {
        let extra = EXTRA_DIMS.read().unwrap();
        let key = extra
            .iter()
            .find(|key| mod_api::__private::dimension_id(**key) == Some(id))?;
        Some(Self::Extra(*key))
    }
    fn is_vanilla(id: &str) -> bool {
        matches!(
            id,
            "minecraft:overworld" | "minecraft:the_nether" | "minecraft:the_end"
        )
    }
    /// Whether this is one of vanilla's built-in levels (Overworld, Nether, End).
    fn is_builtin(self) -> bool {
        matches!(self, Self::Overworld | Self::Nether | Self::End)
    }
    /// Whether the open world has the dimension `id`.
    fn exists(id: &str) -> bool {
        Self::is_vanilla(id) || Self::extra(id).is_some()
    }
    /// The open world's dimensions, in the order its server ticks them.
    fn all() -> Vec<Self> {
        let extra = EXTRA_DIMS.read().unwrap();
        [Self::Overworld, Self::Nether, Self::End]
            .into_iter()
            .chain(extra.iter().copied().map(Self::Extra))
            .collect()
    }
    /// The dimensions past vanilla's three of the world being opened.
    fn set_extra<'a>(ids: impl Iterator<Item = &'a str>) {
        *EXTRA_DIMS.write().unwrap() = ids.map(mod_api::__private::dimension_key).collect();
    }
    /// Adds the dimension `id` a remote server sent, which no local world told of, so that
    /// [`Self::from_id`] finds it.
    fn add_extra(id: &str) {
        if !Self::exists(id) {
            EXTRA_DIMS
                .write()
                .unwrap()
                .push(mod_api::__private::dimension_key(id));
        }
    }
    /// The dimension as mods' level handles name it.
    fn key(self) -> mod_api::DimensionKey {
        match self {
            Self::Overworld => mod_api::DimensionKey::OVERWORLD,
            Self::Nether => mod_api::DimensionKey::NETHER,
            Self::End => mod_api::DimensionKey::END,
            Self::Extra(key) => key,
        }
    }
    /// The level's shape in a world of vanilla's dimension types.
    fn shape(self) -> sunrise_client::save::LevelShape {
        use sunrise_client::save::LevelShape;
        match self {
            Self::Overworld => LevelShape::OVERWORLD,
            Self::Nether => LevelShape::NETHER,
            Self::End => LevelShape::END,
            Self::Extra(_) => LevelShape::OVERWORLD,
        }
    }
}
/// `Inventory.add` on a player's 36 slots (hotbar first) and off hand,
/// `max` the stack's own maximum, one slot at a time (`addResource`): onto an
/// equal stack while there is one (`getSlotWithRemainingSpace`), else into an
/// empty slot of the inventory (`getFreeSlot`, never the off hand), until the
/// stack is used up, no slot is left or a slot takes none of it. What does
/// not fit stays in `stack`; true when some of it is left.
fn inventory_add(
    inventory: &mut [Option<ItemStack>],
    offhand: &mut Option<ItemStack>,
    selected: usize,
    max: u32,
    stack: &mut ItemStack,
) -> bool {
    use sunrise_client::menu::{OFFHAND, free_slot, slot_with_remaining_space};
    loop {
        // `Inventory.add`: an empty stack adds nothing (`isEmpty`).
        if stack.count == 0 {
            return false;
        }
        let slot =
            slot_with_remaining_space(&inventory[..36], offhand.as_ref(), selected, stack, max)
                .or_else(|| free_slot(&inventory[..36]));
        #[cfg(test)]
        let slot =
            rule_replacements::add_slot(&inventory[..36], offhand.as_ref(), selected, stack, max)
                .unwrap_or(slot);
        let Some(slot) = slot else {
            return true;
        };
        let cell = if slot == OFFHAND {
            &mut *offhand
        } else {
            &mut inventory[slot]
        };
        let moved = cell
            .as_ref()
            .map_or(max, |cell| max.saturating_sub(cell.count))
            .min(stack.count);
        // `Inventory.add`'s loop ends once an `addResource` moves nothing
        // (`getCount() < lastSize`): a slot with no room for the stack.
        if moved == 0 {
            return true;
        }
        match cell {
            Some(cell) => cell.count += moved,
            None => {
                *cell = Some(ItemStack {
                    count: moved,
                    ..stack.clone()
                })
            }
        }
        stack.count -= moved;
    }
}

fn facing_offset(facing: &str) -> [i32; 3] {
    match facing {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        "up" => [0, 1, 0],
        _ => [0, -1, 0],
    }
}
/// How far along `dir` (within `reach`) a ray from `eye` enters `b`.
fn ray_box(eye: [f64; 3], dir: [f64; 3], reach: f64, b: Aabb) -> Option<f64> {
    let (min, max) = (b.min_position(), b.max_position());
    let (lo, hi) = ([min.x, min.y, min.z], [max.x, max.y, max.z]);
    let (mut t0, mut t1) = (0.0f64, reach);
    for a in 0..3 {
        if dir[a].abs() < 1e-9 {
            if eye[a] < lo[a] || eye[a] > hi[a] {
                return None;
            }
            continue;
        }
        let (mut ta, mut tb) = ((lo[a] - eye[a]) / dir[a], (hi[a] - eye[a]) / dir[a]);
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        t0 = t0.max(ta);
        t1 = t1.min(tb);
    }
    (t0 <= t1).then_some(t0)
}
/// `Entity.calculateViewVector`, behind `getViewVector`, `getLookAngle` and `GameRenderer.pick`: float radians
/// through `Mth.sin` and `Mth.cos`, vanilla's 65536-entry table. Aims, a deflected projectile's motion, a spear's
/// reach and a placement's cursor all come from it.
fn view_vector(yaw: f32, pitch: f32) -> [f64; 3] {
    let v = sunrise_world::view_direction::view_vector(pitch, yaw);
    [v.x, v.y, v.z]
}
#[cfg(test)]
mod view_vector_tests {
    /// `Entity.calculateViewVector` on HotSpot 25.0.4.1 (x86-64) for angles where double-precision `sin`/`cos`
    /// gave other bits: (pitch, yaw) as float bits, vanilla's x, y and z as double bits.
    #[test]
    fn the_view_vector_has_vanillas_bits() {
        let cases: [(u32, u32, [u64; 3]); 4] = [
            (
                0xc1d5_be2f,
                0x43b0_17dc,
                [
                    0x3fbf_198b_0000_0000,
                    0x3fdc_c507_6000_0000,
                    0x3fec_514d_4000_0000,
                ],
            ),
            (
                0x42b4_0000,
                0x43b4_0001,
                [
                    0x0000_0000_0000_0000,
                    0xbff0_0000_0000_0000,
                    0x3ca1_a626_4000_0000,
                ],
            ),
            (
                0xc2ad_4925,
                0x43a0_0c61,
                [
                    0x3fa3_3b12_a000_0000,
                    0x3fef_f1e7_2000_0000,
                    0x3fa6_fe4a_e000_0000,
                ],
            ),
            (
                0x3b32_7d58,
                0x4ea6_1ddd,
                [
                    0xbfd0_9895_4000_0000,
                    0x8000_0000_0000_0000,
                    0xbfee_e7c7_8000_0000,
                ],
            ),
        ];
        for (pitch, yaw, expected) in cases {
            let v = super::view_vector(f32::from_bits(yaw), f32::from_bits(pitch));
            assert_eq!(
                v.map(f64::to_bits),
                expected,
                "pitch {pitch:#x}, yaw {yaw:#x}"
            );
        }
    }
}
/// `Level.canHaveWeather`: sky light, no ceiling, and not the End (the one
/// level the rule names by id).
fn level_can_have_weather(sky_light: bool, has_ceiling: bool, is_end: bool) -> bool {
    sky_light && !has_ceiling && !is_end
}
/// `Camera.setRotation` (`rotationYXZ(π - yaw, -pitch, 0)`) for the player's
/// view, turned round in third-person front view (`Camera.setup`).
fn camera_rotation(
    camera_type: sunrise_client::camera::CameraType,
    yaw: f32,
    pitch: f32,
) -> glam::Quat {
    let (view_yaw, view_pitch) =
        if camera_type == sunrise_client::camera::CameraType::ThirdPersonFront {
            (yaw + 180.0, -pitch)
        } else {
            (yaw, pitch)
        };
    glam::Quat::from_euler(
        glam::EulerRot::YXZ,
        std::f32::consts::PI - view_yaw.to_radians(),
        -view_pitch.to_radians(),
        0.0,
    )
}
/// The world directions of a camera-facing quad's x and y: `PoseStack.rotate` by the camera's
/// orientation, as `FishingHookRenderer`, `DragonFireballRenderer` and
/// `SubmitNodeCollection.submitNameTag` turn theirs. Both stay whole looking straight up or
/// down, where the view vector has no horizontal part.
fn billboard_axes(orientation: glam::Quat) -> [glam::DVec3; 2] {
    [glam::Vec3::X, glam::Vec3::Y].map(|axis| (orientation * axis).as_dvec3())
}
#[cfg(test)]
mod billboard_axes_tests {
    /// `Camera.setRotation(30, -90)` (looking straight up) on HotSpot 25.0.4.1: its orientation
    /// turns x and y to these, as float bits. glam's quaternion is not JOML's to the last bit.
    #[test]
    fn a_billboard_looking_straight_up_takes_the_cameras_axes() {
        let expected: [[u32; 3]; 2] = [
            [0xbf5d_b3d8, 0x0000_0000, 0xbf00_0002],
            [0x3f00_0002, 0xb180_0001, 0xbf5d_b3d7],
        ];
        let axes = super::billboard_axes(super::camera_rotation(
            sunrise_client::camera::CameraType::FirstPerson,
            30.0,
            -90.0,
        ));
        for (axis, expected) in axes.into_iter().zip(expected) {
            let expected = glam::Vec3::from_array(expected.map(f32::from_bits)).as_dvec3();
            assert!(
                axis.abs_diff_eq(expected, 1e-6),
                "{axis} against {expected}"
            );
        }
    }
}
/// What the falling blocks of one frame share: the depth-only view (none under a pack) and the
/// camera's forward axis.
pub(in crate::game) struct FallingSetup<'a> {
    pub(in crate::game) view: Option<&'a sunrise_client::gfx::JoinedFrusta>,
    pub(in crate::game) forward: glam::Vec3,
}

/// What the experience orbs of one frame share: their sprite and the camera's axes.
pub(crate) struct OrbFrame {
    sprite: Option<sunrise_client_render::hud::Rect>,
    forward: glam::Vec3,
    right: glam::Vec3,
    up: glam::Vec3,
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// `Level.canHaveWeather`, from the level's dimension type (sky light, no
    /// ceiling) and the End, which vanilla names by id. Every rain, thunder
    /// and lightning rule asks this, never the dimension's id.
    fn level_key(&self) -> sunrise_core::LevelKey {
        sunrise_core::LevelKey::builtin_or_overworld(self.dim.id())
    }
    /// The level's dimension type: a mod's or a data pack's own when it has one, else the
    /// built-in level's.
    fn dimension_type(&self) -> &sunrise_world::dimension_type::DimensionFacts {
        // A client of a server has the facts its server sent for the level, which are its own,
        // not the table the server's levels (in this process, for an integrated server) read.
        if let Some(facts) = self.remote.as_ref().and_then(|r| r.level_facts.as_deref()) {
            return facts;
        }
        // Else the level the server named, whether or not the client has the mod that
        // registered it (`self.dim` is then the Overworld).
        let level = self
            .remote
            .as_ref()
            .map(|r| r.level.as_str())
            .filter(|level| !level.is_empty())
            .unwrap_or_else(|| self.dim.id());
        self.dimension_facts_for(
            sunrise_core::LevelKey::parse(level).unwrap_or(sunrise_core::LevelKey::OVERWORLD),
        )
    }
    /// A client only knows its current level's type; other keys use built-in defaults.
    fn dimension_facts_for(
        &self,
        key: sunrise_core::LevelKey,
    ) -> &sunrise_world::dimension_type::DimensionFacts {
        use sunrise_world::dimension_type::DimensionFacts;
        if let Some(remote) = &self.remote {
            if sunrise_core::LevelKey::parse(&remote.level).ok() == Some(key)
                && let Some(facts) = remote.level_facts.as_deref()
            {
                return facts;
            }
            DimensionFacts::of_builtin_level(key)
        } else {
            DimensionFacts::of_level(key)
        }
    }
    fn dimension_type_of(
        &self,
        dim: Dim,
    ) -> &'static sunrise_world::dimension_type::DimensionFacts {
        sunrise_world::dimension_type::DimensionFacts::of_level(
            sunrise_core::LevelKey::parse(dim.id()).unwrap_or(sunrise_core::LevelKey::OVERWORLD),
        )
    }

    fn eye_position(&self) -> Option<[f64; 3]> {
        let p = self.player.as_ref()?;
        Some([
            p.position.x,
            p.position.y + f64::from(p.dimensions.eye_height),
            p.position.z,
        ])
    }
    /// `Player.blockInteractionRange`: the block pick's and mining's reach.
    fn reach(&self) -> f64 {
        self.block_reach
    }
    /// Outline shapes that depend on more than the block state, as one box
    /// in block space: `LightBlock` and `ScaffoldingBlock.getShape` are a full
    /// block while the main hand holds the block's own item
    /// (`CollisionContext.isHoldingItem`), and `ShulkerBoxBlock.getShape`
    /// grows with the lid (`ShulkerBoxBlockEntity.getBoundingBox`).
    fn context_outline(
        &self,
        pos: [i32; 3],
        state: BlockStateId,
        player_context: bool,
        anims: &sunrise_client::block_entity_anim::BlockEntityAnims,
    ) -> Option<[f64; 6]> {
        let name = &self.res.facts().get(state).name;
        if matches!(&**name, "minecraft:light" | "minecraft:scaffolding") {
            return (player_context && self.held().is_some_and(|s| *s.item == **name))
                .then_some([0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        }
        if !name.ends_with("shulker_box") {
            return None;
        }
        let progress = anims.shulkers.get(&pos).map_or(0.0, |s| s.progress);
        let facing = self
            .pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get("facing").cloned());
        let d = facing_offset(facing.as_deref().unwrap_or("up"));
        let mut b = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        for a in 0..3 {
            match d[a] {
                1 => b[a + 3] += 0.5 * f64::from(progress),
                -1 => b[a] -= 0.5 * f64::from(progress),
                _ => {}
            }
        }
        Some(b)
    }
    fn outline_context_kind(&self) -> sunrise_world::collision_context::EntityContextKind {
        self.side_outline_camera_kind()
    }
    /// A block's outline boxes in world space (`getShape` plus the random offset).
    fn outline_boxes(&self, pos: [i32; 3]) -> Vec<[f64; 6]> {
        self.outline_boxes_in_context(&self.world, &self.be_anims, pos, true)
    }
    fn outline_boxes_in_context(
        &self,
        world: &World,
        anims: &sunrise_client::block_entity_anim::BlockEntityAnims,
        pos: [i32; 3],
        player_context: bool,
    ) -> Vec<[f64; 6]> {
        self.outline_boxes_iter(
            world,
            world.block(pos[0], pos[1], pos[2]),
            anims,
            pos,
            player_context,
        )
        .collect()
    }
    /// The declared outline uses vanilla's inside-ray test and interaction override.
    fn clip_declared_outline(
        &self,
        state: BlockStateId,
        pos: [i32; 3],
        origin: [f64; 3],
        dir: [f64; 3],
        reach: f64,
    ) -> Option<Hit> {
        use std::borrow::Cow;
        use sunrise_core::geometry::{Aabb, Vec3};
        let facts = self.res.facts().get(state);
        let cached = self.collision.cached_shapes();
        use native_state_shapes::Outline;
        let (outline, offset) =
            self.selected_outline(&self.world, state, &self.be_anims, pos, true);
        let boxes: Cow<'_, [Aabb]> = match outline {
            Outline::Published(shape) | Outline::Declared(shape, _) => Cow::Borrowed(shape.boxes()),
            Outline::Dynamic(b) => Cow::Owned(vec![Aabb::new(
                Vec3::new(b[0], b[1], b[2]),
                Vec3::new(b[3], b[4], b[5]),
            )]),
            Outline::Default(facts) => Cow::Owned(
                facts
                    .outline
                    .iter()
                    .map(|b| Aabb::new(Vec3::new(b[0], b[1], b[2]), Vec3::new(b[3], b[4], b[5])))
                    .collect(),
            ),
        };
        let offset = Vec3::new(offset[0], offset[1], offset[2]);
        let interaction_offset = cached.shape_offset(
            state,
            sunrise_world::collision::ShapeChannel::Interaction,
            pos,
        );
        let interaction_offset = Vec3::new(
            interaction_offset[0],
            interaction_offset[1],
            interaction_offset[2],
        );
        let interaction: Cow<'_, [Aabb]> = if let Some(shape) = cached.interaction_shape(state) {
            Cow::Borrowed(shape.boxes())
        } else {
            let facing = self
                .pack
                .assets
                .registry
                .state(state)
                .and_then(|s| s.properties.get("facing"))
                .map(String::as_str);
            Cow::Owned(
                interaction_shapes::boxes(&facts.name, facing)
                    .into_iter()
                    .map(|b| Aabb::new(Vec3::new(b[0], b[1], b[2]), Vec3::new(b[3], b[4], b[5])))
                    .collect(),
            )
        };
        let start = Vec3::new(origin[0], origin[1], origin[2]);
        let end = Vec3::new(
            origin[0] + dir[0] * reach,
            origin[1] + dir[1] * reach,
            origin[2] + dir[2] * reach,
        );
        let hit = sunrise_world::interaction_shapes::clip_with_interaction_override(
            boxes.iter().map(|b| b.move_by(offset)),
            interaction.iter().map(|b| b.move_by(interaction_offset)),
            start,
            end,
            BlockPos::new(pos[0], pos[1], pos[2]),
        )?;
        Some(Hit::from_block_hit(pos, &hit))
    }

    /// The first block outline hit within `reach` along the unit `dir`.
    fn clip_ray(&self, origin: [f64; 3], dir: [f64; 3], reach: f64) -> Option<Hit> {
        self.clip_ray_fluids(origin, dir, reach, false)
    }
    /// `LocalPlayer.moveTowardsClosestSpace` at the body's four corners
    /// (aiStep, unless a spectator or a passenger): a corner inside a
    /// suffocating block sets a 0.1 speed towards the nearest side that is
    /// free, on that side's axis; a later corner overwrites an earlier one.
    /// Suffocating is `LocalPlayer.suffocatesAt` (`closest_space_nudge`).
    fn closest_space_nudge(&self) -> [Option<f64>; 2] {
        match self.player.as_ref() {
            Some(p) if !p.spectator && self.riding.is_none() => {
                closest_space_nudge(p, &self.world, self.collision, self.level_suffocation())
            }
            _ => [None; 2],
        }
    }
    /// What the window decides for its player's body every tick: the look
    /// (Player.travel steers sprint-swimming by the pitch), Auto Jump (LocalPlayer.sendPosition), the slowdown of an
    /// item in use (LocalPlayer.modifyInput), the helmet's Respiration
    /// (decreaseAirSupply), the wings, leather boots on powder snow
    /// (PowderSnowBlock.canEntityWalkOnPowderSnow), freeze-immune wear
    /// (LivingEntity.canFreeze), the dimension's FAST_LAVA, the sleeping
    /// pose and blindness (Player.isMobilityRestricted, which stops a sprint).
    /// The window's or the guest's own body reads the item in its selected slot
    /// (`getMainHandItem`) for the shapes that depend on it.
    pub(crate) fn sync_main_hand(&mut self) {
        let held = self
            .inventory
            .get(self.selected)
            .and_then(Option::as_ref)
            .filter(|stack| stack.count > 0)
            .map(|stack| stack.item.clone());
        if let Some(player) = self.player.as_mut() {
            player.main_hand = held;
        }
    }
    pub(crate) fn sync_player_body(&mut self) {
        self.sync_main_hand();
        let glider = self.glider_worn();
        let sleeping = self.sleeping.is_some();
        let item_in_use = self
            .using
            .and_then(|(slot, _)| {
                if slot == shield::OFFHAND_SLOT {
                    self.offhand.as_ref()
                } else {
                    self.inventory.get(slot)?.as_ref()
                }
            })
            .and_then(|s| self.items().get(&s.item));
        let item_use_speed = item_in_use.map(|i| i.use_speed);
        let item_use_slows_sprint = item_in_use.is_some_and(|i| !i.can_sprint);
        let respiration = self.armor[0]
            .as_ref()
            .map_or(0, |s| s.enchantment_level("minecraft:respiration") as i32);
        let leather_boots = worn_boots::leather_boots(&self.armor);
        let freeze_immune = self.wears_freeze_immune();
        let nudge = self.closest_space_nudge();
        let fast_lava = self.dimension_type().fast_lava;
        let blind = self.potions.effects.get("blindness").is_some();
        let Some(player) = self.player.as_mut() else {
            return;
        };
        player.nudge = nudge;
        player.mobility_restricted = blind;
        player.yaw = self.yaw;
        player.pitch = self.pitch;
        player.auto_jump_enabled = self.settings.auto_jump;
        player.item_use_speed = item_use_speed;
        player.item_use_slows_sprint = item_use_slows_sprint;
        player.respiration = respiration;
        player.glider = glider;
        // On someone else's server the glide flag is the server's
        // (`DATA_SHARED_FLAGS_ID`, applied by the SetEntityData handler); the
        // game's own integrated server has no such round trip, so the client
        // keeps predicting the clear a tick after the landing glide.
        player.server_owns_glide = self.integrated.is_none();
        let _ = player.set_sleeping(sleeping);
        player.leather_boots = leather_boots;
        player.freeze_immune = freeze_immune;
        player.fluids.fast_lava = fast_lava;
    }
    /// Where a loot roll at `origin` happens, for a mod's own loot kinds:
    /// the block entity it may copy from, and this level as
    /// `LootContext.getLevel` gives it (`Level.isRaining`: rain above 0.2;
    /// `isThundering`: thunder above 0.9; neither where `canHaveWeather` is
    /// false: a level with a ceiling or no sky light, or the End).
    fn loot_place_at(
        &self,
        origin: [f64; 3],
        block_entity: Option<sunrise_core::nbt::Tag>,
    ) -> sunrise_world::loot::LootPlace {
        let (block_entity, block_components) = match block_entity {
            Some(saved) => {
                let source = spill::source_snapshot(saved, None, None);
                (Some(source.nbt), Some(source.components))
            }
            None => (None, None),
        };
        let (rain, thunder) = self.world.weather.levels(1.0);
        // Level.canHaveWeather: sky light, no ceiling and not the End.
        let weather =
            self.world.shape.sky_light && !self.world.shape.has_ceiling && self.dim != Dim::End;
        sunrise_world::loot::LootPlace {
            origin: Some(origin),
            block_entity,
            block_components,
            storage: Some(self.score.storage.snapshot()),
            level: Some(sunrise_world::loot::LootLevel {
                dimension: self.dim.id().to_owned(),
                game_time: self.world.game_time,
                day_time: self.world.day_time,
                raining: weather && rain > 0.2,
                thundering: weather && thunder > 0.9,
            }),
        }
    }
    /// `Player.drop(stack, false, thrown_from_hand)`: the stack thrown from the eyes along the
    /// view, the main hand swinging for every drop (`LivingEntity.drop`; `PREDICTED` only keeps
    /// the swing from being sent back to the player who made it). `thrown_from_hand` names the
    /// player as the thrower, as the drop key does. The item's id, none for an empty stack or a
    /// toss a mod canceled.
    pub(super) fn drop_stack_from_eyes(
        &mut self,
        stack: ItemStack,
        thrown_from_hand: bool,
    ) -> Option<u32> {
        self.side_drop_stack_from_eyes(stack, thrown_from_hand)
    }
    /// The item entities `tick_dropped_items` took out, back in the level,
    /// before those its hooks spawned meanwhile (`addFreshEntity` appends).
    /// They keep their join numbers; the spawned ones got theirs on joining.
    fn put_back_dropped(&mut self, items: sunrise_client::join_seq::JoinList<ItemEntity>) {
        self.dropped.restore(
            items,
            sunrise_client::join_seq::RestoreOrder::SurvivorsFirst,
        );
    }
    /// Run vanilla block behaviour against the world through `LevelAccess`.
    fn with_level<R>(
        &mut self,
        f: impl FnOnce(&Behaviours, &mut GameLevel<'_>, &mut Ticks) -> R,
    ) -> R {
        self.ensure_stateful_entity_table();
        let result = self.side_with_level_access(false, f);
        self.side_after_with_level();
        result
    }

    /// Placement queries need neither trigger snapshots nor live factory setup or effect drains.
    fn with_placement_preview_level<R>(
        &mut self,
        f: impl FnOnce(&Behaviours, &GameLevel<'_>, &Ticks) -> R,
    ) -> R {
        self.side_with_level_access(true, |behaviours, level, ticks| f(behaviours, level, ticks))
    }

    pub(super) fn native_block_ignite_allowed(
        &mut self,
        pos: BlockPos,
        cause: &'static str,
    ) -> bool {
        self.with_level(|_, level, _| level.allow_block_ignite(pos, cause))
    }
    pub(super) fn native_cauldron_level_change_allowed(
        &mut self,
        pos: BlockPos,
        from: i32,
        to: i32,
        reason: &'static str,
    ) -> bool {
        self.with_level(|_, level, _| level.allow_cauldron_level_change(pos, from, to, reason))
    }
    /// Level.setBlock(pos, state, UPDATE_ALL) with neighbour/shape updates.
    fn level_set(&mut self, pos: [i32; 3], state: BlockStateId) -> Result<()> {
        self.level_set_with_flags(pos, state, level_flags::UPDATE_ALL)
    }
    /// Level.setBlock with the caller's update flags.
    fn level_set_with_flags(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        flags: i32,
    ) -> Result<()> {
        self.set_block_live(pos, state, flags, false);
        self.side_process_level_events()
    }

    /// `Level.setBlock` with the full server available at the block-entity drop site.
    fn set_block_live(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        flags: i32,
        create_entity: bool,
    ) -> bool {
        self.side_set_block_live(pos, state, flags, create_entity)
    }
    fn attack_block(&mut self, pos: [i32; 3]) -> Result<()> {
        let note = self.with_level(|b, level, ticks| {
            b.attack_block(level, ticks, BlockPos::new(pos[0], pos[1], pos[2]))
        });
        // NoteBlock.attack awards the statistic.
        if note {
            self.side_award_custom_stat("play_noteblock", 1);
        }
        self.side_process_level_events()
    }
    /// `AbstractCandleBlock.getParticleOffsets`, in sixteenths of a block:
    /// `CandleBlock.PARTICLE_OFFSETS` by its `candles` (one to four wicks),
    /// then `CandleCakeBlock.PARTICLE_OFFSETS`, whose one wick sits a block up.
    fn candle_offsets(name: &str, candles: Option<usize>) -> &'static [[u8; 3]] {
        const OFFSETS: [&[[u8; 3]]; 5] = [
            &[[8, 8, 8]],
            &[[6, 7, 8], [10, 8, 7]],
            &[[8, 5, 10], [6, 7, 8], [9, 8, 7]],
            &[[7, 5, 9], [10, 7, 9], [6, 7, 6], [9, 8, 6]],
            &[[8, 16, 8]],
        ];
        if name.ends_with("candle_cake") {
            OFFSETS[4]
        } else {
            OFFSETS[candles.unwrap_or(1).clamp(1, 4) - 1]
        }
    }
    /// `DistanceManager`'s tickets of the level in place, from every player in
    /// it (the local player, LAN guests, a server's players; a spectator the
    /// rule skips holds none), its forced chunks and its portal tickets. A game
    /// with neither a server nor a player (the headless fixtures) ticks
    /// everywhere.
    fn update_chunk_tickets(&mut self) {
        self.side_update_chunk_tickets();
    }

    /// Copies the moving-piston block entities into the world, where the
    /// collision queries and the renderer read them.
    ///
    /// A client runs this for every block change that ends a piston's move,
    /// so it stays linear in the pistons moving: a clock of a thousand pistons
    /// ends two thousand moves at once.
    fn sync_moving_pistons(&mut self) {
        use sunrise_client::fast_hash::{FxHashMap, FxHashSet};
        let previous = std::mem::take(&mut self.world.moving_pistons);
        let current: Vec<_> = self
            .block_ticks
            .moving_pistons
            .iter()
            .map(|(p, piston)| ([p.x, p.y, p.z], piston.shape_state()))
            .collect();
        // The positions drawn by `current` or `fallback` so far.
        let mut drawn: FxHashSet<[i32; 3]> = current.iter().map(|(p, _)| *p).collect();
        let mut fallback: Vec<_> = previous
            .into_iter()
            .filter(|(pos, _)| !drawn.contains(pos))
            .collect();
        drawn.extend(fallback.iter().map(|(p, _)| *p));
        // A finished piston stays drawn until the chunk mesh shows the block
        // it placed, however many ticks that takes. Pistons share a few
        // sections, so each section is asked once.
        let older = std::mem::take(&mut self.world.moving_piston_fallback);
        let mut pending: FxHashMap<(i32, i32, i32), bool> = FxHashMap::default();
        for (pos, piston) in older {
            if drawn.contains(&pos) {
                continue;
            }
            let Some(section) = self.section_of(pos) else {
                continue;
            };
            if *pending
                .entry(section)
                .or_insert_with(|| self.side_section_mesh_pending_in(section))
            {
                drawn.insert(pos);
                fallback.push((pos, piston));
            }
        }
        self.world.moving_piston_fallback = fallback;
        self.world.moving_pistons = current;
    }
    /// `DecoratedPotBlock.playerWillDestroy`: a pot mined with an item in
    /// `breaks_decorated_pots`, unless an enchantment prevents shattering, is
    /// cracked first (flags 260), so it shatters into its sherds.
    fn crack_mined_pot(&mut self, pos: [i32; 3], state: BlockStateId) -> BlockStateId {
        let registry = &self.pack.assets.registry;
        let Some(mut pot) = registry
            .state(state)
            .filter(|s| s.name == "minecraft:decorated_pot")
            .cloned()
        else {
            return state;
        };
        let Some(held) = self.held() else {
            return state;
        };
        let breaks = self
            .res
            .items()
            .item_tags
            .get("minecraft:breaks_decorated_pots")
            .is_some_and(|t| t.contains(&*held.item));
        let enchantments = &self.stations().enchantments;
        // `EnchantmentHelper.hasTag`.
        let kept = held
            .all_enchantments()
            .iter()
            .any(|(e, _)| enchantments.in_tag("minecraft:prevents_decorated_pot_shattering", e));
        if !breaks || kept {
            return state;
        }
        pot.properties.insert("cracked".into(), "true".into());
        let Ok(cracked) = registry.resolve(&pot) else {
            return state;
        };
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        self.with_level(|b, level, ticks| b.set_block(level, ticks, at, cracked, 260));
        cracked
    }
    /// The `ItemStack.interactLivingEntity` fallback in `Player.interactOn`,
    /// currently for native Mob entries after the entity's own action passed.
    fn item_interact_mob(&mut self, target: Option<InteractTarget>) -> bool {
        if self.spectator() || !mod_api::__private::has_part_hook(mod_api::HookId::ItemInteractMob)
        {
            return false;
        }
        let Some(index) = self.mob_interaction_target(target) else {
            return false;
        };
        let mob = &self.mobs[index];
        if mob.removed || !mob.alive() {
            return false;
        }
        let (entity, kind) = (
            mob.id as i32,
            sunrise_client::command::args::normalize(&mob.kind.id),
        );
        let Some(stack) = self.held().filter(|stack| stack.count > 0).cloned() else {
            return false;
        };
        let mut result = None;
        self.native_item_interact_mob(self.hand, entity, &kind, &stack, &mut result);
        result.is_some_and(|answer| self.interaction_consumed(answer))
    }
    /// A column whose surface is a full solid, non-fluid, non-leaf block open to the sky.
    fn land_column(&self, x: i32, z: i32) -> Option<i32> {
        if self.dim == Dim::Nether {
            // Below the bedrock ceiling: a solid floor with two air blocks above.
            for y in (32..120).rev() {
                let floor = self.res.facts().get(self.world.block(x, y - 1, z));
                if floor.full_collision
                    && floor.fluid == Fluid::None
                    && self.res.facts().is_air(self.world.block(x, y, z))
                    && self.res.facts().is_air(self.world.block(x, y + 1, z))
                {
                    return Some(y);
                }
            }
            return None;
        }
        let mut y = self.world.top_y() - 1;
        while y > self.world.bottom_y() {
            let s = self.world.block(x, y, z);
            if !self.res.facts().is_air(s) {
                let f = self.res.facts().get(s);
                let ok = f.full_collision
                    && f.fluid == Fluid::None
                    && !f.name.contains("leaves")
                    && f.hardness >= 0.0;
                return ok.then_some(y + 1);
            }
            y -= 1;
        }
        None
    }
    /// Landing rules (farmland trampling, turtle eggs) wrote or removed the block
    /// raw: finish `Level.setBlock`'s neighbour updates, so a crop on trampled
    /// farmland breaks. The local player's and every server player's landings.
    /// The level events they send go out too: a turtle egg's break particles
    /// and sound (`decreaseEggs`' 2001, `Level.destroyBlock`'s for the last).
    ///
    /// A mod hears the change first (`native_entity_change_block_pre`, Bukkit's
    /// `EntityChangeBlockEvent`): cancelling puts the block back and skips the
    /// updates and the events, so nothing the change would have caused happens -
    /// the crop on trampled farmland stays, because breaking it is the
    /// neighbour update below and not a write of its own.
    ///
    /// The game event the rule posts comes back for the caller to post with the
    /// landing player in place (`post_landing_vibration`).
    fn landing_block_updates(
        &mut self,
        player: Option<u128>,
        event: &sunrise_client::landing::FallEvent,
    ) -> Option<LandingVibration> {
        use sunrise_client::landing::FallEvent;
        use sunrise_world::block_landing::LandingEvent;
        let mut vibration = None;
        // The block the landing changed, what it replaced, and what it became.
        let (position, from, to, flags) = match *event {
            FallEvent::Write {
                position,
                from,
                state,
                flags,
            } => (position, from, state, flags as i32),
            FallEvent::Destroy {
                position, state, ..
            } => {
                // `Level.destroyBlock`: BLOCK_DESTROY with no source, after the
                // write (`TurtleEggBlock.decreaseEggs`' last egg).
                vibration = Some(LandingVibration {
                    event: "minecraft:block_destroy",
                    position,
                    state,
                    from_entity: false,
                });
                (position, state, self.air_state(), level_flags::UPDATE_ALL)
            }
            // `FarmlandBlock.turnToBaseBlock`'s BLOCK_CHANGE from the entity that
            // trampled it, and `TurtleEggBlock.decreaseEggs`' BLOCK_DESTROY
            // (`Context.of(state)`, no source): the write is already applied.
            FallEvent::BlockEvent {
                kind,
                position,
                state,
                with_entity,
            } => {
                // A cancelled change put the block back: nothing happened.
                let current = self.world.block(position.x, position.y, position.z);
                let cancelled = match kind {
                    LandingEvent::BlockChange => current != state,
                    LandingEvent::BlockDestroy => current == state,
                };
                if cancelled {
                    return None;
                }
                return Some(LandingVibration {
                    event: match kind {
                        LandingEvent::BlockChange => "minecraft:block_change",
                        LandingEvent::BlockDestroy => "minecraft:block_destroy",
                    },
                    position,
                    state,
                    from_entity: matches!(kind, LandingEvent::BlockChange) && with_entity,
                });
            }
            FallEvent::LevelEvent {
                event,
                position,
                data,
            } => {
                self.side_level_event_at(event, [position.x, position.y, position.z], data);
                return None;
            }
            _ => return None,
        };
        if self.landing_change_cancelled(player, position, from, to) {
            self.world
                .set(position.x, position.y, position.z, from)
                .ok();
            return None;
        }
        if matches!(event, FallEvent::Destroy { .. }) {
            self.side_level_event_at(2001, [position.x, position.y, position.z], from.0 as i32);
        }
        self.with_level(|b, level, ticks| b.notify_written(level, ticks, position, flags));
        vibration
    }
    /// Post a landing's block game event (`FarmlandBlock.turnToBaseBlock`'s
    /// BLOCK_CHANGE, `TurtleEggBlock.decreaseEggs`' BLOCK_DESTROY) as the player
    /// whose landing it was, who must be in place (`as_guest`). A sculk sensor, a
    /// calibrated sensor and a warden hear it.
    fn post_landing_vibration(&mut self, vibration: LandingVibration) {
        let at = [
            vibration.position.x,
            vibration.position.y,
            vibration.position.z,
        ];
        let who = vibration
            .from_entity
            .then_some(sunrise_client::mobs::Who::Player);
        self.side_post_block_game_event(vibration.event, at, who, Some(vibration.state));
    }
    /// `Blocks.AIR.defaultBlockState`: what a landing's destroy leaves.
    fn air_state(&self) -> BlockStateId {
        self.pack
            .assets
            .registry
            .default_state("minecraft:air")
            .unwrap_or(BlockStateId(0))
    }
    /// Whether a mod cancelled a landing's block change
    /// (`native_entity_change_block_pre`): asked with the two blocks' names and
    /// the player whose landing it was (`None` when no player did it).
    fn landing_change_cancelled(
        &mut self,
        player: Option<u128>,
        position: BlockPos,
        from: BlockStateId,
        to: BlockStateId,
    ) -> bool {
        self.side_landing_change_cancelled(player, position, from, to)
    }
    fn tick(&mut self) -> Result<()> {
        self.side_tick()
    }
    

    fn draw_reload_overlay(
        &self,
        batch: &mut HudBatch,
        bar: f32,
        fade: sunrise_client::level_loading::OverlayFade,
    ) {
        sunrise_client::level_loading::draw_loading_overlay(
            batch,
            &self.gui.atlas,
            bar,
            fade,
            self.settings.file.bool("darkMojangStudiosBackground"),
        );
    }
    /// An item frame's held item, apart from the frame (one id may mark one run).
    pub(super) const MOTION_FRAME_ITEM: u64 = 4 << 32;
    /// A block a piston moves, by the cell it moves to (`motion_at`).
    pub(super) const MOTION_MOVING_BLOCK: u64 = 6 << 32;
    /// The local player's flame vertices are tracked separately from the body run.
    const MOTION_OWN_PLAYER_FIRE: u64 = crate::game::MOTION_OWN_PLAYER | 1;

    /// The orb's sprite and the camera's axes, once a frame however many runs of orbs draw.
    pub(super) fn orb_frame(&self) -> OrbFrame {
        let sprite = self
            .gui
            .atlas
            .sprites
            .get("entity/experience/experience_orb")
            .copied();
        let (yaw, pitch) = (self.yaw.to_radians(), self.pitch.to_radians());
        let forward = glam::Vec3::new(
            -yaw.sin() * pitch.cos(),
            -pitch.sin(),
            yaw.cos() * pitch.cos(),
        );
        let right = forward.cross(glam::Vec3::Y).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        OrbFrame {
            sprite,
            forward,
            right,
            up,
        }
    }

    /// The brush's `ProjectileUtil.getHitResultOnViewVector` excludes only the source entity.
    pub(super) fn mob_hit_within(&self, reach: f64) -> Option<(usize, f64, [f64; 3])> {
        let source = self.side_pick_camera_id();
        let (eye, dir) = self.aim()?;
        let mut nearest = f64::INFINITY;
        let mut best: Option<(usize, f64, [f64; 3])> = None;
        for (i, m) in self.mobs.iter().enumerate() {
            if m.removed || Some(m.id) == source {
                continue;
            }
            if let Some(t) = ray_box(eye, dir, reach, m.bounds())
                && t != 0.0
                && t < nearest
            {
                nearest = t;
                let point = Vec3::new(
                    eye[0] + dir[0] * t,
                    eye[1] + dir[1] * t,
                    eye[2] + dir[2] * t,
                );
                best = Some((i, t, interact_location(point, m.position)));
            }
        }
        best
    }
    /// `Player.isSweepAttack` with the player's speed as it is, then
    /// `native_sweep_attack`, on either side: whether the hit on `target` sweeps.
    pub(super) fn sweep_decision(&mut self, scale: f32, crit: bool, target: &str) -> bool {
        let Some(p) = self.player.as_ref() else {
            return false;
        };
        let sword = self.held().is_some_and(|s| {
            self.res
                .items()
                .item_tags
                .get("minecraft:swords")
                .is_some_and(|tag| tag.contains(&*s.item))
        });
        let vanilla = p.is_sweep_attack(scale, crit, sword);
        let (mut sweeping, mut canceled) = (vanilla, false);
        // `setCanceled` skips later mods and keeps the current `sweeping`.
        self.native_sweep_attack(target, vanilla, &mut sweeping, &mut canceled);
        sweeping
    }
    /// Voxel traversal that also stops at fluid source blocks (bucket pick).
    fn pick_fluid(&self) -> Option<Hit> {
        let (origin, dir) = self.aim()?;
        let mut t = 0.0;
        let mut last = origin.map(|v| v.floor() as i32);
        let mut first = true;
        while t <= self.reach() {
            let p = [
                origin[0] + dir[0] * t,
                origin[1] + dir[1] * t,
                origin[2] + dir[2] * t,
            ];
            let b = p.map(|v| v.floor() as i32);
            // `BlockGetter.traverseBlocks` visits the block the ray starts in
            // first: the eyes' own water, under water (#3787).
            if first || b != last {
                let state = self.world.block(b[0], b[1], b[2]);
                let f = self.res.facts().get(state);
                let face = [last[0] - b[0], last[1] - b[1], last[2] - b[2]];
                if f.fluid != Fluid::None
                    && f.fluid_level == 8
                    && !f.fluid_falling
                    && !f.waterlogged
                    && f.render_shape != sunrise_client::blocks::RenderShape::Model
                {
                    if !first {
                        return Some(Hit {
                            pos: b,
                            face,
                            point: p,
                            inside: false,
                        });
                    }
                    let above = self.world.block(b[0], b[1] + 1, b[2]);
                    let same_above = self.res.facts().get(above).fluid == f.fluid;
                    if let Some((face, point)) =
                        clip_start_source(origin, dir, self.reach(), b, same_above)
                    {
                        return Some(Hit {
                            pos: b,
                            face,
                            point,
                            // A start in the shape hits at the eyes.
                            inside: point == origin,
                        });
                    }
                } else if !first && !self.res.facts().is_air(state) && f.fluid == Fluid::None {
                    // `ClipContext.Block.OUTLINE`: the ray stops at the block's
                    // outline, not its cell; it passes through the empty part
                    // of a torch, a fence or a flower to the water behind.
                    if let Some(hit) =
                        outline_stop(origin, dir, self.reach(), b, face, || self.outline_boxes(b))
                    {
                        return Some(hit);
                    }
                }
                last = b;
                first = false;
            }
            t += 0.05;
        }
        None
    }
    /// The cauldron at `pos`, and whether water is above it
    /// (`CauldronInteractions.isUnderWater`).
    pub(super) fn cauldron_at(
        &self,
        pos: [i32; 3],
    ) -> Option<(sunrise_world::chunk::BlockState, bool)> {
        let registry = self.pack.assets.registry;
        let id = self.world.block(pos[0], pos[1], pos[2]);
        let state = registry
            .state(id)
            .filter(|s| cauldron_interactions::snapshot().contains_block(&s.name))
            .cloned()?;
        let above = self.world.block(pos[0], pos[1] + 1, pos[2]);
        let under_water = cauldrons::is_under_water(self.res.facts(), above);
        Some((state, under_water))
    }

    /// What `BlockTransformer.transformBlock` would do with a held axe, hoe
    /// or shovel on the block at `pos`, without doing it: the new state, its
    /// sound, loot table, the durability it costs, its particle and its
    /// transform type.
    pub(super) fn block_transform(
        &self,
        stack: &ItemStack,
        hit: &Hit,
        sneaking: bool,
    ) -> Option<(BlockStateId, String, Option<String>, u32, String, String)> {
        let blocking = self
            .offhand
            .as_ref()
            .is_some_and(|s| self.blocks_attacks(s).is_some())
            && !sneaking;
        self.block_transform_in(
            stack,
            hit,
            TransformContext {
                world: &self.world,
                player: self.player.is_some().then(|| self.own_entity_id() as u32),
                hand: self.hand,
                blocking,
                simulated: false,
            },
        )
    }

    fn block_transform_in(
        &self,
        stack: &ItemStack,
        hit: &Hit,
        context: TransformContext<'_>,
    ) -> Option<(BlockStateId, String, Option<String>, u32, String, String)> {
        use sunrise_client::block_transform::Around;
        let TransformContext {
            world,
            player,
            hand,
            blocking,
            simulated,
        } = context;
        let Hit { pos, face, .. } = *hit;
        struct Column<'w> {
            world: &'w World,
            registry: &'w sunrise_world::registry::BlockRegistry,
            pos: [i32; 3],
        }
        impl Around for Column<'_> {
            fn state(&self, o: [i32; 3]) -> sunrise_world::chunk::BlockState {
                let id =
                    self.world
                        .block(self.pos[0] + o[0], self.pos[1] + o[1], self.pos[2] + o[2]);
                self.registry.state(id).cloned().unwrap_or_else(|| {
                    sunrise_world::chunk::BlockState {
                        name: "minecraft:air".into(),
                        properties: Default::default(),
                    }
                })
            }
        }
        let id = self
            .res
            .items()
            .get(&stack.item)
            .and_then(|i| i.block_transformer.clone())?;
        // playerHasBlockingItemUseIntent: a shield in the off hand is raised
        // instead, unless sneaking.
        if blocking {
            return None;
        }
        let face_name = match face {
            [0, -1, 0] => "down",
            [0, 1, 0] => "up",
            [0, 0, -1] => "north",
            [0, 0, 1] => "south",
            [-1, 0, 0] => "west",
            _ => "east",
        };
        let registry = self.pack.assets.registry;
        let block = world.block(pos[0], pos[1], pos[2]);
        let around = Column {
            world,
            registry,
            pos,
        };
        let vanilla = self
            .res
            .transformers()
            .apply(self.items(), registry, &id, face_name, &around)
            .and_then(|(_, t, state)| Some((registry.resolve(&state).ok()?, t)));
        let result = |new, t: &sunrise_client::block_transform::Transform| {
            (
                new,
                t.sound.clone(),
                t.loot.clone(),
                t.damage,
                t.particle.clone(),
                t.transform_type.clone(),
            )
        };
        let mapped = self.axe_mapped(&id, block);
        if mod_api::__private::mods().is_empty() {
            if let Some((new, ability)) = mapped {
                return Some(Self::tool_ability_result(new, ability));
            }
            let (new, t) = vanilla?;
            return Some(result(new, t));
        }
        use mod_api::ToolAbility::*;
        let actions: &[_] = match id.as_str() {
            "minecraft:axe" => &[AxeStrip, AxeScrape, AxeWaxOff],
            "minecraft:shovel" => &[ShovelFlatten],
            "minecraft:hoe" => &[HoeTill],
            _ => return vanilla.map(|(new, t)| result(new, t)),
        };
        for &ability in actions {
            let mut state = match mapped {
                Some((new, from)) => (from == ability).then_some(new),
                None => vanilla
                    .as_ref()
                    .and_then(|(state, t)| vanilla_tool_state(&id, &t.particle, ability, *state)),
            };
            let matches_vanilla = state.is_some() && mapped.is_none();
            let mut cancel = false;
            let tool = mod_api::ToolUse::from(mod_api::__private::built::ToolUse {
                player,
                hand,
                pos,
                face,
                click: hit.point,
                ability,
                simulated,
            });
            self.mod_tool_modification(stack, block, &tool, &mut state, &mut cancel);
            if cancel {
                return None;
            }
            if let Some(new) = state {
                if matches_vanilla {
                    let (_, t) = vanilla?;
                    return Some(result(new, t));
                }
                return Some(Self::tool_ability_result(new, ability));
            }
        }
        None
    }
    fn set_state_named(&mut self, pos: [i32; 3], name: &str, keep: &[&str]) -> Result<bool> {
        let registry = &self.pack.assets.registry;
        let old = self.world.block(pos[0], pos[1], pos[2]);
        let Some(default) = registry.default_state(name) else {
            return Ok(false);
        };
        let mut state = registry.state(default).cloned().ok_or("state")?;
        if let Some(previous) = registry.state(old) {
            for k in keep {
                if let Some(v) = previous.properties.get(*k) {
                    state.properties.insert((*k).to_owned(), v.clone());
                }
            }
        }
        let id = registry.resolve(&state).unwrap_or(default);
        self.level_set(pos, id)?;
        Ok(true)
    }
    pub(super) fn can_place_used_fire(&mut self, target: [i32; 3]) -> bool {
        let pos = BlockPos::new(target[0], target[1], target[2]);
        let direction = match ((self.yaw / 90.0 + 0.5).floor() as i32).rem_euclid(4) {
            0 => Direction::South,
            1 => Direction::West,
            2 => Direction::North,
            _ => Direction::East,
        };
        self.with_level(|b, level, ticks| b.fire_can_be_placed_at(level, ticks, pos, direction))
    }
    fn place_used_fire(&mut self, target: [i32; 3]) -> Result<()> {
        let pos = BlockPos::new(target[0], target[1], target[2]);
        let state = self.with_level(|b, level, ticks| b.fire_state_for(level, ticks, pos));
        self.level_set(target, state)
    }
    /// `Player.giveExperiencePoints`: `add_experience_levels` runs once for each
    /// level gained or lost, and the mods hear of the level change once.
    pub(super) fn give_experience_points(&mut self, points: i32) {
        self.side_give_experience_points(points);
    }

    pub(in crate::game) fn bed_facing(&self, head: [i32; 3]) -> i32 {
        let state = self.world.block(head[0], head[1], head[2]);
        let facing = self
            .pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get("facing"))
            .map(String::as_str);
        sunrise_client::mobs::villager::bed_facing_code(facing)
    }

    /// Clear all entity collections shared by server and client games.
    pub(in crate::game) fn clear_shared_entities(&mut self) {
        self.mobs.clear();
        self.dropped.clear();
        self.orbs.clear();
        self.creatures.projectiles.clear();
        self.falling.clear();
        self.tnts.clear();
        self.boats.clear();
        self.carts.clear();
        self.armor_stands.clear();
        self.cushions.clear();
        self.item_frames.clear();
        self.paintings.clear();
        self.dragon.discard();
        self.crystals.clear();
        self.breath_clouds.clear();
        self.tridents.clear();
        self.rockets.clear();
        self.bolts.clear();
        self.ominous_spawners.clear();
        self.fangs.list.clear();
        self.potions.clear_thrown();
        self.potions.clouds.clear();
        self.end.clear_remote_eyes();
        self.riding = None;
        self.persist = Default::default();
        self.entity_attachments.clear();
    }

    /// The non-air blocks of a box in this world, with counts per block.
    pub(in crate::game) fn blocks_json(&self, lo: [i32; 3], hi: [i32; 3]) -> serde_json::Value {
        let mut list = Vec::new();
        let mut census: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    let state = self.world.block(x, y, z);
                    if self.res.facts().is_air(state) {
                        continue;
                    }
                    let name = self.res.facts().get(state).name.to_string();
                    *census.entry(name).or_insert(0) += 1;
                    list.push(serde_json::json!([x, y, z, self.state_string(state)]));
                }
            }
        }
        serde_json::json!({ "from": lo, "to": hi, "counts": census, "blocks": list })
    }

    pub(in crate::game) fn state_string(&self, state: BlockStateId) -> String {
        self.block_state_string(state).unwrap_or_else(|| "?".into())
    }

    pub(in crate::game) fn vanilla_root(&self) -> PathBuf {
        if self.ui.vanilla_root.as_os_str().is_empty() {
            self.options.resources.clone()
        } else {
            self.ui.vanilla_root.clone()
        }
    }
}

#[cfg(test)]
impl crate::game::ClientGame<'_> {
    #[cfg(test)]
    fn hud(&mut self, size: [u32; 2]) -> HudBatch {
        self.hud_with_reload(size, None)
    }
}

/// `mark_motion` ids of entities that aren't mobs or dropped items (boats, minecarts, armor
/// stands, item frames, paintings, primed TNT, falling blocks, rockets, tridents, thrown
/// potions): their entity id under this tag.
pub(super) const MOTION_OBJECT: u64 = 3 << 32;

/// A block entity, by its position (`motion_block_entity`).
pub(super) const MOTION_BLOCK_ENTITY: u64 = 5 << 32;

/// The player's own model (third person, and a shader pack's shadow pass).
pub(super) const MOTION_OWN_PLAYER: u64 = 7 << 32;

/// `computeNextAutosaveInterval`'s arithmetic: `Math.max(100, (int)(rate * 300.0F))`.
fn autosave_interval(rate: f32) -> i64 {
    i64::from((rate * 300.0) as i32).max(100)
}

/// winit's events for a window's game, with any loop: a headless client (#4951) has no
/// winit loop and calls these from its own.
impl<__Side: crate::game::Side> Game<'_, __Side> {}
/// misc/forcefield.png for the world border wall (square, power of two).
fn load_forcefield(root: &sunrise_assets::resource_source::ResourceSource) -> (u32, u32, Vec<u8>) {
    root.read("assets/minecraft/textures/misc/forcefield.png")
        .map_err(Into::into)
        .and_then(|bytes| sunrise_client::hud::decode_bytes(&bytes))
        .ok()
        .filter(|(w, h, _)| w == h && w.is_power_of_two())
        .unwrap_or((1, 1, vec![0; 4]))
}
struct ServerConstructPrep {
    level: Level,
    save: Arc<SaveDir>,
    painting_variants: paintings::PaintingVariants,
    item_features: sunrise_items::creative_catalog::ItemFeatureRequirements,
    creative_catalog: Arc<sunrise_items::creative_catalog::Catalog>,
    world: World,
    now: Instant,
    pack: PackResources,
    gui: UiResources,
    options: Options,
    settings: Settings,
    streaming: Streaming,
}

struct ClientConstructPrep {
    level: Level,
    painting_variants: paintings::PaintingVariants,
    armor_stand_assets: armor_stands::ArmorStandAssets,
    item_features: sunrise_items::creative_catalog::ItemFeatureRequirements,
    creative_screen: Creative,
    client_prepared: Box<client_state::ClientCpuResources>,
    world: World,
    tutorial: tutorial::Tutorial,
    now: Instant,
    particles: sunrise_client::particles::ParticleEngine,
    resource_source: sunrise_assets::resource_source::ResourceSource,
    pack: PackResources,
    gui: UiResources,
    options: Options,
    settings: Settings,
    audio: Audio,
    streaming: Streaming,
    standalone_geometry: native_block_entity_draw::StandaloneGeometry,
}

struct ServerConstructEffects {
    structure_spawns: sunrise_client::mobs::spawn_selection::StructureSpawns,
    clock: FixedStep,
    tick: sunrise_client::parallel::TickThread,
    skin_profiles: RefCell<sunrise_client::skins::Resolver>,
    profile: profile_capture::Profile,
    ui: chat::UiState,
}

struct ClientConstructEffectsBundle {
    clock: FixedStep,
    tick: sunrise_client::parallel::TickThread,
    skin_downloads: RefCell<sunrise_client::skins::Downloader>,
    skin_profiles: RefCell<sunrise_client::skins::Resolver>,
    profile: profile_capture::Profile,
    registered_debug_entries: std::sync::Arc<[sunrise_client::debug_entries::RegisteredDebugEntry]>,
    debug_entries: sunrise_client::debug_entries::DebugEntries,
    ui: chat::UiState,
    client: Box<client_state::ClientConstructEffects>,
}

pub(in crate::game) struct ClientSidePrep {
    pub(in crate::game) audio: client_side::ClientShared<Audio>,
    pub(in crate::game) particles:
        client_side::ClientShared<sunrise_client::particles::ParticleEngine>,
    pub(in crate::game) armor_stand_assets:
        client_side::ClientOnly<armor_stands::ArmorStandAssets>,
    pub(in crate::game) creative_screen: client_side::ClientOnly<Creative>,
    pub(in crate::game) skin_downloads:
        client_side::ClientOnly<RefCell<sunrise_client::skins::Downloader>>,
    pub(in crate::game) registered_debug_entries: client_side::ClientOnly<
        std::sync::Arc<[sunrise_client::debug_entries::RegisteredDebugEntry]>,
    >,
    pub(in crate::game) debug_entries:
        client_side::ClientOnly<sunrise_client::debug_entries::DebugEntries>,
}

// Out-of-line init bundles for wrapped Game fields so finish_construct's unoptimised stack stays small.
pub(in crate::game) struct WrappedClientInit {
    pending_entry: client_side::ClientOnly<Option<PendingWorldEntry>>,
    pack_reload: client_side::ClientOnly<Option<packs::PackReload>>,
    server_packs: client_side::ClientOnly<server_packs::ServerPackState>,
    client_reports: client_side::ClientOnly<Arc<sunrise_client::chunk_io::Reports>>,
    command_prompt: client_side::ClientOnly<Option<remote_chat::CommandPrompt>>,
    dialogs: client_side::ClientOnly<dialogs::DialogState>,
    pending_initial_models:
        client_side::ClientOnly<Option<Box<initial_client_models::PendingClientStart>>>,
    stitching_block_atlas: client_side::ClientOnly<Option<Arc<RenderAssets>>>,
    lod: client_side::ClientOnly<Option<sunrise_client::lod::Lod>>,
    lod_blue_map: client_side::ClientOnly<Option<Arc<sunrise_client::bluemap::BlueMap>>>,
    video_modes: client_side::ClientOnly<sunrise_client::window::VideoModeList>,
    screen_state: client_side::ClientOnly<screen_build::ScreenState>,
    window: client_side::ClientShared<Option<Arc<Window>>>,
    skin_models:
        client_side::ClientOnly<std::cell::RefCell<sunrise_client::mobs::skin::SkinModels>>,
    lent_level_random: client_side::ClientOnly<
        Option<std::rc::Rc<std::cell::RefCell<mod_api::LegacyRandomSource>>>,
    >,
    pad: client_side::ClientShared<pad::Pads>,
    live_scenes: client_side::ClientOnly<std::cell::RefCell<native_scene_live::LiveScenes>>,
    debug_renderers:
        client_side::ClientOnly<Vec<sunrise_client::debug_entries::ActiveDebugRenderer>>,
    pickups: client_side::ClientOnly<pickups::Pickups>,
    creation_job:
        client_side::ClientOnly<Option<std::thread::JoinHandle<std::result::Result<(), String>>>>,
    title: client_side::ClientShared<Option<Title>>,
    text_displays:
        client_side::ClientOnly<indexmap::IndexMap<i32, text_displays::RemoteTextDisplay>>,
    recipe_ui: client_side::ClientOnly<recipe_book::RecipeUi>,
    spawners: client_side::ClientOnly<spawner::SpawnerState>,
    mod_screen: client_side::ClientOnly<Option<native_hooks::OpenNativeScreen>>,
    pending_screenshot: client_side::ClientOnly<Option<std::sync::mpsc::Sender<String>>>,
    screenshot_sequence: client_side::ClientOnly<Option<screenshot_sequence::Sequence>>,
    pending_render_capture: client_side::ClientOnly<Option<std::sync::mpsc::Sender<String>>>,
    pending_bench:
        client_side::ClientOnly<Option<(Instant, std::sync::mpsc::Sender<String>, bool)>>,
    flight: client_side::ClientOnly<Option<control::Flight>>,
    ui_wait: client_side::ClientOnly<ui_control::UiWait>,
    input_trace: client_side::ClientOnly<input_trace::InputTrace>,
    repro: client_side::ClientOnly<repro::Repro>,
    occlusion: client_side::ClientOnly<entity_culling::EntityOcclusion>,
    async_particle_culler: client_side::ClientOnly<RefCell<particle_culling::ParticleAsyncState>>,
    pending_uploads: client_side::ClientOnly<Vec<sunrise_client::streaming::MeshUpload>>,
    pending_uploads_merge: client_side::ClientOnly<Vec<sunrise_client::streaming::MeshUpload>>,
    mod_jobs: client_side::ClientOnly<mod_browser::Jobs>,
    client_item_use: client_side::ClientShared<Option<native_claimed_items::ActiveClientItemUse>>,
    client_tasks: client_side::ClientOnly<sunrise_client::net::ClientTasks>,
    pending_client_tasks:
        client_side::ClientOnly<std::collections::VecDeque<sunrise_client::net::ClientTask>>,
    world_options: client_side::ClientOnly<Option<lan_screen::WorldOptions>>,
    command_block_screen: client_side::ClientOnly<Option<command_blocks::CommandBlockScreen>>,
    block_editor: client_side::ClientOnly<Option<block_editor::BlockEditScreen>>,
    clipboard: client_side::ClientOnly<sunrise_client::clipboard::SystemClipboard>,
    pending_save: client_side::ClientOnly<Option<saving::ClosingSave>>,
    tab_list: client_side::ClientOnly<sunrise_client::tab_list::TabList>,
    adv_ui: client_side::ClientOnly<advancements_screen::AdvUi>,
}

pub(in crate::game) struct WrappedServerInit {
    player_autosaves: server_side::ServerOnly<sunrise_client::lan::playerdata::Autosaves>,
    session_lock: server_side::ServerShared<Option<sunrise_client::save::SessionLock>>,
    paused_mob_hurt: server_side::ServerOnly<Option<Box<sunrise_client::mobs::PausedMobHurt>>>,
    raiding: server_side::ServerOnly<raids::Raiding>,
    world_tables: server_side::ServerShared<Option<WorldTables>>,
    server_loot: server_side::ServerOnly<sunrise_client::structure_blocks::LootSnapshot>,
    levels: server_side::ServerOnly<levels::Levels>,
    tickets: server_side::ServerShared<sunrise_client::ticket_levels::ChunkTickets>,
    random_tick_sections:
        server_side::ServerOnly<sunrise_client::random_tick_sections::RandomTickSections>,
    spill: server_side::ServerOnly<spill::Spill>,
    lent_mobs: server_side::ServerOnly<RefCell<native_unspawned_mob::LentMobs>>,
    test_defs: server_side::ServerOnly<
        std::cell::RefCell<HashMap<String, Option<test_instances::TestDef>>>,
    >,
    game_tests: server_side::ServerOnly<game_tests::GameTests>,
    native_commands: server_side::ServerOnly<native_server::NativeCommandState>,
    lan_building_boxes: server_side::ServerOnly<building_players::BuildingPlayers>,
    lan_displaced_players: server_side::ServerOnly<Vec<lan_actions::DisplacedPlayer>>,
    fake_action: server_side::ServerOnly<std::sync::Arc<native_server::FakeAction>>,
    server_lists:
        server_side::ServerOnly<Option<std::sync::Arc<std::sync::Mutex<dedicated::Lists>>>>,
    cmds: server_side::ServerOnly<commands::extra::CommandExtras>,
    reload_resources: server_side::ServerOnly<mod_api::RetainedListeners>,
    combat: server_side::ServerOnly<combat::CombatTracker>,
    guest_combat: server_side::ServerOnly<std::collections::HashMap<u32, combat::CombatTracker>>,
    fns: server_side::ServerOnly<functions::FnState>,
    exec: server_side::ServerOnly<commands::execute::ExecState>,
}

pub(in crate::game) struct WrappedRendererInit {
    renderer: client_side::ClientShared<Option<Box<Renderer>>>,
}

struct WrappedPrepInit {
    item_features: sunrise_items::creative_catalog::ItemFeatureRequirements,
    skin_profiles: RefCell<sunrise_client::skins::Resolver>,
    level: Level,
    painting_variants: paintings::PaintingVariants,
    world: World,
    now: Instant,
    pack: PackResources,
    gui: UiResources,
    options: Options,
    settings: Settings,
    streaming: Streaming,
    clock: FixedStep,
    tick: sunrise_client::parallel::TickThread,
    profile: profile_capture::Profile,
    ui: chat::UiState,
}

// Const-generic impl keeps constructor helpers out of line without creating replacement slots.
struct ConstructInit<const NO_SLOT: bool>;

impl<const NO_SLOT: bool> ConstructInit<NO_SLOT> {
    #[inline(never)]
    fn build_wrapped_client() -> Box<WrappedClientInit> {
        Box::new(WrappedClientInit {
            pending_entry: client_side::ClientOnly::new(None),
            pack_reload: client_side::ClientOnly::new(None),
            server_packs: client_side::ClientOnly::new(Default::default()),
            client_reports: client_side::ClientOnly::new(Arc::default()),
            command_prompt: client_side::ClientOnly::new(None),
            dialogs: client_side::ClientOnly::new(Default::default()),
            pending_initial_models: client_side::ClientOnly::new(None),
            stitching_block_atlas: client_side::ClientOnly::new(None),
            lod: client_side::ClientOnly::new(None),
            lod_blue_map: client_side::ClientOnly::new(None),
            video_modes: client_side::ClientOnly::new(Default::default()),
            screen_state: client_side::ClientOnly::new(screen_build::ScreenState::default()),
            window: client_side::ClientShared::new(None),
            skin_models: client_side::ClientOnly::new(Default::default()),
            lent_level_random: client_side::ClientOnly::new(None),
            pad: client_side::ClientShared::new(Default::default()),
            live_scenes: client_side::ClientOnly::new(Default::default()),
            debug_renderers: client_side::ClientOnly::new(Vec::new()),
            pickups: client_side::ClientOnly::new(Default::default()),
            creation_job: client_side::ClientOnly::new(None),
            title: client_side::ClientShared::new(None),
            text_displays: client_side::ClientOnly::new(Default::default()),
            recipe_ui: client_side::ClientOnly::new(Default::default()),
            spawners: client_side::ClientOnly::new(Default::default()),
            mod_screen: client_side::ClientOnly::new(None),
            pending_screenshot: client_side::ClientOnly::new(None),
            screenshot_sequence: client_side::ClientOnly::new(None),
            pending_render_capture: client_side::ClientOnly::new(None),
            pending_bench: client_side::ClientOnly::new(None),
            flight: client_side::ClientOnly::new(None),
            ui_wait: client_side::ClientOnly::new(Default::default()),
            input_trace: client_side::ClientOnly::new(Default::default()),
            repro: client_side::ClientOnly::new(Default::default()),
            occlusion: client_side::ClientOnly::new(Default::default()),
            async_particle_culler: client_side::ClientOnly::new(RefCell::new(Default::default())),
            pending_uploads: client_side::ClientOnly::new(Vec::new()),
            pending_uploads_merge: client_side::ClientOnly::new(Vec::new()),
            mod_jobs: client_side::ClientOnly::new(Default::default()),
            client_item_use: client_side::ClientShared::new(None),
            client_tasks: client_side::ClientOnly::new(Default::default()),
            pending_client_tasks: client_side::ClientOnly::new(Default::default()),
            world_options: client_side::ClientOnly::new(None),
            command_block_screen: client_side::ClientOnly::new(None),
            block_editor: client_side::ClientOnly::new(None),
            clipboard: client_side::ClientOnly::new(Default::default()),
            pending_save: client_side::ClientOnly::new(None),
            tab_list: client_side::ClientOnly::new(Default::default()),
            adv_ui: client_side::ClientOnly::new(Default::default()),
        })
    }
    #[inline(never)]
    fn build_wrapped_renderer() -> Box<WrappedRendererInit> {
        Box::new(WrappedRendererInit {
            renderer: client_side::ClientShared::new(None),
        })
    }
    #[inline(never)]
    fn build_be_anims(
        registry: &'static sunrise_world::registry::BlockRegistry,
    ) -> sunrise_client::block_entity_anim::BlockEntityAnims {
        let mut anims = sunrise_client::block_entity_anim::BlockEntityAnims::default();
        anims.classify(registry);
        anims
    }
    #[inline(never)]
    fn build_loop_context(
        res: &Resources,
        loop_side: sunrise_world::native_load_context::LoopSide,
    ) -> Arc<sunrise_world::native_load_context::LoopContext> {
        Arc::new(res.core.item_data.context().loop_context(loop_side, []))
    }
    #[inline(never)]
    fn build_wrapped_server() -> Box<WrappedServerInit> {
        Box::new(WrappedServerInit {
            player_autosaves: server_side::ServerOnly::new(Default::default()),
            session_lock: server_side::ServerShared::new(None),
            paused_mob_hurt: server_side::ServerOnly::new(None),
            raiding: server_side::ServerOnly::new(Default::default()),
            world_tables: server_side::ServerShared::new(None),
            server_loot: server_side::ServerOnly::new(Default::default()),
            levels: server_side::ServerOnly::new(Default::default()),
            tickets: server_side::ServerShared::new(Default::default()),
            random_tick_sections: server_side::ServerOnly::new(Default::default()),
            spill: server_side::ServerOnly::new(Default::default()),
            lent_mobs: server_side::ServerOnly::new(Default::default()),
            test_defs: server_side::ServerOnly::new(std::cell::RefCell::new(HashMap::new())),
            game_tests: server_side::ServerOnly::new(game_tests::GameTests::default()),
            native_commands: server_side::ServerOnly::new(Default::default()),
            lan_building_boxes: server_side::ServerOnly::new(Default::default()),
            lan_displaced_players: server_side::ServerOnly::new(Vec::new()),
            fake_action: server_side::ServerOnly::new(Default::default()),
            server_lists: server_side::ServerOnly::new(None),
            cmds: server_side::ServerOnly::new(Default::default()),
            reload_resources: server_side::ServerOnly::new(Default::default()),
            combat: server_side::ServerOnly::new(Default::default()),
            guest_combat: server_side::ServerOnly::new(Default::default()),
            fns: server_side::ServerOnly::new(Default::default()),
            exec: server_side::ServerOnly::new(Default::default()),
        })
    }
    #[inline(never)]
    fn wrap_server_prep_and_effects(
        prep: Box<ServerConstructPrep>,
        effects: Box<ServerConstructEffects>,
    ) -> (Box<WrappedPrepInit>, Box<ServerState>) {
        let ws = Self::build_wrapped_server();
        let server = ServerState::build(
            effects.structure_spawns,
            server_side::ServerShared::new(prep.save),
            server_side::ServerOnly::new(prep.creative_catalog),
            ws,
        );
        let wp = Box::new(WrappedPrepInit {
            item_features: prep.item_features,
            skin_profiles: effects.skin_profiles,
            level: prep.level,
            painting_variants: prep.painting_variants,
            world: prep.world,
            now: prep.now,
            pack: prep.pack,
            gui: prep.gui,
            options: prep.options,
            settings: prep.settings,
            streaming: prep.streaming,
            clock: effects.clock,
            tick: effects.tick,
            profile: effects.profile,
            ui: effects.ui,
        });
        (wp, server)
    }
    #[inline(never)]
    fn wrap_client_prep_and_effects(
        prep: Box<ClientConstructPrep>,
        effects: Box<ClientConstructEffectsBundle>,
    ) -> (Box<WrappedPrepInit>, Box<ClientState>) {
        let music_toast = prep.settings.file.int("musicToast").clamp(0, 2) as u8;
        let wc = Self::build_wrapped_client();
        let wr = Self::build_wrapped_renderer();
        let extra = Box::new(ClientSidePrep {
            audio: client_side::ClientShared::new(prep.audio),
            particles: client_side::ClientShared::new(prep.particles),
            armor_stand_assets: client_side::ClientOnly::new(prep.armor_stand_assets),
            creative_screen: client_side::ClientOnly::new(prep.creative_screen),
            skin_downloads: client_side::ClientOnly::new(effects.skin_downloads),
            registered_debug_entries: client_side::ClientOnly::new(
                effects.registered_debug_entries,
            ),
            debug_entries: client_side::ClientOnly::new(effects.debug_entries),
        });
        let client = ClientState::build(
            prep.now,
            music_toast,
            prep.tutorial,
            prep.standalone_geometry,
            prep.client_prepared,
            effects.client,
            extra,
            wc,
            wr,
        );
        let wp = Box::new(WrappedPrepInit {
            item_features: prep.item_features,
            skin_profiles: effects.skin_profiles,
            level: prep.level,
            painting_variants: prep.painting_variants,
            world: prep.world,
            now: prep.now,
            pack: prep.pack,
            gui: prep.gui,
            options: prep.options,
            settings: prep.settings,
            streaming: prep.streaming,
            clock: effects.clock,
            tick: effects.tick,
            profile: effects.profile,
            ui: effects.ui,
        });
        (wp, client)
    }
    #[inline(never)]
    #[allow(clippy::boxed_local)]
    fn assemble_game<'a, S>(
        res: &'a Arc<Resources>,
        wp: Box<WrappedPrepInit>,
        side: Box<S>,
    ) -> Game<'a, S> {
        let loop_side = wp.options.execution_role.loop_side();
        let mod_sources = wp.options.mods.clone();
        let registry = wp.pack.assets.registry;
        let creative = wp.options.creative;
        let inventory = vec![None; 36];
        let look = wp.options.look;
        let yaw = look.map_or(wp.level.rotation[0], |l| l.0);
        let pitch = look.map_or(wp.level.rotation[1], |l| l.1);
        Game {
            world_biomes: None,
            world_biomes_base: None,
            res: res.clone(),
            rest_loader: None,
            integrated_starting: None,
            collision: res.collision(),
            motion: res.motion(),
            health_resources: res.motion().health.clone(),
            damage_tag_generation: 1,
            pack: wp.pack,
            gui: wp.gui,
            world: wp.world,
            streaming: wp.streaming,
            options: wp.options,
            player: None,
            spawn: None,
            clock: wp.clock,
            previous_position: Vec3::ZERO,
            eye: 1.62,
            yaw,
            pitch,
            inventory,
            selected: 0,
            carried: None,
            craft_grid: vec![None; 9],
            craft_size: 2,
            container: None,
            station: None,
            potions: Default::default(),
            scratch_slot: None,
            mining: None,
            use_delay: 0,
            creative,
            mode: if creative {
                sunrise_client::command::args::GameMode::Creative
            } else {
                sunrise_client::command::args::GameMode::Survival
            },
            pause: false,
            previous: wp.now,
            ticks: 0,
            error: None,
            level: wp.level,
            loop_context: Self::build_loop_context(res, loop_side),
            tick: wp.tick,
            setting_overrides: Default::default(),
            damage_hooks: native_hooks::DAMAGE_HOOKS,
            dropped: Default::default(),
            mobs: Default::default(),
            mob_index: Default::default(),
            creatures: Default::default(),
            be_anims: Self::build_be_anims(registry),
            using: None,
            use_duration: 0,
            use_start: Default::default(),
            armor: [None, None, None, None],
            offhand: None,
            equipment: Default::default(),
            hand: Hand::Main,
            cooldowns: Default::default(),
            autosaving: false,
            experience: Experience::default(),
            orbs: Default::default(),
            swing: Default::default(),
            swings_started: 0,
            clicks: Default::default(),
            settings: wp.settings,
            flash: 0,
            sleeping: None,
            sleep_timer: 0,
            dim: Dim::Overworld,
            world_flat: false,
            portals: Default::default(),
            end: Default::default(),
            block_ticks: Ticks::new(0),
            level_events: Vec::new(),
            falling: Default::default(),
            paintings: Default::default(),
            painting_variants: wp.painting_variants,
            item_frames: Default::default(),
            armor_stands: Default::default(),
            cushions: Default::default(),
            boats: Default::default(),
            carts: Default::default(),
            bolts: Default::default(),
            maps: maps::Maps::default(),
            dragon: Default::default(),
            crystals: Default::default(),
            breath_clouds: Default::default(),
            skin_profiles: wp.skin_profiles,
            riding: None,
            posed_rider: None,
            pending: Default::default(),
            hook: Default::default(),
            tridents: Default::default(),
            rockets: Default::default(),
            ominous_spawners: Default::default(),
            tnts: Default::default(),
            transactions: Default::default(),
            spawner_tick_positions: Vec::new(),
            trial_spawner_tick_positions: Vec::new(),
            loop_states: Default::default(),
            item_features: wp.item_features,
            creative_contents: Default::default(),
            entity_initialization: Default::default(),
            kinetic_feedback: HashMap::new(),
            control: None,
            perf: Default::default(),
            events: Default::default(),
            profile: wp.profile,
            agent: agent::Agent::default(),
            mods: mod_sources,
            recipe_tables: Default::default(),
            native_lifecycle: Default::default(),
            resident_entities: Default::default(),
            native_configs: Default::default(),
            native_net: Default::default(),
            sky_darken_memo: Default::default(),
            persist: Default::default(),
            entity_attachments: Default::default(),
            collider_placements: Default::default(),
            collider_histories: Default::default(),
            attachment_indexes: Default::default(),
            remote: None,
            realm_session: false,
            lan_session: false,
            replay: Default::default(),
            lan: None,
            integrated: None,
            serving: None,
            #[cfg(feature = "friends")]
            friends: None,
            passages: Default::default(),
            previous_mode: None,
            entity_reach: 3.0,
            block_reach: 4.5,
            player_state: Default::default(),
            server_items: None,
            ui: wp.ui,
            mount: Default::default(),
            stats: Default::default(),
            score: Default::default(),
            border: Default::default(),
            adv: Default::default(),
            fangs: Default::default(),
            side: *side,
        }
    }
}

impl<'a> ServerGame<'a> {
    /// A logical server game (dedicated server or integrated server thread).
    pub(in crate::game) fn new(
        res: &'a Arc<Resources>,
        pack: PackResources,
        gui: UiResources,
        options: Options,
        settings: Settings,
        prepared: Option<GameCpuResources>,
    ) -> Result<Self> {
        debug_assert!(options.execution_role.is_logical_server());
        let prep = Self::prepare_construct(res, pack, gui, options, settings, prepared)?;
        let effects = Self::prepare_effects(&prep);
        Ok(Self::finish_construct(res, prep, effects))
    }

    #[inline(never)]
    fn prepare_construct(
        res: &'a Arc<Resources>,
        pack: PackResources,
        gui: UiResources,
        options: Options,
        settings: Settings,
        prepared: Option<GameCpuResources>,
    ) -> Result<Box<ServerConstructPrep>> {
        #[cfg(test)]
        Self::initialize_mod_runtime_for_tests();
        // A scoped hook on this thread asks this game's parts.
        mod_api::__private::mark_game_thread();
        let root = options.resources.clone();
        let registry = pack.assets.registry;
        let level = Level::default();
        let save = Arc::new(SaveDir::new(
            std::env::current_dir()?
                .join("saves")
                .join(".sunrise-title"),
            sunrise_client::save::LevelShape::OVERWORLD,
            None,
        ));
        let prepared = prepared.map(Ok).unwrap_or_else(|| {
            GameCpuResources::load(
                &sunrise_assets::resource_source::ResourceRoot::new(
                    root.clone(),
                    packs::cache_dir(),
                    pack.source.clone(),
                ),
                None,
                pack.render.model_library.as_deref().ok(),
                !options.execution_role.is_dedicated_server(),
            )
        })?;
        let world = crate::game::authoritative_world_for_mods(res, 0);
        let now = Instant::now();
        // Until a world is opened the streamer is idle.
        let mut streaming = Streaming::new(
            Arc::new(TestTerrain::new(registry, res.biomes())?),
            pack.context.clone(),
            1,
        );
        streaming.paused = true;
        let painting_variants = prepared.painting_variants;
        let item_features = prepared.item_features;
        let creative_catalog = prepared.creative_screen.catalog().clone();
        Ok(Box::new(ServerConstructPrep {
            level,
            save,
            painting_variants,
            item_features,
            creative_catalog,
            world,
            now,
            pack,
            gui,
            options,
            settings,
            streaming,
        }))
    }

    #[inline(never)]
    fn prepare_effects(prep: &ServerConstructPrep) -> Box<ServerConstructEffects> {
        let slow_frame_ms = prep
            .options
            .spikes
            .unwrap_or(sunrise_client::client_perf::SLOW_FRAME_MS);
        #[cfg(not(test))]
        let structure_spawns =
            sunrise_client::mobs::spawn_selection::StructureSpawns::load(&prep.options.resources);
        #[cfg(test)]
        let structure_spawns = resource_load::cached_structure_spawns(&prep.options.resources);
        let clock = FixedStep::default();
        let tick = sunrise_client::parallel::TickThread::mint();
        let skin_profiles = RefCell::new(sunrise_client::skins::Resolver::new());
        let profile = profile_capture::Profile::new(slow_frame_ms);
        let ui = chat::UiState::new();
        Box::new(ServerConstructEffects {
            structure_spawns,
            clock,
            tick,
            skin_profiles,
            profile,
            ui,
        })
    }

    #[inline(never)]
    fn finish_construct(
        res: &'a Arc<Resources>,
        prep: Box<ServerConstructPrep>,
        effects: Box<ServerConstructEffects>,
    ) -> Self {
        let (wp, server) = ConstructInit::<true>::wrap_server_prep_and_effects(prep, effects);
        let mut game = ConstructInit::<true>::assemble_game(res, wp, server);
        game.reset_entity_residency();
        game
    }
}

impl<'a> ClientGame<'a> {
    /// A client game with no world open, over the loaded resources.
    #[allow(dead_code)]
    pub(in crate::game) fn new(
        res: &'a Arc<Resources>,
        pack: PackResources,
        gui: UiResources,
        options: Options,
        settings: Settings,
        audio: Audio,
        prepared: Option<GameCpuResources>,
    ) -> Result<Self> {
        Self::construct(res, pack, gui, options, settings, audio, prepared, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::game) fn construct(
        res: &'a Arc<Resources>,
        pack: PackResources,
        gui: UiResources,
        options: Options,
        settings: Settings,
        audio: Audio,
        prepared: Option<GameCpuResources>,
        initial: Option<&mut Option<initial_client_models::InitialModelGeneration>>,
    ) -> Result<Self> {
        debug_assert!(!options.execution_role.is_logical_server());
        // Preparation, effectful init, and struct assembly live in separate out-of-line frames to keep unoptimised stack small.
        let prep =
            Self::prepare_construct(res, pack, gui, options, settings, audio, prepared, initial)?;
        let effects = Self::prepare_effects(&prep);
        Ok(Self::finish_construct(res, prep, effects))
    }

    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn prepare_construct(
        res: &'a Arc<Resources>,
        pack: PackResources,
        gui: UiResources,
        options: Options,
        settings: Settings,
        audio: Audio,
        prepared: Option<GameCpuResources>,
        initial: Option<&mut Option<initial_client_models::InitialModelGeneration>>,
    ) -> Result<Box<ClientConstructPrep>> {
        #[cfg(test)]
        Self::initialize_mod_runtime_for_tests();
        // A scoped hook on this thread asks this game's parts.
        mod_api::__private::mark_game_thread();
        let root = options.resources.clone();
        let registry = pack.assets.registry;
        let level = Level::default();
        let prepared = prepared.map(Ok).unwrap_or_else(|| {
            GameCpuResources::load(
                &sunrise_assets::resource_source::ResourceRoot::new(
                    root.clone(),
                    packs::cache_dir(),
                    pack.source.clone(),
                ),
                None,
                pack.render.model_library.as_deref().ok(),
                true,
            )
        })?;
        let world = crate::game::world_for_mods(res.facts().air, 0);
        let tutorial = tutorial::Tutorial::new(tutorial::Step::from_option(
            settings.file.int("tutorialStep"),
        ));
        let now = Instant::now();
        let mut particles = sunrise_client::particles::ParticleEngine::new(
            &pack.source,
            &gui.atlas.sprites,
            &gui.atlas.translucent,
        );
        particles.set_block_state_source(crate::game::particle_block_states);
        let resource_source = pack.source.clone();
        let (pack, gui) = if let Some(initial) = initial {
            let mut unpublished = pack.clone();
            unpublished.render = Arc::new(RenderAssets::unpublished());
            let context = &pack.context;
            unpublished.context = Arc::new(WorkerContext {
                assets: context.assets.clone(),
                render: unpublished.render.clone(),
                tables: context.tables.clone(),
                biomes: context.biomes.clone(),
                light: context.light.clone(),
                hidden_states: context.hidden_states.clone(),
                part_tables: context.part_tables.clone(),
            });
            unpublished.layer_names = Default::default();
            let metrics = UiResources::server(&pack.source)?;
            *initial = Some(initial_client_models::InitialModelGeneration { pack, gui });
            (unpublished, metrics)
        } else {
            (pack, gui)
        };
        // Until a world is opened the streamer is idle.
        let mut streaming = Streaming::new(
            Arc::new(TestTerrain::new(registry, res.biomes())?),
            pack.context.clone(),
            1,
        );
        streaming.paused = true;
        let standalone_geometry = native_block_entity_draw::StandaloneGeometry::new(&pack, &gui);
        let GameCpuResources {
            painting_variants,
            frame_model,
            glow_frame_model,
            map_frame_model,
            glow_map_frame_model,
            armor_stand_assets,
            waypoint_styles,
            boat_models,
            cart_models,
            crystal_model,
            cushion_model,
            dragon_model,
            arrow_model,
            bee_stinger_model,
            trident_model,
            entity_models,
            item_features,
            creative_screen,
        } = prepared;
        let client_prepared = Box::new(client_state::ClientCpuResources {
            frame_model,
            glow_frame_model,
            map_frame_model,
            glow_map_frame_model,
            waypoint_styles,
            boat_models,
            cart_models,
            crystal_model,
            cushion_model,
            dragon_model,
            arrow_model,
            bee_stinger_model,
            trident_model,
            entity_models,
        });
        Ok(Box::new(ClientConstructPrep {
            level,
            painting_variants,
            armor_stand_assets,
            item_features,
            creative_screen,
            client_prepared,
            world,
            tutorial,
            now,
            particles,
            resource_source,
            pack,
            gui,
            options,
            settings,
            audio,
            streaming,
            standalone_geometry,
        }))
    }

    // Evaluates all effectful literal initialisers in their original base order before tail assembly.
    #[inline(never)]
    fn prepare_effects(prep: &ClientConstructPrep) -> Box<ClientConstructEffectsBundle> {
        let slow_frame_ms = prep
            .options
            .spikes
            .unwrap_or(sunrise_client::client_perf::SLOW_FRAME_MS);
        let fog = sunrise_client::fog::FogState::default();
        let clock = FixedStep::default();
        let vanilla_mappings = sunrise_client::keymapping::VanillaMappings::default();
        let health_hud = hud_layout::HealthHud::default();
        let tick = sunrise_client::parallel::TickThread::mint();
        let music = {
            // The manager starts with the saved option (MusicManager's constructor).
            let mut music = sunrise_client::music::MusicManager::default();
            music.frequency = sunrise_client::music::MusicFrequency::parse(
                &prep.settings.file.name("musicFrequency"),
            );
            music
        };
        let ambience = sunrise_client::ambience::Ambience::default();
        let frame_clock = sunrise_client::window::SystemClock::default();
        let narration = sunrise_client::screen_narrator::ScreenNarrator::default();
        let skin_downloads = RefCell::new({
            let mut downloads = sunrise_client::skins::Downloader::new(
                std::env::current_dir()
                    .unwrap_or_default()
                    .join("assets/skins"),
            );
            downloads.set_resource_root(prep.resource_source.clone());
            downloads
        });
        let skin_profiles = RefCell::new(sunrise_client::skins::Resolver::new());
        let profile = profile_capture::Profile::new(slow_frame_ms);
        let tick_script = tick_script::TickScript::from_env();
        let social = social::Social::default();
        let registered_debug_entries = sunrise_client::debug_entries::registered();
        let debug_entries = sunrise_client::debug_entries::DebugEntries::load(
            &std::env::current_dir().unwrap_or_default(),
        );
        let ui = chat::UiState::new();
        let client = Box::new(client_state::ClientConstructEffects {
            fog,
            vanilla_mappings,
            health_hud,
            music,
            ambience,
            frame_clock,
            narration,
            tick_script,
            social,
        });
        Box::new(ClientConstructEffectsBundle {
            clock,
            tick,
            skin_downloads,
            skin_profiles,
            profile,
            registered_debug_entries,
            debug_entries,
            ui,
            client,
        })
    }

    #[inline(never)]
    fn finish_construct(
        res: &'a Arc<Resources>,
        prep: Box<ClientConstructPrep>,
        effects: Box<ClientConstructEffectsBundle>,
    ) -> Self {
        let (wp, client) = ConstructInit::<true>::wrap_client_prep_and_effects(prep, effects);
        ConstructInit::<true>::assemble_game(res, wp, client)
    }
}

/// `key.hotbar.1` to `key.hotbar.9` in a menu.
const HOTBAR_DIGITS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];
/// The other keys `InputConstants.Key.getNumericKeyValue` gives a digit.
const NUMERIC_KEYS: [KeyCode; 11] = [
    KeyCode::Digit0,
    KeyCode::Numpad0,
    KeyCode::Numpad1,
    KeyCode::Numpad2,
    KeyCode::Numpad3,
    KeyCode::Numpad4,
    KeyCode::Numpad5,
    KeyCode::Numpad6,
    KeyCode::Numpad7,
    KeyCode::Numpad8,
    KeyCode::Numpad9,
];

/// A title-shaped frame: black clear and panorama, only the HUD is drawn.
/// The window's `HWND`, for the boot screen's GDI paint (`sunrise_threads`):
/// the raw handle read is safe, the paint is not.
#[cfg(target_os = "windows")]
fn hwnd(window: &Window) -> Option<isize> {
    use winit::raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    match window.window_handle() {
        Ok(raw) => match raw.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
            _ => None,
        },
        Err(_) => None,
    }
}

pub use launch_api::{UsageError, exit_code};

pub(crate) fn run_with(
    options: Options,
    native: sunrise_mod_package::runtime::Runtime,
) -> Result<()> {
    let mut options = Options::read(options)?;
    options.mods.native = Arc::new(native);
    if options.execution_role == execution_role::ExecutionRole::DedicatedServer
        && !dedicated::preflight(&mut options)?
    {
        return Ok(());
    }
    native_hooks::install_block_contact_hooks();
    // Before any worker starts: on Linux they go behind the server thread
    // and the render thread.
    sunrise_threads::put_helpers_behind();
    if options.execution_role.is_client() {
        // The render thread, like the server thread, ahead of the workers on
        // macOS and Windows. While terrain loads, the workers
        // (sunrise_threads::jobs) and other programs outnumber the cores, and a frame that gave its core up (in a GPU submit) waited
        // 10 to 200 ms to get one back.
        sunrise_threads::favour_current();
    }
    // A dedicated server too: vanilla's "Server Shutdown Thread" (`DedicatedServer.initServer`'s
    // shutdown hook) halts the server and saves its players and levels on SIGTERM, SIGINT or SIGHUP.
    if options.execution_role.is_client()
        || options.execution_role == execution_role::ExecutionRole::DedicatedServer
    {
        sunrise_threads::catch_stop_signals();
    }
    let started = Instant::now();
    // A dedicated server times its spans always, so its slow ring holds the steps of a lag spike (microseconds a tick).
    if options.spikes.is_none()
        && options.execution_role == execution_role::ExecutionRole::DedicatedServer
    {
        options.spikes = Some(dedicated::SLOW_TICK_MS);
    }
    tick_profile::want_spans(tick_profile::SPANS_SPIKES, options.spikes.is_some());
    // A player who allows performance statistics also says what their slow frames spent their
    // time in: this thread, which draws the frames, times its steps (wall time only).
    if options.execution_role.is_client()
        && sunrise_telemetry::Consent::load(&sunrise_client::logging::game_home())
            .allows(sunrise_telemetry::Kind::Perf)
    {
        sunrise_client::spans::record_here(true);
    }
    let vanilla_root = options.resources.clone();
    // Saves from before 1.13 upgrade with vanilla's tables, prepared from the player's jar.
    sunrise_world::datafix::set_resources(
        &std::path::absolute(&vanilla_root).unwrap_or_else(|_| vanilla_root.clone()),
    );
    let preparation = match preflight_resources(
        &vanilla_root,
        sunrise_hooks::engine::get_the_game(),
        options.execution_role.is_dedicated_server(),
    ) {
        Ok(profile) => profile,
        Err(error) => {
            // Keep the introductory GUI for an installation with no manifest.
            // A present but unusable preparation needs the exact re-prepare error.
            if !options.offscreen
                && !synchronous_startup(&options)
                && !vanilla_root
                    .join(sunrise_core::prepared_profile::MANIFEST)
                    .is_file()
            {
                log::error!("Sunrise: {error}");
                let settings = Settings::load(&crate::game::options_path());
                return no_resources::show(&vanilla_root, &settings);
            }
            return Err(error.into());
        }
    };
    let mut early_settings = Settings::load(&crate::game::options_path());
    options
        .mods
        .capture_code_switches(&early_settings.mods.disabled);
    options
        .mods
        .select(&std::env::current_dir()?, &early_settings.mods.enabled);
    mods::validate_startup_packages(&options.mods)?;
    let jar_store = sunrise_client::logging::game_home().join("jars");
    let admission = options.mods.admit_startup_packages(&jar_store)?;
    api_values::install_packages(admission.native_packages(), admission.content_packages())?;
    mods::install_registration_data(
        &vanilla_root,
        mods::runtime_files::sources(&options.mods),
        mods::package_files::sources(&admission),
    )?;
    {
        crate::game::part_admission::admit_parts(mod_api::__private::mods())?;
        // The structure types those mods registered, handed to the engine before
        // any world opens, as its StructureRegistry reads them.
        if sunrise_world::worldgen::structure::mod_types::install_mod_structure_types(
            native_structure_types::installed_structure_types(),
        )
        .is_err()
        {
            log::warn!("Mod structure types already installed; keeping the first table");
        }
        // Their pool element types, before any world decodes its template pools.
        if sunrise_world::worldgen::structure::mod_elements::install_mod_pool_element_types(
            native_pool_elements::installed_pool_element_types(),
        )
        .is_err()
        {
            log::warn!("Mod pool element types already installed; keeping the first table");
        }
        // Their feature types, before any world decodes its configured features.
        if sunrise_world::worldgen::mod_features::install_mod_feature_types(
            native_feature_types::installed_feature_types(),
        )
        .is_err()
        {
            log::warn!("Mod feature types already installed; keeping the first table");
        }
        // The levels they registered, below the data packs of every world opened.
        if sunrise_world::worldgen::overrides::install_mod_dimensions(
            native_dimensions::installed_dimensions(),
        )
        .is_err()
        {
            log::warn!("Mod dimensions already installed; keeping the first table");
        }
        // Their chunk generators and biome sources, which those levels' and the packs' stems name.
        native_chunk_generator::install();
        native_biome_source::install();
        mods::register_components(&vanilla_root)?;
        sunrise_client::item_sync::install_consume_effect_codecs()?;
        mods::register_standalone_models(options.execution_role);
        mods::register_layer_definitions(options.execution_role, &vanilla_root)?;
        mods::register_debug_entries(options.execution_role);
        mods::register_gui_layers(options.execution_role);
        mods::register_client_tooltip_component_factories(options.execution_role);
        mods::register_map_decoration_types();
        mods::register_range_select_item_model_properties(options.execution_role);
        mods::register_select_item_model_properties(options.execution_role);
        mods::register_resource_sources(options.execution_role);
        mods::register_enchantment_damage_effects();
        mods::register_client_presentations(options.execution_role)?;
        mods::register_meshing(options.execution_role);
        mods::register_particles(options.execution_role)?;
        mods::freeze_declarations();
    }
    // Minecraft.<init>: read the last start's flag, then mark this one as not
    // finished until onGameLoadFinished. A dedicated server has no options.txt.
    if options.execution_role.is_client() {
        if unclean_start_recovery(&mut early_settings) {
            log::warn!(
                "Detected unexpected shutdown during last game startup: resetting fullscreen mode"
            );
        }
        early_settings.file.set(
            "startedCleanly",
            sunrise_client::options_file::V::Bool(false),
        );
        early_settings.save(&crate::game::options_path());
    }
    // Minecraft's constructor opens the narrator; a server has none.
    if options.execution_role.is_client() {
        narrator::start(&early_settings);
    }
    // Someone is playing: the anonymous ping behind the launcher's "N playing now", and the
    // official server in their list (servers.dat in the game folder, the working directory). Not
    // for a server, a benchmark, a run that quits by itself, a harness's fixture or a run that
    // sunrise-ctl drives: those start in a source checkout and wrote the list there.
    if !options.offscreen && !synchronous_startup(&options) {
        let version = sunrise_client::telemetry::version();
        let game_dir = sunrise_client::logging::player_dir();
        sunrise_hooks::engine::started(&sunrise_hooks::Started::new(&version, &game_dir));
    }
    // A game window's hooks (Discord's presence): never for a server.
    activity::client_started(&options);
    // Snapshots of derived tables live in the game folder, never beside the
    // shared resources.
    Behaviours::set_cache_dir(std::env::current_dir()?.join(".sunrise-cache"));
    if !synchronous_startup(&options) {
        return run_loading(options, early_settings, vanilla_root, None, started);
    }
    let roots = if options.execution_role.is_dedicated_server() {
        // Dedicated startup sees enabled mod assets but no client's resource packs.
        let base = mods::content_root(&vanilla_root, &options.mods)?;
        match preparation {
            sunrise_core::prepared_profile::Profile::Full => base,
            sunrise_core::prepared_profile::Profile::Server => base.with_server_profile(),
        }
        .into()
    } else {
        packs::startup_root(
            &vanilla_root,
            &early_settings.resource_packs,
            &[],
            &options.mods,
        )?
    };
    let mut loaded = load_resources_sync(
        roots,
        pack_finders::data_packs(&options.mods),
        StartupLoadContext {
            vanilla_root: &vanilla_root,
            launch_world: launch_world(&options),
            started,
        },
        &mut early_settings,
        mods::LoadRole::startup(options.execution_role),
        &Progress::new(),
        &options.mods,
    )?;
    let gui = loaded.gui.ok_or("missing menu bundle")?;
    options.resources = loaded.root.clone();
    let resources = loaded.resources;
    if options.execution_role.is_dedicated_server() {
        return start_dedicated_game(
            &resources,
            loaded.pack,
            gui,
            options,
            early_settings,
            started,
            None,
        );
    }
    let audio = if !options.execution_role.is_client() || options.headless {
        Audio::silent(&loaded.pack.source)
    } else {
        Audio::with_device(&loaded.pack.source, &early_settings.sound_device)
    };
    let game = start_client_game(
        &resources,
        loaded.pack,
        gui,
        options,
        loaded.pack_error,
        audio,
        vanilla_root,
        started,
        loaded.rest_loader,
        early_settings,
        None,
        std::mem::take(&mut loaded.client_applies),
        loaded.entity_models,
        None,
    )?;
    if let Some(mut game) = game {
        // Minecraft.onGameLoadFinished: a start that made it this far is clean.
        if game.options.is_client_start() && game.options.headless {
            game.settings.file.set(
                "startedCleanly",
                sunrise_client::options_file::V::Bool(true),
            );
            game.settings.save(&crate::game::options_path());
        }
        run_event_loop(game)?;
    }
    Ok(())
}

/// The right and left arms of a player model layer (`PlayerModel.rightArm`
/// and `leftArm`, with their sleeves), each keeping the layer.
fn player_arms(
    root: &std::path::Path,
    key: &str,
    document: Option<&sunrise_client::mobs::EntityModelDocument>,
) -> [Option<sunrise_client::mobs::Part>; 2] {
    let model = match document {
        Some(document) => document.model(key),
        None => sunrise_client::mobs::load_model(root, key),
    };
    ["right_arm", "left_arm"].map(|name| model.as_ref().and_then(|m| m.detached_child(name)))
}

/// The most `Player.sleepCounter` reaches: 100 asleep, then up to here over the
/// waking fade.
const MAX_SLEEP_TIMER: u32 = 109;

// Player.tick: cap the sleeping fade at 100, then fade out over 10 waking ticks.
fn tick_sleep_timer(timer: u32, sleeping: bool) -> u32 {
    if sleeping {
        (timer + 1).min(100)
    } else if (1..MAX_SLEEP_TIMER).contains(&timer) {
        timer + 1
    } else {
        0
    }
}

/// Where a ray entering `pos` first meets the block's outline boxes, if it
/// does: a hollow block (torch, fence, flower) lets the ray through.
fn outline_stop(
    origin: [f64; 3],
    dir: [f64; 3],
    reach: f64,
    pos: [i32; 3],
    face: [i32; 3],
    boxes: impl FnOnce() -> Vec<[f64; 6]>,
) -> Option<Hit> {
    let (t, face) = interaction_shapes::entry(origin, dir, &boxes(), reach, face)?;
    Some(Hit {
        pos,
        face,
        point: std::array::from_fn(|i| origin[i] + dir[i] * t),
        inside: false,
    })
}
/// `VoxelShape.clip` of a fluid source's shape for a ray that starts in the
/// source's own block `cell`: the face and point it hits, in `pick_fluid`'s
/// `Hit` terms. `FlowingFluid.getShape` is the whole block under the same
/// fluid and `getOwnHeight` (8/9) tall otherwise. A start inside the shape
/// (tested 1e-7 of the way along) hits at the start, on the face opposite
/// `Direction.getApproximateNearest(dir)`; from above it, a ray going down
/// enters through the top while still in this block.
fn clip_start_source(
    eye: [f64; 3],
    dir: [f64; 3],
    reach: f64,
    cell: [i32; 3],
    same_above: bool,
) -> Option<([i32; 3], [f64; 3])> {
    let height = if same_above {
        1.0
    } else {
        f64::from(8.0f32 / 9.0f32)
    };
    let probe_y = eye[1] + dir[1] * reach * 1.0e-7;
    if probe_y - f64::from(cell[1]) < height {
        // Direction.values() order; the first of equal dot products wins.
        let normals: [[i32; 3]; 6] = [
            [0, -1, 0],
            [0, 1, 0],
            [0, 0, -1],
            [0, 0, 1],
            [-1, 0, 0],
            [1, 0, 0],
        ];
        let mut nearest = normals[2];
        let mut best = f64::NEG_INFINITY;
        for n in normals {
            let dot =
                f64::from(n[0]) * dir[0] + f64::from(n[1]) * dir[1] + f64::from(n[2]) * dir[2];
            if dot > best {
                best = dot;
                nearest = n;
            }
        }
        return Some((nearest.map(|v| -v), eye));
    }
    if dir[1] >= 0.0 {
        return None;
    }
    let t = (f64::from(cell[1]) + height - eye[1]) / dir[1];
    if t > reach {
        return None;
    }
    let point = [
        eye[0] + dir[0] * t,
        eye[1] + dir[1] * t,
        eye[2] + dir[2] * t,
    ];
    (point[0].floor() as i32 == cell[0] && point[2].floor() as i32 == cell[2])
        .then_some(([0, 1, 0], point))
}

fn reset_world_progress(adv: &mut advancements::AdvState, stats: &mut stats::Stats) {
    *adv = Default::default();
    *stats = Default::default();
}

pub(crate) use closest_space::closest_space_nudge;
#[allow(unused_imports)]
#[cfg(test)]
use inventory_input::drop_whole_stack;
#[allow(unused_imports)]
use subtitles::{Subtitles, in_game_ui, subtitles};

#[allow(unused_imports)]
#[cfg(test)]
use hud_render::block_outline_drawn;
#[allow(unused_imports)]
use hud_render::{debug_crosshair_drawn, is_game_master_block};
#[cfg(test)]
mod test_neighbourhood {
    //! Shared fixture step for tests that tick entities on prepared data.
    use super::*;

    impl<S: crate::game::Side> Game<'_, S> {
        /// Loads the chunks within 2 columns of `(cx, cz)` that are not loaded yet.
        /// Entity ticking waits for every chunk in that 5x5 neighbourhood
        /// (`ChunkMap.prepareEntityTickingChunk`), so a fixture that ticks mobs,
        /// items or TNT in a chunk calls this after setting up its own chunks.
        pub(crate) fn load_ticking_neighbourhood(&mut self, cx: i32, cz: i32) {
            for z in cz - 2..=cz + 2 {
                for x in cx - 2..=cx + 2 {
                    if self.world.chunks.contains_key(&(x, z)) {
                        continue;
                    }
                    self.world.chunks.insert(
                        (x, z),
                        std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(x, z)),
                    );
                    // A chunk that is already registered keeps its ticks.
                    let _ = self
                        .block_ticks
                        .add_empty_chunk(sunrise_core::position::ChunkPos::new(x, z));
                }
            }
        }
    }
}

fn write_level_for_role(
    level: &mut Level,
    dir: &std::path::Path,
    role: execution_role::ExecutionRole,
    items: &crate::items::Items,
    keep_owner_file: bool,
) -> Result<()> {
    match (role.writes_singleplayer_player(), keep_owner_file) {
        (true, false) => level.write(dir, items),
        // A crash save keeps the owner's file as last saved, and a legacy `Data.Player`.
        (true, true) => level.write_keeping_player(dir),
        (false, _) => level.write_without_player(dir),
    }
}

/// A server's data load with no mods and no settings, for loaders under test.
#[cfg(test)]
fn test_server_load() -> sunrise_world::native_load_context::LoadContext {
    use sunrise_world::native_load_context::{DataLoadOrigin, LoadContext, LoadInputs};
    LoadContext::new(
        DataLoadOrigin::ServerDataLoad,
        Arc::new(LoadInputs::new(Default::default(), Default::default())),
        [],
    )
}

#[cfg(test)]
mod nested_orb_spawn_tests {}

#[cfg(test)]
pub(crate) fn in_own_process(name: &str, body: impl FnOnce()) {
    const OWN_PROCESS: &str = "SUNRISE_TEST_OWN_PROCESS";
    if std::env::var_os(OWN_PROCESS).is_some_and(|v| v == name) {
        return body();
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([name, "--exact", "--include-ignored", "--test-threads=1"])
        .env(OWN_PROCESS, name)
        .output()
        .unwrap();
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.status.success() && report.contains("test result: ok. 1 passed;"),
        "{name} failed in its own process:\n{report}"
    );
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    fn presence_status(&self) -> sunrise_client::social::PresenceStatus {
        use sunrise_client::social::PresenceStatus;
        if self.integrated.is_some() {
            // `getSingleplayerServer().getMultiplayerScope()`: LAN while the world is
            // opened to LAN (a pause-menu Open to LAN or an invite code).
            if self.lan.is_some() {
                PresenceStatus::PlayingHostedServer
            } else {
                PresenceStatus::PlayingOffline
            }
        } else if self.remote.is_some() {
            // `getCurrentServer().isRealm()`: the connection came from Realms.
            if self.realm_session {
                PresenceStatus::PlayingRealms
            } else {
                PresenceStatus::PlayingServer
            }
        } else {
            PresenceStatus::Online
        }
    }
}

#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::sign_chain_tests;

#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use client_side::sign_outline_tests;

#[cfg(test)]
#[path = "game/interface/tests/sign_use_tests.rs"]
mod sign_use_tests;

#[allow(unused_imports)]
pub(in crate::game) use {
    copper_golems::golem_preserved_equipment,
    native_client_particles::particle_block_states,
    native_datafix::upgrade_nbt,
    native_hooks::{
        another_mods, authoritative_world_for_mods, mod_data_root, native_config_dir,
        startup_math_profile, startup_setting, world_for_mods,
    },
    options::{game_dir, options_path},
    remote::{inventory_screen_slot, inventory_screen_slot_with_type, remote_station_kind},
    stacked_on_other::{
        stacked_listened, stacked_listened_for_side, stacked_listens_for, stacked_listens_for_side,
    },
};
