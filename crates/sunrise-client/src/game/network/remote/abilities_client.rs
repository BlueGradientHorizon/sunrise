#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::abilities::WALKING_SPEED;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::abilities::walking_speed;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::abilities::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::lan::abilities::PlayerAbilities;

#[cfg(test)]
#[path = "gamemode_split_tests_client.rs"]
pub(in crate::game) mod gamemode_split_tests;
