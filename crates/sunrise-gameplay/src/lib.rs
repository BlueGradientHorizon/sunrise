//! Vanilla's logical side: the gameplay the server jar holds, common code included.
#![forbid(unsafe_code)]
pub mod allays;
pub mod block_entity_ticks;
pub use sunrise_gameplay_common::{
    brushing, cauldron_contents, class_tags, container_openers, crash_save, execution_role,
    game_modes, game_tests, hit, interaction_shapes, minecart_cells, projectile_spawned_validation,
    smithing_icons, structure_blocks, test_blocks, test_entities, test_environments,
    test_instances, world_clock, worn_boots,
};
pub mod cat_spawner;
pub mod custom_spawner;
pub mod dedicated;
pub mod dispense_rules;
pub mod dragon_contacts;
pub mod entity_uuids;
pub mod foxes;
pub mod hoppers;
pub mod initial_spawn;
pub mod lan;
pub mod leash_knot;
pub mod lectern;
pub mod mob_level;
pub mod mob_looting;
pub mod mount_rules;
pub mod particle_options;
pub mod teleport_transition;
pub mod tick_profile;
pub mod trident;
pub mod village_siege;
pub mod wandering_trader;
