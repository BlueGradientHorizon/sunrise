#[allow(unused_imports)]
use crate::game::{in_own_process, native_entity_refs::tests::with_client_game};
#[allow(unused_imports)]
use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
use sunrise_protocol::{
    connection::Pipe,
    packets::play::{BossAction, BossEvent, Clientbound},
};
