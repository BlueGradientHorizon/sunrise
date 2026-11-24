#[allow(unused_imports)]
use crate::game::native_entity_refs::tests::with_client_game;
#[allow(unused_imports)]
use crate::game::remote::RemoteDimensionEntry;
#[allow(unused_imports)]
use sunrise_client::camera::CameraType;
#[allow(unused_imports)]
use sunrise_client::motion::PlayerMotion;
#[allow(unused_imports)]
use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
use sunrise_protocol::{
    connection::Pipe,
    packets::play::{self, Clientbound as C, SpawnInfo},
};

/// The local player and a villager drawn over camera entity, camera type and
/// sleeping, against what the real 26.3 `LevelExtractor` drew.
#[test]
#[ignore = "run by tools/test_camera_parity.py"]
fn local_player_and_camera_entity_draw_like_the_real_client() {
    run_local_player_and_camera_entity_draw_like_the_real_client();
}

#[allow(unused_imports)]
pub(in crate::game) use crate::game::client_side::remote_client::tests::camera_draw_tests::run_local_player_and_camera_entity_draw_like_the_real_client;
