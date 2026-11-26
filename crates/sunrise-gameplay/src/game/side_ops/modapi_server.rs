//! Server-side implementations of `side_ops::modapi`.
use crate::game::ServerGame;
use sunrise_gameplay_common::game::transactions::decision::Residence;
use mod_api::AttachmentAccess;

impl ServerGame<'_> {
    pub(in crate::game) fn server_attachment_map(
        &mut self,
        holder: mod_api::Holder,
        access: AttachmentAccess,
    ) -> Result<&mut mod_api::Attachments, mod_api::AttachmentError> {
        self.common.world.attachment_syncs.insert(holder);
        self.attachment_map_quiet(holder, access)
    }

    pub(in crate::game) fn server_entity_residence(&self, holder: mod_api::EntityRef) -> Residence {
        use sunrise_sim::join_seq::residency::table::EntityLookup;
        match self.server_resident_entity(holder.id()) {
            EntityLookup::Taken => Residence::Taken,
            EntityLookup::Found { .. } => Residence::Found,
            EntityLookup::Missing => Residence::Missing,
        }
    }
}
