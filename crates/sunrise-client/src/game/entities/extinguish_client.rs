#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::extinguish::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::motion::MotionEvent;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::position::BlockPos;

#[cfg(test)]
#[path = "extinguish/extinguish_tests_client.rs"]
pub(in crate::game) mod extinguish_tests;
