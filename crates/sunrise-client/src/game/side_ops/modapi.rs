//! Side operations for mod-API host adapters (`AttachmentHost`, `GameHost::residence`).
use crate::game::{Game, transactions::decision::Residence};
use mod_api::AttachmentAccess;

impl Game<'_> {
    pub(in crate::game) fn side_attachment_map(
        &mut self,
        holder: mod_api::Holder,
        access: AttachmentAccess,
    ) -> Result<&mut mod_api::Attachments, mod_api::AttachmentError> {
        if self.is_logical_server() {
            self.server_attachment_map(holder, access)
        } else {
            self.client_attachment_map(holder, access)
        }
    }

    pub(in crate::game) fn side_entity_residence(&self, holder: mod_api::EntityRef) -> Residence {
        if self.is_logical_server() {
            self.server_entity_residence(holder)
        } else {
            self.client_entity_residence(holder)
        }
    }
}
