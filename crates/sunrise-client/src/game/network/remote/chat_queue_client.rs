#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, QueuedChat, QueuedChatKind, Remote, Signature};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::chat_queue::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::time::{Duration, Instant};

impl Game<'_> {
    /// `ChatListener.messageDelay`: Chat Delay in milliseconds, as Java
    /// truncates it.
    pub(in crate::game) fn chat_delay_ms(&self) -> i64 {
        (self.settings.chat_delay * 1000.0) as i64
    }
    /// `ChatListener.queueSize`: the messages Chat Delay holds, for the chat's
    /// `chat.queue` row.
    pub(in crate::game) fn chat_queue_len(&self) -> usize {
        self.remote.as_ref().map_or(0, |r| r.chat_queue.len())
    }
}

impl Remote {
    /// `removeFromDelayedMessageQueue`: suppress presentation and the pending
    /// acknowledgement. Invalid signatures are not queue deletion keys.
    pub(in crate::game::client_side) fn remove_queued_chat(
        &mut self,
        signature: &Signature,
    ) -> bool {
        let queued = self.chat_queue.len();
        self.chat_queue.retain(|chat| {
            !matches!(
                chat.kind,
                QueuedChatKind::Player { .. } | QueuedChatKind::Filtered
            ) || chat.signature.as_ref() != Some(signature)
        });
        self.chat_queue.len() != queued
    }
}
