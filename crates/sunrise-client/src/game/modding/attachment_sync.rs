//! Attachment sync (docs/modding/src/how-to/native-mods.md, "Server attachments"): the server
//! sends the synced values of the holders a level pass lent to mods, and whole
//! snapshots to the players who started seeing a holder; a client keeps them.
//!
//! The channel is an engine declaration in the negotiated payload table
//! (`raw_payloads::engine`): a message goes only to a player whose connection offered it
//! and who sees the holder. It is prepared against the current epoch and published at
//! the root commit, in order with the player's other packets.
use super::Game;
use mod_api::{PayloadBytes, PayloadId, PayloadSendError};
use sunrise_mod_runtime::attachments::Holder;
use sunrise_protocol::attachment_sync::{self, Message};
use sunrise_sim::lan::{conn::PlayerSender, hub::AttachmentSnapshot};
use sunrise_sim::raw_payloads::Endpoint;

impl Game<'_, crate::game::server_side::ServerState> {
    /// The messages that give a player `holder`'s synced attachments whole: its
    /// own, and for a chunk those of its block entities. A player holder with
    /// none still gives one, to drop what the client holds.
    fn attachment_snapshot(&mut self, holder: Holder) -> Vec<Message> {
        let changes = match holder {
            // Read in place: a chunk lent to a mod is copied and saved.
            Holder::Chunk([x, z]) => self
                .world
                .chunks
                .get(&(x, z))
                .and_then(|chunk| chunk.saved.as_ref())
                .map(|extras| extras.attachments.synced()),
            // Read in place: lending the map would give the entity one.
            Holder::Entity(id) => self.entity_attachments.get(&id).map(|map| map.synced()),
            _ => self
                .attachment_map_quiet(holder, mod_api::AttachmentAccess::Existing)
                .ok()
                .map(|map| map.synced()),
        }
        .unwrap_or_default();
        let mut holders = vec![(holder, changes)];
        if let Holder::Chunk([x, z]) = holder {
            holders.extend(
                self.world
                    .block_entities
                    .chunk_attachments(x, z)
                    .into_iter()
                    .map(|((x, y, z), map)| (Holder::BlockEntity([x, y, z]), map.synced())),
            );
        }
        holders
            .into_iter()
            .filter(|(holder, changes)| !changes.is_empty() || matches!(holder, Holder::Player(_)))
            .map(|(holder, changes)| Message {
                holder,
                replace: true,
                changes,
            })
            .collect()
    }

    /// The holders players started seeing are sent whole to them, in the pass
    /// of the level they are in (another's stay queued), then the block
    /// entities a piston landed whole to the players who see them, then the
    /// synced changes of the holders lent to mods since the last pass to the
    /// players who see them.
    pub(in crate::game) fn flush_attachment_sync(&mut self) {
        let landings = std::mem::take(&mut self.world.attachment_landings);
        let Some(host) = self.side.lan.as_mut() else {
            return;
        };
        let snapshots = std::mem::take(&mut host.hub.attachment_snapshots);
        // Colliders: the changes first, to those who already track the entity; the players who
        // just started to are sent the whole collider below instead.
        let fresh = Self::fresh_collider_pairs(
            &snapshots
                .iter()
                .map(|s| (s.player, s.holder))
                .collect::<Vec<_>>(),
        );
        self.flush_collider_sync(&fresh);
        for snapshot in snapshots {
            let AttachmentSnapshot { player, holder } = snapshot;
            let collider_entity = match holder {
                Holder::Entity(id) => Some(id),
                _ => None,
            };
            let Some(host) = self.side.lan.as_mut() else {
                return;
            };
            let hub = &mut host.hub;
            let Some(guest) = hub.online_players().find(|g| g.uuid == player) else {
                continue;
            };
            // Each channel is its own: a peer with only the collider channel still gets the
            // collider.
            let reads = guest.negotiated_attachment_sync();
            let reads_collider = collider_entity.is_some() && guest.negotiated_collider_sync();
            if !reads && !reads_collider {
                continue;
            }
            if guest.dimension != hub.config.dimension {
                hub.attachment_snapshots.push(snapshot);
                continue;
            }
            // Both non-player entities' (`Holder::Entity`) and tracked players'
            // (`Holder::Player`) initial non-empty synced attachments were already sent
            // atomically inside `ServerEntity::pairing`'s `BundleDelimiter` bundle; keep
            // `Holder::Entity` snapshots here for collider sync only. The recipient's own
            // `Holder::Player(uuid)` from `send_level_info` still sends with `replace: true`
            // even when empty, clearing stale replica fields on join or respawn.
            let messages = if reads && !matches!(holder, Holder::Entity(_)) {
                self.attachment_snapshot(holder)
            } else {
                Vec::new()
            };
            let sender = self
                .side
                .lan
                .as_ref()
                .and_then(|host| host.hub.guests.iter().find(|g| g.uuid == player))
                .map(|guest| guest.sender.clone());
            if let Some(sender) = sender {
                for message in &messages {
                    self.send_attachment_sync(std::slice::from_ref(&sender), message);
                }
                if let (true, Some(id)) = (reads_collider, collider_entity) {
                    self.send_collider_snapshot(&sender, id);
                }
            }
        }
        // A landed block entity is a new holder: what its clients held of it
        // went with their moving piston's copy.
        for holder in landings {
            let messages = self.attachment_snapshot(holder);
            let senders = self
                .side
                .lan
                .as_ref()
                .map_or_else(Vec::new, |host| host.hub.attachment_recipients(holder));
            for message in &messages {
                self.send_attachment_sync(&senders, message);
            }
        }
        for holder in std::mem::take(&mut self.world.attachment_syncs) {
            let Ok(map) = self.attachment_map_quiet(holder, mod_api::AttachmentAccess::Existing)
            else {
                continue;
            };
            let changes = map.take_sync_changes();
            if changes.is_empty() {
                continue;
            }
            let message = Message {
                holder,
                replace: false,
                changes,
            };
            let senders = self
                .side
                .lan
                .as_ref()
                .map_or_else(Vec::new, |host| host.hub.attachment_recipients(holder));
            self.send_attachment_sync(&senders, &message);
        }
    }

    /// Queue `message` for each of `senders` through the negotiated transport. A peer
    /// that has the channel absent, or is between epochs, is sent nothing and that is no
    /// error. Every other refusal is named in the log: a message
    /// past the connection's frame limits or one that cannot be written is not sent and the
    /// player stays joined; a full outgoing queue closes the connection, as a full queue
    /// did before, since the replica would miss the update.
    fn send_attachment_sync(&mut self, senders: &[PlayerSender], message: &Message) {
        if senders.is_empty() {
            return;
        }
        let body = attachment_sync::encode(message).map_err(|e| e.to_string());
        for sender in senders {
            let Err(error) = self.queue_attachment_sync(sender, &body) else {
                continue;
            };
            if !refused(&error) {
                continue;
            }
            log::warn!(
                "Sunrise: attachment sync for {:?} not sent: {error:?}",
                message.holder
            );
            if matches!(error, PayloadSendError::Backpressure) {
                sender.close();
            }
        }
    }

    /// Queue one encoded message (or its encoding failure) for one connection: the
    /// transport's typed answer, `FrameTooLarge` past the connection's frame limits.
    fn queue_attachment_sync(
        &mut self,
        sender: &PlayerSender,
        body: &Result<Vec<u8>, String>,
    ) -> Result<(), PayloadSendError> {
        let body = body
            .as_ref()
            .map_err(|reason| PayloadSendError::EncodeFailed(reason.clone()))?;
        self.queue_current_payload(
            Endpoint::server(sender.clone()),
            &PayloadId(attachment_sync::CHANNEL.into()),
            PayloadBytes(body.clone()),
        )
        .map(|_| ())
    }
}

/// Whether a send error is a refusal to name, not a peer without the channel or a
/// connection between epochs.
fn refused(error: &PayloadSendError) -> bool {
    !matches!(
        error,
        PayloadSendError::UnsupportedPeer(_)
            | PayloadSendError::NotNegotiated
            | PayloadSendError::WrongProtocol(_)
            | PayloadSendError::StaleEpoch
            | PayloadSendError::Disconnected
    )
}

#[cfg(test)]
#[path = "attachment_sync/general_snapshot_tests.rs"]
mod general_snapshot_tests;
#[cfg(test)]
#[path = "attachment_sync/route_cases_tests.rs"]
mod route_cases_tests;
#[cfg(test)]
#[path = "attachment_sync/transport_tests.rs"]
mod transport_tests;
