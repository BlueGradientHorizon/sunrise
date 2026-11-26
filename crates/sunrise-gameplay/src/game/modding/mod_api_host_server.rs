use crate::game::ServerGame;
use super::mod_api_host::world_attachments;
use mod_api::AttachmentAccess;
use sunrise_mod_runtime::attachments::{ATTACHMENTS_KEY, attachment_compound};

/// The entity families whose saved maps bind an owning lifetime
/// (`bind_saved_entity_attachments`), so a first store publishes index rows for them.
///
/// Every other family stays unindexed on a first store until its saved binding lands: the value
/// is kept and saved, but no query visits it. Cushions, hooks, TNT, fangs, crystals, falling
/// blocks, rockets, tridents, spawners, area-effect clouds, fence knots, the dragon itself and
/// `Other` (markers, displays) are unindexed today (#995).
fn saved_binding_target(target: super::commands::Target) -> bool {
    use super::commands::Target;
    matches!(
        target,
        Target::Mob(_)
            | Target::Item(_)
            | Target::Orb(_)
            | Target::Projectile(_)
            | Target::Stand(_)
            | Target::Frame(_)
            | Target::Painting(_)
            | Target::Boat(_)
            | Target::Cart(_)
            | Target::DragonPart(_)
            | Target::General(_)
    )
}

impl ServerGame<'_> {
    /// Attachment membership uses physical presence (except removed mobs), not the
    /// command resolver's pending-removal masks. Limit resolution to this level.
    pub(in crate::game) fn attachment_entity_exists(&self, id: u32) -> bool {
        use sunrise_sim::join_seq::residency::table::EntityLookup;
        matches!(
            self.resident_entities
                .table
                .borrow_mut()
                .entity_lookup(&self.dim.key(), id, |at| {
                    use sunrise_sim::join_seq::residency::Kind;
                    match at.owner.kind {
                        Kind::Hook | Kind::Player => self
                            .locate_specialized_resident_owner(at)
                            .map(|mut source| {
                                // Hooks were counted by physical presence, regardless of pending
                                // removals. Players were not, but must still validate their row.
                                source.value.1 = at.owner.kind == Kind::Hook;
                                source
                            }),
                        _ => self.locate_resident_attachment_owner(at),
                    }
                }),
            EntityLookup::Found {
                value: (_, true),
                ..
            }
        )
    }

    /// Object ownership for persistence cleanup, never a level-query membership set.
    pub(in crate::game::server_side) fn retained_entity_ids(
        &self,
    ) -> std::collections::HashSet<u32> {
        let mut retained = self.live_entity_ids();
        retained.extend(self.common.persist.off_level.keys().copied());
        retained
    }

    /// The attachments of entities that left the level, dropped: each tick
    /// while any entity has some. Only those entities are looked at, and
    /// nothing else of the entities' memory changes.
    pub(in crate::game::server_side) fn prune_entity_attachments(&mut self) {
        let alive = self.retained_entity_ids();
        let gone: Vec<u32> = self
            .common.persist
            .uuids
            .keys()
            .copied()
            .filter(|id| self.common.entity_attachments.contains_key(id) && !alive.contains(id))
            .collect();
        for id in gone {
            self.common.forget_entity_initialization(mod_api::Holder::Entity(id));
            self.common.entity_attachments.remove(&id);
        }
    }

    pub(in crate::game) fn native_authoritative(&self) -> bool {
        self.common.is_logical_server()
    }

    /// Access a holder's attachment map, to lend to a mod: a server's, whose
    /// synced changes are sent at the pass's end, or a client's copy of what
    /// its server synced.
    #[allow(dead_code)]
    pub(in crate::game::server_side) fn attachment_map(
        &mut self,
        holder: mod_api::Holder,
        access: AttachmentAccess,
    ) -> Result<&mut mod_api::Attachments, mod_api::AttachmentError> {
        self.side_attachment_map(holder, access)
    }

    /// `attachment_map` without recording the holder for the sync. Inside a transaction, the
    /// map is kept as it was on its first access, so an abort puts it back.
    pub(in crate::game) fn attachment_map_quiet(
        &mut self,
        holder: mod_api::Holder,
        access: AttachmentAccess,
    ) -> Result<&mut mod_api::Attachments, mod_api::AttachmentError> {
        if !self.native_authoritative() {
            return Err(mod_api::AttachmentError::MissingHolder);
        }
        self.journal_attachments(holder);
        self.attachment_slot(holder, access)
    }

    /// Bind first-store contents to their real owner, including writes made before
    /// insertion by EntityConstructing listeners. Never invent a temporary owner.
    pub(in crate::game::server_side) fn bind_resident_entity_attachments(
        &mut self,
        id: u32,
    ) -> Result<(), mod_api::AttachmentError> {
        use mod_api::AttachmentError;
        let id = std::num::NonZeroU32::new(id).ok_or(AttachmentError::MissingHolder)?;
        if matches!(
            self.with_resident_slot(&self.dim.key(), id, |slot| slot.is_bound()),
            super::entity_residency::SlotAccess::Done(true)
        ) {
            return Ok(());
        }
        if !matches!(
            self.server_resident_entity(id.get()),
            sunrise_sim::join_seq::residency::table::EntityLookup::Found {
                value,
                ..
            } if saved_binding_target(value)
        ) {
            return Ok(());
        }
        if !self.common.entity_attachments.contains_key(&id.get()) {
            return Ok(());
        }
        // Cleanup owns per-level UUID metadata, while attachment maps are global.
        // Record the actual part identity so its level alone retires its map.
        if self.level_dragon_part(id.get()).is_some() {
            let uuid = self.uuid_for(id.get());
            self.common.persist.uuids.insert(id.get(), uuid);
        }
        let level = self
            .level(self.common.dim.id())
            .ok_or(AttachmentError::MissingHolder)?;
        let holder =
            mod_api::AttachmentHolder::Entity(mod_api::EntityRef::new(level.epoch(), id.get()));
        let index = self.common.attachment_indexes.entry(self.common.dim).or_default().clone();
        let mut map = self.common.entity_attachments.remove(&id.get()).unwrap();
        self.with_resident_slot(&self.common.dim.key(), id, |slot| {
            if !slot.is_bound() {
                slot.bind(&mut map, &index, holder);
            }
        });
        self.common.entity_attachments.insert(id.get(), map);
        Ok(())
    }

    /// Where `holder`'s attachment map lives now: an entity's by its object ID, a player's where
    /// their state is (swapped in for a guest's turn, or kept beside), the world's others.
    pub(in crate::game) fn attachment_slot(
        &mut self,
        holder: mod_api::Holder,
        access: AttachmentAccess,
    ) -> Result<&mut mod_api::Attachments, mod_api::AttachmentError> {
        use mod_api::AttachmentError;
        if let mod_api::Holder::Entity(id) = holder {
            if self.common.entity_initialization.active != Some(holder) {
                use super::entity_residency::SlotAccess;
                use sunrise_sim::join_seq::residency::Kind;
                let id = std::num::NonZeroU32::new(id).ok_or(AttachmentError::MissingHolder)?;
                match self.with_resident_slot(&self.common.dim.key(), id, |_| ()) {
                    SlotAccess::Done(()) => {}
                    SlotAccess::Taken => return Err(AttachmentError::HolderBusy),
                    SlotAccess::Missing => return Err(AttachmentError::MissingHolder),
                    SlotAccess::UnsupportedKind(kind) => {
                        let name = match kind {
                            Kind::Mob => "mob",
                            Kind::Item => "item",
                            Kind::Orb => "orb",
                            Kind::Projectile => "projectile",
                            Kind::Stand => "armorstand",
                            Kind::Frame => "itemframe",
                            Kind::Painting => "painting",
                            Kind::Boat => "boat",
                            Kind::Cart => "minecart",
                            Kind::Other => "other",
                            Kind::Fangs => "fangs",
                            Kind::Crystal => "crystal",
                            Kind::Dragon => "dragon",
                            Kind::Tnt => "tnt",
                            Kind::Falling => "falling",
                            Kind::Rocket => "rocket",
                            Kind::Trident => "trident",
                            Kind::Spawner => "spawner",
                            Kind::Cloud => "cloud",
                            Kind::Bolt => "bolt",
                            Kind::Cushion => "cushion",
                            Kind::Knot => "knot",
                            Kind::Player => "player",
                            Kind::Hook => "hook",
                            Kind::General => "general",
                            Kind::DragonPart => "dragonpart",
                        };
                        return Err(AttachmentError::UnsupportedHolder(name));
                    }
                }
            }
            if self.common.entity_initialization.active != Some(holder)
                && !self.attachment_entity_exists(id)
            {
                return Err(AttachmentError::MissingHolder);
            }
            self.initialize_entity_on_access(holder);
            attachment_compound(
                self.common.persist
                    .raw
                    .get(&id)
                    .and_then(|tag| tag.get(ATTACHMENTS_KEY)),
            )?;
            if access == AttachmentAccess::Store
                && self
                    .common.persist
                    .removals
                    .contains(&super::commands::Target::Painting(id))
            {
                return Err(AttachmentError::MissingHolder);
            }
            if access == AttachmentAccess::Store
                && self.common.entity_initialization.active != Some(holder)
            {
                self.common.entity_attachments.entry(id).or_default();
                self.bind_resident_entity_attachments(id)?;
            }
            return Ok(self.common.entity_attachments.entry(id).or_default());
        }
        if let mod_api::Holder::Player(uuid) = holder {
            let cloning = self.cloning_guest(uuid).map(|guest| guest.conn);
            if self.common.entity_initialization.active != Some(holder)
                && cloning.is_none()
                && self.server.lan_acting_uuid != Some(uuid)
                && self
                    .server
                    .lan
                    .as_ref()
                    .and_then(|host| host.hub.holder_guest(uuid, self.common.dim.id()))
                    .is_none()
            {
                return Err(AttachmentError::MissingHolder);
            }
            self.initialize_entity_on_access(holder);
            if self.server.lan_acting_uuid == Some(uuid) {
                if access == AttachmentAccess::Store
                    && let Some(conn) = self.server.lan_acting
                {
                    self.bind_player_attachments(conn, true);
                }
                return Ok(&mut self.common.player_state.attachments);
            }
            let conn = cloning
                .or_else(|| {
                    let host = self.server.lan.as_ref()?;
                    let guest = if self.common.entity_initialization.active == Some(holder) {
                        host.hub.guests.iter().find(|guest| guest.uuid == uuid)
                    } else {
                        host.hub.holder_guest(uuid, self.common.dim.id())
                    }?;
                    Some(guest.conn)
                })
                .ok_or(AttachmentError::MissingHolder)?;
            if self.server.lan_guest_swaps.contains(&conn) {
                return Err(AttachmentError::MissingHolder);
            }
            if access == AttachmentAccess::Store {
                self.bind_player_attachments(conn, true);
            }
            return Ok(self
                .server
                .lan
                .as_mut()
                .and_then(|host| host.hub.guests.attachments_mut(conn))
                .ok_or(AttachmentError::MissingHolder)?);
        }
        world_attachments(&mut self.common.world, holder)
    }
}

use sunrise_gameplay_common::game::side_ops::modapi::ModapiSideGameExt as _;

use sunrise_gameplay_common::game::entity_initialization_common::EntityInitializationCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
