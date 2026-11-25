#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tracking::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::particles::TrackingBody;

impl Game<'_> {
    pub(in crate::game) fn detach_remote_tracking_entity(&mut self, id: i32) {
        let target = TrackingTarget::Entity(id);
        if self.particles.0.has_tracking_target(target) {
            let last_body = self.remote_entity_tracking_body(id);
            self.particles
                .0
                .detach_tracking_target(target, || last_body);
        }
    }
    /// The current entity position and dimensions used by `TrackingEmitter.tick`.
    /// Read body copies rather than the network position of an interpolated entity.
    pub(in crate::game::client_side) fn remote_entity_tracking_body(
        &self,
        id: i32,
    ) -> Option<TrackingBody> {
        let remote = self.remote.as_ref()?;
        if let Some(row) = remote.general.rows.get(&id) {
            let (width, height) =
                super::super::general::general_type_catalog().dimensions(row.type_slot);
            return Some(TrackingBody {
                feet: row.position,
                width,
                height,
            });
        }
        let (kind, last_position) = remote.tracked(id)?;
        let eid = id as u32;
        // The dragon's render and motion body is separate from its mob row.
        if kind != "minecraft:ender_dragon"
            && (kind == "minecraft:player" || self.res.mob_types().contains_key(kind))
            && let Some(i) = self.mob_index.find_ref(&self.mobs, eid)
        {
            return Some(mob_tracking_body(&self.mobs[i]));
        }
        if let Some(entity) = self.client.interactions.get(&id) {
            let Vec3 { x, y, z } = entity.position;
            return Some(TrackingBody {
                feet: [x, y, z],
                width: f64::from(entity.width),
                height: f64::from(entity.height),
            });
        }
        let (mut width, mut height) = self.res.rest().spawner_types.get(kind)?.entity_dimensions();
        let feet = match kind {
            "minecraft:ender_dragon" => {
                let dragon = self.dragon.as_ref().filter(|dragon| dragon.id == eid)?;
                let Vec3 { x, y, z } = dragon.position;
                [x, y, z]
            }
            "minecraft:armor_stand" => {
                let stand = self.armor_stands.iter().find(|stand| stand.id == eid)?;
                (width, height) = super::super::entity::armor_stand_size(stand);
                let Vec3 { x, y, z } = stand.position;
                [x, y, z]
            }
            "minecraft:area_effect_cloud" => {
                let cloud = self.potions.clouds.iter().find(|cloud| cloud.id == eid)?;
                width = f64::from(cloud.radius * 2.0);
                cloud.at
            }
            "minecraft:splash_potion"
            | "minecraft:lingering_potion"
            | "minecraft:experience_bottle" => {
                let (_, _, _, position, _) =
                    self.potions.thrown_tracked().find(|(id, ..)| *id == eid)?;
                [position.x, position.y, position.z]
            }
            "minecraft:fishing_bobber" => {
                let hook = self
                    .client
                    .remote_hooks
                    .iter()
                    .find(|hook| hook.id == eid)?;
                let Vec3 { x, y, z } = hook.position;
                [x, y, z]
            }
            _ => self
                .text_displays
                .0
                .get(&id)
                .map(|display| display.position().to_array())
                .or_else(|| {
                    self.level_entity(id)
                        .and_then(|target| self.target_position(target))
                })
                .unwrap_or(last_position),
        };
        Some(TrackingBody {
            feet,
            width,
            height,
        })
    }
}
