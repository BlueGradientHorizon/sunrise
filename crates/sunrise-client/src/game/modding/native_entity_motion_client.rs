use super::entity::{Entity as _, EntityHandle, with_entity};
use crate::game::native_entity_motion_values::corners;
use crate::game::{Game, commands_common::Target};
use mod_api::EntityRef;
use sunrise_core::geometry::Vec3;

impl Game<'_, crate::game::client_side::ClientState> {
    fn client_walk_entity(&self, id: u32) -> Option<EntityHandle> {
        if let Some(i) = self.mobs.iter().position(|e| e.id == id) {
            return Some(EntityHandle::Mob(i));
        }
        if let Some(i) = self.dropped.iter().position(|e| e.id == id) {
            return Some(EntityHandle::Item(i));
        }
        if let Some(i) = self.boats.iter().position(|e| e.id == id) {
            return Some(EntityHandle::Boat(i));
        }
        if let Some(i) = self.carts.iter().position(|e| e.id == id) {
            return Some(EntityHandle::Cart(i));
        }
        if let Some(i) = self.armor_stands.iter().position(|e| e.id == id) {
            return Some(EntityHandle::ArmorStand(i));
        }
        if let Some(i) = self.tnts.iter().position(|e| e.id == id) {
            return Some(EntityHandle::Tnt(i));
        }
        if let Some(i) = self.falling.iter().position(|e| e.id == id) {
            return Some(EntityHandle::Falling(i));
        }
        if let Some(i) = self.orbs.iter().position(|e| e.id == id) {
            return Some(EntityHandle::Orb(i));
        }
        if let Some(i) = self.creatures.projectiles.iter().position(|e| e.id == id) {
            return Some(EntityHandle::Projectile(i));
        }
        if let Some(i) = self.rockets.iter().position(|e| e.id == id) {
            return Some(EntityHandle::Rocket(i));
        }
        if let Some(i) = self.tridents.iter().position(|e| e.id == id) {
            return Some(EntityHandle::Trident(i));
        }
        self.resident_entities
            .general
            .rows
            .iter()
            .position(|e| e.id == id)
            .map(EntityHandle::General)
    }

    pub(in crate::game) fn client_entity_velocity(&self, entity: EntityRef) -> Option<[f64; 3]> {
        let t = self.native_client_entity_target(entity)?;
        let v = match t {
            Target::Player => self.player.as_ref()?.velocity,
            _ => match self.client_walk_entity(entity.id()) {
                Some(h) => with_entity!(self, h, e => e.velocity()),
                None => Vec3::ZERO,
            },
        };
        Some([v.x, v.y, v.z])
    }

    pub(in crate::game) fn client_entity_bounding_box(
        &self,
        entity: EntityRef,
    ) -> Option<[[f64; 3]; 2]> {
        if let Some(pos) = self.side_client_display_position(entity) {
            return Some([pos; 2]);
        }
        if let Some((remote, id)) = self.native_client_entity_context(entity)
            && let Some(row) = remote.general.rows.get(&id)
        {
            return Some(corners(row.bounds()));
        }
        let t = self.native_client_entity_target(entity)?;
        if t == Target::Player && self.side.remote.is_some() {
            return Some(corners(self.player.as_ref()?.bounds()));
        }
        if let Some(h) = self.client_walk_entity(entity.id()) {
            return Some(corners(with_entity!(self, h, e => {
                self.client_copy_box(e.id(), e.position(), e.bounds())
            })));
        }
        let (lo, hi) = match t {
            Target::Other(id) => {
                let i = self.side.interactions.get(&(id as i32))?;
                return Some(corners(i.bounds()));
            }
            Target::DragonPart(id) => {
                let dragon = self.dragon.as_ref()?;
                let part = crate::game::dragon_common::dragon_part_index(dragon.id, id)?;
                return Some(corners(dragon.part_geometry(part).1));
            }
            Target::Crystal(id) => {
                let c = self.crystals.iter().find(|c| c.id == id)?;
                return Some(corners(self.client_copy_box(id, c.position, c.bounds())));
            }
            Target::Cushion(id) => self.cushions.iter().find(|c| c.id == id)?.bounds(),
            Target::Frame(id) => self.item_frames.iter().find(|f| f.id == id)?.bounds(),
            Target::Painting(id) => self.paintings.iter().find(|p| p.id == id)?.bounds(),
            _ => return None,
        };
        if lo.iter().chain(&hi).any(|v| !v.is_finite()) {
            return None;
        }
        Some([lo, hi])
    }

    pub(in crate::game) fn client_entity_eye_height(&self, entity: EntityRef) -> Option<f64> {
        if let Some(target) = self.native_client_entity_target(entity) {
            return Some(self.target_eye_height(target));
        }
        let (remote, id) = self.native_client_entity_context(entity)?;
        remote.tracked(id)?;
        self.side
            .interactions
            .get(&id)
            .map(|i| f64::from(i.eye_height()))
    }
}
