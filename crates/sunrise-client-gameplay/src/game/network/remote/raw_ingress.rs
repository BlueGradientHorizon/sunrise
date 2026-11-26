//! Receiving-provider credit follows ordered events and decoder placeholders.
use super::opening::RemoteOpening;
#[cfg(test)]
use super::{C, Game, Received, play};
#[cfg(test)]
use std::time::{Duration, Instant};
use sunrise_client_lib::net::Event;

pub(in crate::game) fn local_transfer_notice() -> Event {
    Event::Disconnected {
        reason: sunrise_game_data::lang::get("disconnect.transfer"),
        key: Some("disconnect.transfer".into()),
        from_server: true,
    }
}

pub(in crate::game) fn packet_error_notice() -> Event {
    Event::Disconnected {
        reason: sunrise_game_data::lang::get("disconnect.packetError"),
        key: Some("disconnect.packetError".into()),
        from_server: false,
    }
}

#[derive(Default)]
pub(in crate::game) struct ListenerTurn {
    pub(in crate::game) opening: Option<RemoteOpening>,
    pub(in crate::game) end: Option<Event>,
    pub(in crate::game) packet_error: bool,
}

#[cfg(test)]
impl crate::game::ClientGame<'_> {
    #[cfg(test)]
    pub(in crate::game) fn wait_for_test_remote_event(&mut self, until: impl Fn(&Event) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let remote = self.client.remote.as_mut().unwrap();
        let mut backlog = std::mem::take(&mut remote.backlog);
        loop {
            remote.receive_credited_backlog(&mut backlog);
            if backlog
                .iter()
                .any(|item| matches!(item, Received::Event(event, _) if until(event)))
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "expected event did not reach main"
            );
            std::thread::yield_now();
        }
        remote.backlog = backlog;
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
impl crate::game::ClientGame<'_> {
    #[cfg(test)]
    pub(in crate::game) fn acknowledge_test_configuration(&mut self) {
        self.wait_for_test_remote_event(|event| matches!(event, Event::FinishConfiguration(_)));
        self.poll_remote_events(true).unwrap();
    }
}

/// Protocol error raised while handling a play packet (`ClientCommonPacketListenerImpl.onPacketError`).
#[derive(Debug)]
pub(in crate::game) struct PacketError(pub(in crate::game) &'static str);

impl std::fmt::Display for PacketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for PacketError {}
