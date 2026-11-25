#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::*;

#[cfg(test)]
#[path = "tests/pickup_post_tests_client.rs"]
pub(in crate::game) mod pickup_post_tests;
