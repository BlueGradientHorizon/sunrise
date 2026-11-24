//! A client rider's own move runs inside `tick_local_player`: its look is this tick's and its
//! ground and collision flags are what the move left, after the seat is taken.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::boats::Boat;
#[allow(unused_imports)]
use crate::game::native_entity_refs::tests::with_client_game;
#[allow(unused_imports)]
use sunrise_client::motion::PlayerMotion;
#[allow(unused_imports)]
use winit::keyboard::KeyCode;
