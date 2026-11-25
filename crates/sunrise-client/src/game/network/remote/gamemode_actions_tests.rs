//! Packet and prediction regressions for vanilla 26.3 game mode actions.
#[allow(unused_imports)]
use crate::game::Game;
#[allow(unused_imports)]
use crate::game::native_entity_refs::tests::with_client_game;
#[allow(unused_imports)]
use crate::game::native_hotbar_slot::hotbar_slot_tests::logged_in;
#[allow(unused_imports)]
use mod_api::InteractionAttempt;
#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use std::time::Duration;
#[allow(unused_imports)]
use sunrise_client::{chunk::Chunk, first_person::Hand, items::ItemStack};
#[allow(unused_imports)]
use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
use sunrise_protocol::connection::ConnReader;
#[allow(unused_imports)]
use sunrise_protocol::packets::{
    configuration::Ping,
    play::{self, Clientbound as C, Serverbound as S},
};

pub(in crate::game) fn add_entity(
    game: &mut crate::game::ClientGame<'_>,
    id: i32,
    kind: &str,
    position: [f64; 3],
) {
    let kind = game
        .remote
        .as_ref()
        .unwrap()
        .builtin
        .get("minecraft:entity_type")
        .unwrap()
        .id(kind)
        .unwrap();
    game.handle_packet(C::AddEntity(play::AddEntity {
        id,
        uuid: id as u128,
        kind,
        position,
        velocity: [0.0; 3],
        pitch: 0,
        yaw: 0,
        head_yaw: 0,
        data: 0,
    }))
    .unwrap();
}

pub(in crate::game) fn outgoing(
    game: &crate::game::ClientGame<'_>,
    reader: &mut ConnReader,
) -> Vec<S> {
    // Fence the click's outgoing packets without using a read timeout as evidence of no Attack.
    game.send(S::Pong(Ping { id: i32::MIN }));
    reader
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut packets = Vec::new();
    loop {
        let (id, body) = reader.recv().unwrap();
        let packet = S::decode(id, &body).unwrap();
        if matches!(&packet, S::Pong(ping) if ping.id == i32::MIN) {
            return packets;
        }
        packets.push(packet);
    }
}

pub(in crate::game) fn ranged_stick(min: f32, max: f32, margin: f32) -> ItemStack {
    let mut stack = ItemStack::new("minecraft:stick", 1);
    stack.data_mut().set_other(
        "minecraft:attack_range",
        Some(Tag::Compound(
            [
                ("min_reach".into(), Tag::Float(min.to_bits())),
                ("max_reach".into(), Tag::Float(max.to_bits())),
                ("min_creative_reach".into(), Tag::Float(min.to_bits())),
                ("max_creative_reach".into(), Tag::Float(max.to_bits())),
                ("hitbox_margin".into(), Tag::Float(margin.to_bits())),
            ]
            .into_iter()
            .collect(),
        )),
    );
    stack
}
