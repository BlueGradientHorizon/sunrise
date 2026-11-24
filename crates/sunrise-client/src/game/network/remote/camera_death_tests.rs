#[allow(unused_imports)]
use crate::game::native_entity_refs::tests::with_client_game;
#[allow(unused_imports)]
use crate::game::remote::RemoteDimensionEntry;
#[allow(unused_imports)]
use sunrise_client::{level_loading::Reason, motion::PlayerMotion};
#[allow(unused_imports)]
use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
use sunrise_protocol::{
    connection::Pipe,
    packets::play::{self, Clientbound as C, SpawnInfo},
};

/// Generic living camera only (oracle drives a deathTime stand-in, not a real creeper); the camera player's own fov modifier is open.
#[test]
#[ignore = "run by tools/test_camera_parity.py"]
fn a_spectated_dying_living_entity_squeezes_the_fov_26_3() {
    run_a_spectated_dying_living_entity_squeezes_the_fov_26_3();
}

#[allow(unused_imports)]
pub(in crate::game) use crate::game::client_side::remote_client::tests::camera_death_tests::run_a_spectated_dying_living_entity_squeezes_the_fov_26_3;
