#[allow(unused_imports)]
use crate::game::remote::RemoteDimensionEntry;
#[allow(unused_imports)]
use crate::game::{
    Game, native_entity_refs::tests::with_client_game, native_lifecycle::NativeLifecycle,
};
#[allow(unused_imports)]
use mod_api::ClientHost;
#[allow(unused_imports)]
use sunrise_client::{level_loading::Reason, motion::PlayerMotion};
#[allow(unused_imports)]
use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
use sunrise_protocol::{
    connection::Pipe,
    packets::play::{self, Clientbound as C, SpawnInfo},
};

#[allow(unused_imports)]
pub(in crate::game) use crate::game::client_side::remote_client::client_camera_tests::run_a_named_mob_the_server_camera_is_attached_to_shows_no_name_tag;
