#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::recipe_properties::*;

#[cfg(test)]
#[path = "recipe_properties/tests_server.rs"]
pub(in crate::game) mod tests;
