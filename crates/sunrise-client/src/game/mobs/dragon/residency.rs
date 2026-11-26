//! Dragon-owned identity and attachment metadata.
use super::*;
#[derive(Default)]
pub(in crate::game) struct DragonResidency {
    pub(in crate::game) lifetime: sunrise_mod_runtime::attachments::AttachmentLifetime,
}

/// A real sub-entity: geometry, identity and attachment authority move and
/// retire together with this part, never with a shared parent attachment map.
pub(in crate::game) struct DragonPart(
    pub Vec3,
    pub Vec3,
    pub(in crate::game) sunrise_mod_runtime::attachments::AttachmentLifetime,
    pub(in crate::game) [i32; 4],
);
impl Default for DragonPart {
    fn default() -> Self {
        Self(
            Vec3::ZERO,
            Vec3::ZERO,
            Default::default(),
            sunrise_sim::save::insecure_uuid(&mut random_source::entity()),
        )
    }
}
impl sunrise_mod_runtime::attachments::HasAttachmentLifetime for DragonPart {
    fn attachment_lifetime(&mut self) -> &mut sunrise_mod_runtime::attachments::AttachmentLifetime {
        &mut self.2
    }
}
impl sunrise_mod_runtime::attachments::HasAttachmentLifetime for Dragon {
    fn attachment_lifetime(&mut self) -> &mut sunrise_mod_runtime::attachments::AttachmentLifetime {
        &mut self.residency.lifetime
    }
}

impl Game<'_, crate::game::server_side::ServerState> {
    /// Install a replacement only after retiring the previous parts' sidecars.
    pub(in crate::game::server_side) fn replace_resident_dragon(&mut self, dragon: Dragon) {
        self.discard_resident_dragon();
        self.dragon.replace(dragon);
    }

    /// Physical retirement must forget sidecar data before IDs can be reused,
    /// not merely unregister the attachment index when the parts are dropped.
    pub(in crate::game::server_side) fn discard_resident_dragon(&mut self) {
        if let Some(id) = self
            .dragon
            .as_ref()
            .filter(|dragon| dragon.id != 0 || self.remote.is_some())
            .map(|dragon| dragon.id)
        {
            for part in 0..8 {
                self.forget_entity(dragon_part_id(id, part));
            }
            // The parent's own sidecars go with it too, its `noPhysics` write
            // included; the parts above are its separate entity ids.
            self.forget_entity(id);
        }
        self.dragon.discard();
    }

    /// The dragon the level holds and the index (`getSubEntities` order) of
    /// its part with that entity id, while the dragon is there, dying or
    /// not: a part is an entity for as long as its parent is.
    pub(in crate::game::server_side) fn resident_dragon_part_source(
        &self,
        id: u32,
    ) -> Option<(&Dragon, usize)> {
        use sunrise_sim::join_seq::residency::{Source, table::EntityLookup};
        let EntityLookup::Found {
            location,
            value: Target::DragonPart(_),
        } = self.server_resident_entity(id)
        else {
            return None;
        };
        if location.owner.dimension != self.dim.key() {
            return None;
        }
        let Source::DragonPart(offset) = location.source else {
            return None;
        };
        Some((self.dragon.as_ref()?, usize::from(offset)))
    }

    pub(in crate::game) fn server_resident_dragon_part_index(&self, id: u32) -> Option<usize> {
        self.resident_dragon_part_source(id).map(|(_, part)| part)
    }

    pub(in crate::game::server_side) fn assign_resident_crystal_ids(&mut self) {
        crate::game::entity_residency::assign_resident_ids!(
            self.crystals,
            c,
            &mut self.side.next_entity,
            None
        );
    }
}
