#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::Breather;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::assert_isolated_breathing_tick;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::breathing_guest_tick;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::guest;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{Arc, atomic::AtomicI32, mpsc};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    assets::Assets,
    lan::{Joined, PlayerSender, guest::SavedBody},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::random::RandomSource;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::configuration::ClientInformation;

#[path = "tests/fall_reset_server.rs"]
pub(in crate::game) mod fall_reset;

#[path = "tests/travel_server.rs"]
pub(in crate::game) mod travel;
