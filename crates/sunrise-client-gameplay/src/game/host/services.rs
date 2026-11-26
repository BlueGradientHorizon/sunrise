#[cfg(feature = "client")]
use crate::game::ClientGame;
#[cfg(feature = "client")]
use mod_api::*;
#[cfg(feature = "client")]
use std::sync::Arc;

#[cfg(feature = "client")]
impl ClientSession for ClientGame<'_> {
    fn client_server(&self) -> Option<ClientServer<'_>> {
        self.native_client_server()
    }
    fn client_language(&self) -> Option<Arc<str>> {
        self.native_client_language()
    }
    fn client_latency(&self) -> Option<u32> {
        self.native_client_latency()
    }
    fn client_tab_list(&self) -> Option<ClientTabList> {
        self.native_client_tab_list()
    }
    fn client_frame_stats(&self) -> Option<ClientFrameStats> {
        self.native_client_frame_stats()
    }
}
