#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::collision_source::Mover;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::item_entities::GivenBack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::item_entities::bob_offset;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::item_entities::*;

#[cfg(test)]
#[path = "tests/thrown_items_tests_client.rs"]
pub(in crate::game) mod thrown_items_tests;
