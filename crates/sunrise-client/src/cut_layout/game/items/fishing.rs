

#[cfg(test)]
#[path = "fishing_tests/block_hit.rs"]
pub(in crate::game) mod incoming_block_hit_tests;

#[cfg(test)]
#[path = "fishing_tests/fishing_air_tests.rs"]
pub(in crate::game) mod fishing_air_tests;

#[cfg(test)]
#[path = "fishing_tests/fishing_catch_tests.rs"]
pub(in crate::game) mod fishing_catch_tests;

#[cfg(test)]
#[path = "fishing_tests/fishing_medium_tests.rs"]
pub(in crate::game) mod fishing_medium_tests;

#[cfg(test)]
#[path = "fishing_tests/fishing_targets_tests.rs"]
pub(in crate::game) mod fishing_targets_tests;
#[path = "fishing/impact.rs"]
pub(crate) mod impact;
