//! Client-side implementations of `side_ops::modapi`.
use super::Game;
use crate::game::transactions::decision::Residence;
use mod_api::AttachmentAccess;

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn client_attachment_map(
        &mut self,
        holder: mod_api::Holder,
        _access: AttachmentAccess,
    ) -> Result<&mut mod_api::Attachments, mod_api::AttachmentError> {
        let Some(remote) = self.remote.as_mut() else {
            return Err(mod_api::AttachmentError::MissingHolder);
        };
        remote.attachments.entry(holder).or_default();
        self.bind_remote_attachment(holder);
        let remote = self.remote.as_mut().unwrap();
        Ok(remote.attachments.get_mut(&holder).unwrap())
    }

    pub(in crate::game) fn client_entity_residence(&self, holder: mod_api::EntityRef) -> Residence {
        let Some(remote) = self.remote.as_ref() else {
            return Residence::Missing;
        };
        if self.native_lifecycle.epoch() != Some(holder.epoch()) {
            return Residence::Missing;
        }
        let Ok(id) = i32::try_from(holder.id()) else {
            return Residence::Missing;
        };
        if (id == remote.entity_id && self.player.is_some())
            || self.dragon.as_ref().is_some_and(|dragon| {
                crate::game::server_side::dragon::dragon_part_index(dragon.id, holder.id())
                    .is_some()
            })
            || remote.tracked(id).is_some()
        {
            Residence::Found
        } else {
            Residence::Missing
        }
    }
}
