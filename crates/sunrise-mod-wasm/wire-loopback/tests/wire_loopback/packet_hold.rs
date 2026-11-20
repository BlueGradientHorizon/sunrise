//! Hold identifiers, counts and named refusals through the real generated boundary.
use super::*;
use mod_api::prelude::*;
use mod_api::{HoldError, HoldId, SendPacketError};
use std::cell::{Cell, RefCell};
#[derive(Default)]
struct Client {
    result: RefCell<Option<Result<usize, HoldError>>>,
    seen: RefCell<Vec<HoldId>>,
    world: Cell<bool>,
    active: Cell<bool>,
}
impl mod_api::Host for Client {}
mod_api::impl_host_areas!(Client);
impl mod_api::ClientHost for Client {}
mod_api::impl_client_host_areas!(Client, except(network));
impl NetworkClientHost for Client {
    fn hold_packets(&mut self) -> Result<HoldId, HoldError> {
        if self.active.get() {
            return Err(HoldError::Limit {
                packets: 256,
                bytes: 1_048_576,
            });
        }
        if self.world.get() {
            self.active.set(true);
            Ok(HoldId::new(4178))
        } else {
            Err(HoldError::NoWorld)
        }
    }
    fn release_held(&mut self, hold: HoldId) -> Result<usize, HoldError> {
        self.active.set(false);
        self.seen.borrow_mut().push(hold);
        self.result
            .borrow_mut()
            .take()
            .unwrap_or(Err(HoldError::Unknown))
    }
    fn drop_held(&mut self, hold: HoldId) -> Result<(), HoldError> {
        self.release_held(hold).map(|_| ())
    }
}
#[test]
fn hold_id_count_limit_and_partial_send_error_cross_native_and_wasm_boundary() {
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
        // SAFETY: the client and frame live through these synchronous guest calls.
        unsafe { boundary::serve(id, &mut *cx.borrow_mut(), bytes, &mut out) }.unwrap();
        out.into_bytes()
    })));
    let _transport = TransportScope(None);
    assert_eq!(guest::Guest.hold_packets(), Err(HoldError::NoWorld));
    client.world.set(true);
    let id = guest::Guest.hold_packets().unwrap();
    assert_eq!(id, HoldId::new(4178));
    assert_eq!(
        guest::Guest.hold_packets(),
        Err(HoldError::Limit {
            packets: 256,
            bytes: 1_048_576
        })
    );
    *client.result.borrow_mut() = Some(Ok(3));
    assert_eq!(guest::Guest.release_held(id), Ok(3));
    *client.result.borrow_mut() = Some(Err(HoldError::Limit {
        packets: 256,
        bytes: 1_048_576,
    }));
    assert_eq!(
        guest::Guest.release_held(id),
        Err(HoldError::Limit {
            packets: 256,
            bytes: 1_048_576
        })
    );
    *client.result.borrow_mut() = Some(Err(HoldError::Limit {
        packets: 256,
        bytes: 1_048_576,
    }));
    assert_eq!(
        guest::Guest.drop_held(id),
        Err(HoldError::Limit {
            packets: 256,
            bytes: 1_048_576
        })
    );
    *client.result.borrow_mut() = Some(Err(HoldError::Send {
        sent: 1,
        error: SendPacketError::Backpressure,
    }));
    assert_eq!(
        guest::Guest.release_held(id),
        Err(HoldError::Send {
            sent: 1,
            error: SendPacketError::Backpressure
        })
    );
    *client.result.borrow_mut() = Some(Ok(0));
    assert_eq!(guest::Guest.drop_held(id), Ok(()));
    assert_eq!(guest::Guest.release_held(id), Err(HoldError::Unknown));
    assert_eq!(*client.seen.borrow(), [HoldId::new(4178); 6]);
}
