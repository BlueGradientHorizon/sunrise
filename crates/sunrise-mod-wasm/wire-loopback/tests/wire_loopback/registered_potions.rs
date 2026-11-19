use super::*;
use mod_api::PotionInfo;
use mod_api::prelude::ItemsClientHost;

struct Client;
impl mod_api::Host for Client {}
mod_api::impl_host_areas!(Client);
// A boundary reply fixture: vanilla's first potions and a mod namespace, not the game's registry.
impl ItemsClientHost for Client {
    fn potions(&self) -> Vec<PotionInfo> {
        [
            "minecraft:water",
            "minecraft:long_swiftness",
            "wire_test:sticky",
        ]
        .into_iter()
        .map(|id| PotionInfo::new(id.into()))
        .collect()
    }
}
impl mod_api::ClientHost for Client {}
mod_api::impl_client_host_areas!(Client, except(items));

#[test]
fn registered_potions_keep_order_and_ids_across_the_host_call() {
    install_counter();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let mut client = Client;
    let expected = vec![
        PotionInfo::new("minecraft:water".into()),
        PotionInfo::new("minecraft:long_swiftness".into()),
        PotionInfo::new("wire_test:sticky".into()),
    ];
    for potion in &expected {
        let mut writer = Writer::new();
        writer.put(potion);
        assert_eq!(
            Reader::decode::<PotionInfo>(writer.bytes()).unwrap(),
            *potion
        );
    }
    let mut frame = boundary::Frame::default();
    frame.set_client_host_mut(&mut client);
    let mut cx = boundary::Cx::default();
    cx.frames.push(frame);
    let cx = std::cell::RefCell::new(cx);
    // Restores the previous transport even when an assertion below unwinds.
    let _transport = TransportScope(None);
    guest::set_transport(Some(Rc::new(move |id, bytes| {
        let mut out = Writer::new();
        // SAFETY: the client and its frame outlive this synchronous transport.
        unsafe { boundary::serve(id, &mut *cx.borrow_mut(), bytes, &mut out) }.unwrap();
        out.into_bytes()
    })));
    assert_eq!(guest::Guest.potions(), expected);
}
