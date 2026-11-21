//! The compact output of the authoritative network NBT decoder.
use super::{Message, decode_changes, value_limits};
use crate::{Error, Result, packets::configuration::CustomPayload, retained::OwnedHeap};
use sunrise_core::nbt::{NetworkTape, Tag};
use sunrise_mod_runtime::attachments::{AttachmentSync, Holder};

/// A structurally decoded attachment message awaiting the holder's turn.
pub struct StagedMessage {
    holder: Holder,
    replace: bool,
    changes: usize,
    tape: NetworkTape,
    payload: CustomPayload,
}

pub(super) fn stage(mut payload: CustomPayload) -> Result<StagedMessage> {
    let mut tape = NetworkTape::default();
    let (holder, replace, changes) = decode_changes(&payload.data, |reader, namespace, name| {
        tape.push_string(namespace);
        tape.push_string(name);
        if !matches!(reader.stage_nbt(value_limits(), &mut tape)?, 0 | 10) {
            return Err(Error::new("attachment sync: value is not a compound"));
        }
        Ok(())
    })?;
    tape.shrink_to_fit();
    // Array values can fill most of a memory frame. Reuse their packet bytes
    // as typed backing instead of retaining a second allocation in ingress.
    tape.swap_array_endianness(&mut payload.data);
    Ok(StagedMessage {
        holder,
        replace,
        changes: changes.len(),
        tape,
        payload,
    })
}

impl StagedMessage {
    /// Bytes of the owned staging allocations, not counting the inline message.
    pub fn owned_heap_bytes(&self) -> usize {
        self.tape.owned_heap_bytes() + self.payload.owned_heap_bytes()
    }

    /// Build the holder's values, then restore the original packet for observers.
    pub fn into_message(mut self) -> (Message, CustomPayload) {
        let mut tags = self.tape.tags(&self.payload.data);
        let changes = (0..self.changes)
            .map(|_| {
                let (Some(Tag::String(namespace)), Some(Tag::String(name))) =
                    (tags.next(), tags.next())
                else {
                    unreachable!("each attachment change has a namespace and name");
                };
                let value = match tags.next() {
                    Some(Tag::End) => None,
                    Some(Tag::Compound(compound)) => Some(compound),
                    _ => unreachable!("attachment roots are compounds or removals"),
                };
                AttachmentSync {
                    namespace,
                    name,
                    value,
                }
            })
            .collect();
        drop(tags);
        self.tape.swap_array_endianness(&mut self.payload.data);
        (
            Message {
                holder: self.holder,
                replace: self.replace,
                changes,
            },
            self.payload,
        )
    }
}
