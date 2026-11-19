//! `BlockStates::requires_correct_tool` from a wasm mod reaches the backend with the state
//! intact, and the answer comes back as the native read's.
use super::*;
use mod_api::__private::game::{BackendMutRefOf, BackendRefOf};
use mod_api::{BlockState, BlockStateRecord, HostBlockStates};

thread_local! {
    static ASKED: std::cell::RefCell<Vec<BlockState>> = const { std::cell::RefCell::new(Vec::new()) };
}

struct Server;
impl mod_api::Host for Server {
    fn backend(&self) -> Option<mod_api::BackendRef<'_>> {
        Some(mod_api::BackendRef::of(self))
    }
}
mod_api::impl_host_areas!(Server);
impl mod_api::ServerHost for Server {
    fn backend_mut(&mut self) -> Option<mod_api::BackendMutRef<'_>> {
        Some(mod_api::BackendMutRef::of(self))
    }
}
mod_api::impl_server_host_areas!(Server);
impl mod_api::Backend for Server {}
mod_api::impl_backend_areas!(Server, except(block_states));
impl mod_api::prelude::BlockStatesBackend for Server {
    /// State 0 needs a tool, state 1 doesn't, and state 2 is unknown.
    fn block_state_requires_correct_tool(&self, state: BlockState) -> Option<bool> {
        ASKED.with_borrow_mut(|asked| asked.push(state));
        let block = REGISTERED_BLOCK.get().unwrap();
        let index = (0..3).find(|&i| state == BlockStateRecord::new(block, i).into());
        match index {
            Some(0) => Some(true),
            Some(1) => Some(false),
            _ => None,
        }
    }
}
impl mod_api::BackendMut for Server {}
mod_api::impl_backend_mut_areas!(Server);

#[test]
fn a_wasm_mods_requires_tool_read_crosses_whole() {
    ASKED.with_borrow_mut(Vec::clear);
    // The fixture mod registers the block the states are of.
    install_counter();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let block = REGISTERED_BLOCK.get().unwrap();
    let states: Vec<BlockState> = (0..3)
        .map(|index| BlockStateRecord::new(block, index).into())
        .collect();
    let mut server = Server;
    {
        let host = &mut server;
        let mut frame = boundary::Frame::default();
        frame.set_server_host_mut(host);
        let mut cx = Box::new(boundary::Cx::default());
        cx.frames.push(frame);
        let pointer = &mut *cx as *mut boundary::Cx;
        // SAFETY: the context and borrowed host outlive synchronous proxy calls.
        let transport = unsafe { host_dispatch::transport(pointer) };
        let previous =
            LOOPBACK_TRANSPORT.with(|active| active.borrow_mut().replace(transport.clone()));
        let _restore = TransportScope(previous);
        guest::set_transport(Some(transport));
        let facts = guest::Guest.block_states();
        assert_eq!(facts.requires_correct_tool(states[0]), Some(true));
        assert_eq!(facts.requires_correct_tool(states[1]), Some(false));
        assert_eq!(facts.requires_correct_tool(states[2]), None);
        guest::set_transport(None);
    }
    // The server was handed exactly the states the guest asked about, in order.
    assert_eq!(ASKED.with_borrow_mut(std::mem::take), states);
}
