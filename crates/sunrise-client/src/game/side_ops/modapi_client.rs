//! Client-side implementations of `side_ops::modapi`.
use super::Game;
use crate::game::transactions::decision::Residence;
use mod_api::AttachmentAccess;

impl Game<'_> {
    pub(in crate::game) fn client_attachment_map(
        &mut self,
        holder: mod_api::Holder,
        _access: AttachmentAccess,
    ) -> Result<&mut mod_api::Attachments, mod_api::AttachmentError> {
        if self.client_attachment_entity_exists(holder) {
            self.initialize_entity_on_access(holder);
        }
        let Some(remote) = self.remote.as_mut() else {
            return Err(mod_api::AttachmentError::MissingHolder);
        };
        remote.attachments.entry(holder).or_default();
        self.bind_remote_attachment(holder);
        let remote = self.remote.as_mut().unwrap();
        Ok(remote.attachments.get_mut(&holder).unwrap())
    }

    pub(in crate::game) fn client_entity_residence(&self, holder: mod_api::EntityRef) -> Residence {
        if self.native_client_entity_target(holder).is_some() {
            Residence::Found
        } else {
            Residence::Missing
        }
    }
}
