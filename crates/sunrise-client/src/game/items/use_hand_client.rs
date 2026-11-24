#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::use_hand::*;

#[cfg(test)]
#[path = "tests/scaffolding_inside_tests_client.rs"]
pub(in crate::game) mod scaffolding_inside_tests;

#[cfg(test)]
#[path = "tests/bow_enchantment_tests_client.rs"]
pub(in crate::game) mod bow_enchantment_tests;
