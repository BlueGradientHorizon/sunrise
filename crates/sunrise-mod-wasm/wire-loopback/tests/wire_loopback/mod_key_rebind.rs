//! `ClientInputClientHost::set_mod_key` and `reset_mod_key` from a wasm mod reach the
//! client with the mod, the mapping, the key and the modifiers intact, and each named
//! refusal comes back as the native call returns it.
use super::*;
use mod_api::prelude::*;
use mod_api::{ClientModifiers, ModKeyRefusal};
use std::cell::RefCell;

/// What reached the client: the mod, the mapping, and the key and modifiers of a rebind
/// (none for a reset).
type Call = (String, String, Option<(String, u32)>);

/// A client that answers as the game would for these mappings, and records what reached it.
#[derive(Default)]
struct Client {
    calls: RefCell<Vec<Call>>,
}
impl mod_api::Host for Client {}
mod_api::impl_host_areas!(Client);
impl mod_api::ClientHost for Client {}
mod_api::impl_client_host_areas!(Client, except(client_input));
impl ClientInputClientHost for Client {
    fn set_mod_key(
        &mut self,
        mod_id: &str,
        name: &str,
        key: &str,
        modifiers: ClientModifiers,
    ) -> Result<(), ModKeyRefusal> {
        let call = (
            mod_id.to_owned(),
            name.to_owned(),
            Some((key.to_owned(), modifiers.bits)),
        );
        self.calls.borrow_mut().push(call);
        match name {
            "missing" => Err(ModKeyRefusal::NotRegistered),
            _ if mod_id != "keys" => Err(ModKeyRefusal::NotOwner),
            _ if key == "key.keyboard.nope" => Err(ModKeyRefusal::UnknownKey),
            _ if modifiers.bits > 7 => Err(ModKeyRefusal::UnknownModifiers),
            _ => Ok(()),
        }
    }
    fn reset_mod_key(&mut self, mod_id: &str, name: &str) -> Result<(), ModKeyRefusal> {
        self.calls
            .borrow_mut()
            .push((mod_id.to_owned(), name.to_owned(), None));
        match name {
            "missing" => Err(ModKeyRefusal::NotRegistered),
            _ => Ok(()),
        }
    }
}

#[test]
fn a_wasm_mods_rebind_reaches_the_client_and_gets_each_refusal_by_name() {
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
        // SAFETY: client and its frame remain live for this synchronous transport.
        unsafe { boundary::serve(id, &mut *cx.borrow_mut(), bytes, &mut out) }.unwrap();
        out.into_bytes()
    })));
    let sets = [
        ("keys", "zoom", "key.keyboard.x", 2, Ok(())),
        (
            "keys",
            "missing",
            "key.keyboard.x",
            0,
            Err(ModKeyRefusal::NotRegistered),
        ),
        (
            "other",
            "zoom",
            "key.keyboard.x",
            0,
            Err(ModKeyRefusal::NotOwner),
        ),
        (
            "keys",
            "zoom",
            "key.keyboard.nope",
            0,
            Err(ModKeyRefusal::UnknownKey),
        ),
        (
            "keys",
            "zoom",
            "key.mouse.left",
            8,
            Err(ModKeyRefusal::UnknownModifiers),
        ),
    ];
    for (mod_id, name, key, bits, answer) in sets {
        assert_eq!(
            guest::Guest.set_mod_key(mod_id, name, key, ClientModifiers::new(bits)),
            answer
        );
    }
    assert_eq!(guest::Guest.reset_mod_key("keys", "zoom"), Ok(()));
    assert_eq!(
        guest::Guest.reset_mod_key("keys", "missing"),
        Err(ModKeyRefusal::NotRegistered)
    );
    guest::set_transport(None);
    let mut reached: Vec<Call> = sets
        .iter()
        .map(|(m, n, k, b, _)| (m.to_string(), n.to_string(), Some((k.to_string(), *b))))
        .collect();
    reached.push(("keys".into(), "zoom".into(), None));
    reached.push(("keys".into(), "missing".into(), None));
    assert_eq!(*client.calls.borrow(), reached);
}

#[test]
fn every_mod_key_refusal_round_trips_on_the_wire() {
    for refusal in [
        ModKeyRefusal::NotRegistered,
        ModKeyRefusal::NotOwner,
        ModKeyRefusal::UnknownKey,
        ModKeyRefusal::UnknownModifiers,
        ModKeyRefusal::Unavailable,
    ] {
        let mut w = Writer::new();
        w.put(&Err::<(), _>(refusal));
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        let back: Result<(), ModKeyRefusal> = r.take().unwrap();
        assert_eq!(back, Err(refusal));
    }
}
