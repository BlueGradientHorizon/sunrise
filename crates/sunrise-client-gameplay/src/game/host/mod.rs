
#[cfg(feature = "client")]
use super::ClientGame;

#[cfg(feature = "client")]
impl Host for ClientGame<'_> {
    fn backend(&self) -> Option<BackendRef<'_>> {
        Some(BackendRef::of(self))
    }
}

#[cfg(feature = "client")]
impl ClientHost for ClientGame<'_> {
    fn client_backend_mut(&mut self) -> Option<ClientBackendMutRef<'_>> {
        Some(ClientBackendMutRef::of(self))
    }
}
#[cfg(feature = "client")]
impl Backend for ClientGame<'_> {}
#[cfg(feature = "client")]
impl ClientBackendMut for ClientGame<'_> {}
include!(concat!(env!("OUT_DIR"), "/game_host_generated.rs"));

pub(crate) mod services;

pub(in crate::game) mod phase_contexts;

pub(crate) mod draw;
