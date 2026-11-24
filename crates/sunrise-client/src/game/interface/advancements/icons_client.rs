#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, ItemStack};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::icons::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::item::Stack;

#[cfg(test)]
#[path = "icons/tests_client.rs"]
pub(in crate::game) mod tests;
