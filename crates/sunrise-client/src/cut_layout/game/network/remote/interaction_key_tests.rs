use super::*;
use mod_api::{ClientHost, InteractionKeyMappingTriggeredEvent};

pub(in crate::game) struct KeyControl {
    pub(in crate::game) cancel: bool,
    pub(in crate::game) swing: bool,
    pub(in crate::game) hand: Hand,
    pub(in crate::game) calls: Arc<Mutex<Vec<Hand>>>,
    pub(in crate::game) packets: Arc<Mutex<Vec<String>>>,
}
#[mod_api::__subscriptions]
impl Hooks for KeyControl {
    fn client_packet_send(
        &self,
        _: &dyn ClientHost,
        e: &mut mod_api::ClientPacketSendEvent<'_>,
    ) -> Flow {
        self.packets.lock().unwrap().push(e.packet.id.to_string());
        Flow::Continue
    }
    fn interaction_key_mapping_triggered(
        &self,
        _: &mut dyn ClientHost,
        e: &mut InteractionKeyMappingTriggeredEvent,
    ) -> Flow {
        self.calls.lock().unwrap().push(e.hand);
        if e.hand == self.hand {
            e.swing_hand = self.swing;
            if self.cancel {
                return Flow::Cancel;
            }
        }
        Flow::Continue
    }
}

pub(in crate::game) struct FirstSilent(pub(in crate::game) std::sync::atomic::AtomicBool);
#[mod_api::__subscriptions]
impl Hooks for FirstSilent {
    fn interaction_key_mapping_triggered(
        &self,
        _: &mut dyn ClientHost,
        e: &mut InteractionKeyMappingTriggeredEvent,
    ) -> Flow {
        if !self.0.swap(true, std::sync::atomic::Ordering::Relaxed) {
            e.swing_hand = false;
        }
        Flow::Continue
    }
}
