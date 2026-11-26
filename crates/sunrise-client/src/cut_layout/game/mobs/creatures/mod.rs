
#[cfg(test)]
pub(in crate::game) mod borrowed_panic_tests;
#[cfg(test)]
pub(in crate::game) mod retained_limits_tests;
#[cfg(test)]
#[path = "../../creatures/spawn_egg_mob_tests.rs"]
pub(in crate::game) mod spawn_egg_mob_tests;
#[cfg(test)]
#[path = "../../creatures/spectator_view_tests.rs"]
pub(in crate::game) mod spectator_view_tests;

#[cfg(test)]
pub(in crate::game) mod control_mobs_sitting_tests;

#[cfg(test)]
pub(in crate::game) mod living_visibility_tests;

#[cfg(test)]
pub(in crate::game) mod enchanted_entity_loot_tests;

#[cfg(test)]
pub(in crate::game) mod enderman_anger_tests;

#[cfg(test)]
pub(in crate::game) mod mob_subscription_tests;

#[cfg(test)]
pub(in crate::game) mod breeding_tests;

#[cfg(test)]
pub(in crate::game) mod projectile_batch_tests;

#[cfg(test)]
pub(in crate::game) mod breeze_random_tests;

#[cfg(test)]
pub(in crate::game) mod armor_destroy_tests;

#[cfg(test)]
pub(in crate::game) mod conversion_tag_tests;

#[cfg(test)]
pub(in crate::game) mod mob_pearl_hook_tests;

#[cfg(test)]
pub(in crate::game) mod breeding_orb_tests;

#[cfg(test)]
pub(in crate::game) mod enchanted_block_loot_breaker_tests;

#[cfg(test)]
pub(in crate::game) mod chorus_projectile_drop_tests;

#[cfg(test)]
pub(in crate::game) mod loaded_arrow_shot_tests;

#[cfg(test)]
pub(in crate::game) mod loaded_mob_arrow_permission_tests;

#[cfg(test)]
pub(in crate::game) mod h169_glide_travel_tests;
#[cfg(test)]
pub(in crate::game) mod pending_birth_matrix_tests;
#[cfg(test)]
pub(in crate::game) mod pending_birth_tests;

#[cfg(test)]
#[path = "../tests/revenge_home_tests.rs"]
pub(in crate::game) mod revenge_home_tests;

#[cfg(test)]
pub(in crate::game) mod death_loot_context_tests;

#[cfg(test)]
pub(in crate::game) mod breeding_heart_tests;

#[cfg(test)]
#[path = "frame_projectile_tests.rs"]
pub(in crate::game) mod frame_projectile_tests;

#[cfg(test)]
#[path = "projectile_targets_tests.rs"]
pub(in crate::game) mod projectile_targets_tests;

#[cfg(test)]
#[path = "projectiles_120_124_tests.rs"]
pub(in crate::game) mod projectiles_120_124_tests;
pub(in crate::game) mod pufferfish_player;

pub(in crate::game) mod death_weapon;

pub(in crate::game) mod borrowed_tick;
