#[allow(unused_imports)]
use crate::game::{Hit, native_entity_refs::tests::with_client_game};
#[allow(unused_imports)]
use mod_api::{ClientHost, ClientMiningPredictEvent, ClientPacketSendEvent, Flow, Hooks, ModEntry};
#[allow(unused_imports)]
use mod_platform::testing::with_mods;
#[allow(unused_imports)]
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[allow(unused_imports)]
use sunrise_client::{chunk::Chunk, motion::PlayerMotion};
#[allow(unused_imports)]
use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
use sunrise_protocol::connection::Pipe;

pub(in crate::game) struct CountActions {
    pub(in crate::game) packets: Arc<AtomicUsize>,
    pub(in crate::game) vetoes: Arc<AtomicUsize>,
}
#[mod_api::__subscriptions]
impl Hooks for CountActions {
    fn client_mining_predict(
        &self,
        _: &dyn ClientHost,
        event: &mut ClientMiningPredictEvent,
    ) -> Flow {
        assert!(!event.prediction.starting);
        assert_eq!(event.prediction.pos, [0, 64, 0]);
        assert!(event.prediction.progress > 0.5);
        self.vetoes.fetch_add(1, Ordering::Relaxed);
        Flow::Cancel
    }

    fn client_packet_send(
        &self,
        _: &dyn ClientHost,
        event: &mut ClientPacketSendEvent<'_>,
    ) -> Flow {
        if event.packet.id.path() == "player_action" {
            self.packets.fetch_add(1, Ordering::Relaxed);
        }
        Flow::Continue
    }
}
