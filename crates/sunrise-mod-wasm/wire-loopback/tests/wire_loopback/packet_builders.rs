use super::*;
use mod_api::prelude::*;
use mod_api::{DisconnectError, SendPacketError, ServerboundPlayPacket, Text, TextFormError};
use std::cell::RefCell;

#[derive(Default)]
struct Client(RefCell<Vec<ServerboundPlayPacket>>);
impl mod_api::Host for Client {}
mod_api::impl_host_areas!(Client);
impl mod_api::ClientHost for Client {}
mod_api::impl_client_host_areas!(Client, except(network));
impl NetworkClientHost for Client {
    fn open_chat(&mut self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn chat_line(&mut self, _: Text) -> Result<(), TextFormError> {
        Ok(())
    }
    fn send_chat(&mut self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn container_click(
        &self,
        request: &mod_api::ContainerClickRequest,
    ) -> Result<ServerboundPlayPacket, mod_api::PacketBuildError> {
        sunrise_client_lib::mod_packet_builders::encode_container_click(
            request,
            &container_builtin(),
            &sunrise_protocol::registry::Dynamic::default(),
            container_engine,
        )
    }
    fn send_packet(&mut self, packet: ServerboundPlayPacket) -> Result<(), SendPacketError> {
        self.0.borrow_mut().push(packet);
        Ok(())
    }
    fn disconnect(&mut self, _: Option<Text>) -> Result<(), DisconnectError> {
        Ok(())
    }
}

#[test]
fn portable_packets_cross_guest_send_packet_without_changing_bytes() {
    mod_api::__private::initialize().unwrap();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let mut client = Client::default();
    let mut frame = boundary::Frame::default();
    frame.set_client_host_mut(&mut client);
    let mut cx = boundary::Cx::default();
    cx.frames.push(frame);
    let cx = RefCell::new(cx);
    guest::set_transport(Some(Rc::new(move |id, bytes| {
        let mut out = Writer::new();
        // SAFETY: the client and frame remain live during this synchronous call.
        unsafe { boundary::serve(id, &mut *cx.borrow_mut(), bytes, &mut out) }.unwrap();
        out.into_bytes()
    })));
    type P = ServerboundPlayPacket;
    let packets = vec![
        P::use_item(1, 7, 90.0, -12.5).unwrap(),
        P::use_item_on(0, [1, 64, -2], 1, [0.5, 1.0, 0.25], true, false, 8).unwrap(),
        P::player_action(0, [1, 64, -2], 3, 7).unwrap(),
        P::interact(9, 1, [0.25, 1.0, -0.5], true).unwrap(),
        P::attack(9).unwrap(),
        P::pick_item_from_block([1, 64, -2], true).unwrap(),
        P::pick_item_from_entity(9, false).unwrap(),
        P::sign_update(
            [1, 64, -2],
            true,
            ["Hello".into(), "世界".into(), "🦀".into(), "".into()],
        )
        .unwrap(),
        P::move_player_pos([1.5, 64.0, -2.25], true, false).unwrap(),
        P::move_player_pos_rot([1.5, 64.0, -2.25], 90.0, -12.5, false, true).unwrap(),
        P::move_player_rot(90.0, -12.5, true, true).unwrap(),
        P::move_player_status_only(true, true).unwrap(),
    ];
    // Literal attack and movement bodies also check the value built on the guest side.
    assert_eq!(packets[4].body, [9]);
    assert_eq!(packets[11].body, [3]);
    for packet in &packets {
        assert_eq!(guest::Guest.send_packet(packet.clone()), Ok(()));
    }
    guest::set_transport(None);
    assert_eq!(*client.0.borrow(), packets);
}

fn container_builtin() -> sunrise_protocol::registry::Builtin {
    sunrise_protocol::registry::Builtin::parse(r#"{"minecraft:item":{"entries":{"minecraft:stick":{"protocol_id":4}}},"minecraft:data_component_type":{"entries":{"minecraft:damage":{"protocol_id":3},"minecraft:lore":{"protocol_id":11}}}}"#).unwrap()
}
fn container_engine(value: &mod_api::ItemStack) -> Option<sunrise_items::items::ItemStack> {
    let patch = value
        .components()
        .map(|p| sunrise_core::nbt::Tag::Compound(p.as_nbt().clone()));
    let (_, extra) = sunrise_items::item_data::decode_components(patch.as_ref());
    Some(sunrise_items::items::ItemStack {
        item: "minecraft:stick".into(),
        count: value.count(),
        damage: value.damage(),
        extra,
    })
}

#[test]
fn plain_container_stacks_and_129_refusal_cross_guest_boundary() {
    mod_api::testing::with_mods(vec![].into(), || {
        mod_api::__private::part_tests::install_parts_fixture(&[]).unwrap();
        mod_api::__private::resolve_items(["minecraft:stick"]).unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut client = Client::default();
        let mut frame = boundary::Frame::default();
        frame.set_client_host_mut(&mut client);
        let mut cx = boundary::Cx::default();
        cx.frames.push(frame);
        let cx = RefCell::new(cx);
        guest::set_transport(Some(Rc::new(move |id, bytes| {
            let mut out = Writer::new();
            // SAFETY: the client and frame remain live during this synchronous call.
            unsafe { boundary::serve(id, &mut *cx.borrow_mut(), bytes, &mut out) }.unwrap();
            out.into_bytes()
        })));
        let mut stack =
            mod_api::ItemStack::new(mod_api::__private::item_key("minecraft:stick").unwrap(), 3)
                .unwrap();
        stack
            .set_components(sunrise_core::nbt::Compound::from([
                ("minecraft:damage".into(), sunrise_core::nbt::Tag::Int(7)),
                (
                    "!minecraft:lore".into(),
                    sunrise_core::nbt::Tag::Compound(Default::default()),
                ),
            ]))
            .unwrap();
        let mut request = mod_api::ContainerClickRequest {
            container_id: 2,
            state_id: 5,
            slot: 36,
            changed_slots: vec![(36, Some(stack.clone())), (37, None)],
            carried: Some(stack),
            ..Default::default()
        };
        let got = guest::Guest.container_click(&request).unwrap();
        use sunrise_protocol::{
            item::{Component, Hashed, NoNames, Stack},
            packets::play::{ContainerClick, Serverbound},
        };
        let network = Stack {
            added: vec![(3, Component::Int(7))],
            removed: vec![11],
            ..Stack::new(4, 3)
        };
        let hashed = Hashed::create(&network, &NoNames);
        let expected = Serverbound::ContainerClick(ContainerClick {
            container: 2,
            state: 5,
            slot: 36,
            button: 0,
            input: 0,
            changed: vec![(36, Some(hashed.clone())), (37, None)],
            carried: Some(hashed),
        })
        .encode();
        let mut reader = sunrise_protocol::buf::Reader::new(&expected);
        reader.varint().unwrap();
        assert_eq!(got.id.to_string(), "minecraft:container_click");
        assert_eq!(got.body, reader.rest());
        assert_eq!(guest::Guest.send_packet(got.clone()), Ok(()));
        request.changed_slots = vec![(36, None); 129];
        assert_eq!(
            guest::Guest.container_click(&request),
            Err(mod_api::PacketBuildError::TooManyChangedSlots {
                packet: "container_click".into(),
                field: "changed_slots".into()
            })
        );
        guest::set_transport(None);
        assert_eq!(*client.0.borrow(), vec![got]);
    });
}
