//! Unified crosshair entity picking (`LocalPlayer.pick` and `ProjectileUtil.getEntityHitResult`).
use super::Game;
use super::entity::{Entity as _, entity_lists_of, for_each_entity};
use super::pick_accumulator::{PickAccumulator, PickCandidate, pick_ray_segment};
use mod_api::{BlockPos, CrosshairTarget, EntityRef};
use sunrise_client::{
    items::ItemStack,
    join_seq::{JoinSeq, Joined as _},
    save::uuid_text,
};
use sunrise_core::geometry::{Aabb, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CrosshairEntityHit {
    pub id: i32,
    pub distance: f64,
    pub point: [f64; 3],
    pub pickable: bool,
    pub suggest_uuid: bool,
}

fn aabb_extents((lo, hi): ([f64; 3], [f64; 3])) -> Aabb {
    Aabb::new(
        Vec3::new(lo[0], lo[1], lo[2]),
        Vec3::new(hi[0], hi[1], hi[2]),
    )
}

impl Game<'_> {
    pub(super) fn crosshair_entity_hit(&self) -> Option<CrosshairEntityHit> {
        if let Some(hit) = self.cached_entity_pick() {
            let (entity, distance, point) = hit?;
            return Some(CrosshairEntityHit {
                id: entity.id() as i32,
                distance,
                point,
                pickable: self.client_pick.entity_metadata.0,
                suggest_uuid: self.client_pick.entity_metadata.1,
            });
        }
        let hit = self.pick_entity_hit_unified(false)?;
        let is_synthetic_dragon_part =
            (super::dragon::SPECIAL..super::dragon::CRYSTAL_IDS).contains(&hit.id);
        let id = hit.id as i32;
        let pickable = !self.interactions.contains_key(&id)
            && !is_synthetic_dragon_part
            && Self::has_pick_result(self.level_entity(id));
        let suggest_uuid = !is_synthetic_dragon_part && self.level_dragon_part(hit.id).is_none();
        Some(CrosshairEntityHit {
            id,
            distance: hit.distance_sq.sqrt(),
            point: hit.point,
            pickable,
            suggest_uuid,
        })
    }

    /// The stored frame decision on a window; original during dispatch.
    pub(super) fn crosshair_target(&self) -> Option<CrosshairTarget> {
        use crate::game::api_values::TryIntoApi;
        if !self.native_window() || self.title.is_some() {
            return None;
        }
        self.player.as_ref()?;
        if self.uses_stored_pick() {
            return self.stored_client_pick();
        }
        let (hit, occluded_block) = self.pick_entity_pass_unified(false);
        if let Some(hit) = hit {
            return Some(CrosshairTarget::Entity {
                entity: EntityRef::new(self.native_lifecycle.epoch()?, hit.id),
                point: hit.point,
            });
        }
        if occluded_block {
            return None;
        }
        let hit = self.pick()?;
        Some(CrosshairTarget::Block {
            pos: BlockPos::new(hit.pos[0], hit.pos[1], hit.pos[2]),
            state: self
                .world
                .block(hit.pos[0], hit.pos[1], hit.pos[2])
                .try_into_api()
                .ok()?,
            hit: hit.native(),
        })
    }

    pub(super) fn pick_entity_candidate_within(
        &mut self,
        max_distance: f64,
        max_distance_sq: f64,
    ) -> Option<CrosshairTarget> {
        let subscribed = mod_api::__private::has_scoped_hook(mod_api::HookId::CanRiderInteract);
        let mut pick = if subscribed {
            PickAccumulator::recording(
                max_distance_sq,
                std::mem::take(&mut self.client_pick.rider_candidates),
            )
        } else {
            PickAccumulator::new(max_distance_sq)
        };
        self.collect_pick_candidates_within(max_distance, true, &mut pick);
        if subscribed {
            let seal = self.transaction_open();
            self.client_pick.rider_candidates = pick.dispatch(self);
            self.transaction_close(&seal, true);
        }
        let hit = pick.hovered?;
        Some(CrosshairTarget::Entity {
            entity: EntityRef::new(self.native_lifecycle.epoch()?, hit.id),
            point: hit.point,
        })
    }

    /// Runs the unified `LocalPlayer.pick` entity pass for unstored consumers.
    pub(super) fn pick_entity_hit_unified(&self, include_knots: bool) -> Option<PickCandidate> {
        self.pick_entity_pass_unified(include_knots).0
    }

    fn pick_entity_pass_unified(&self, include_knots: bool) -> (Option<PickCandidate>, bool) {
        let Some((eye, dir)) = self.aim() else {
            return (None, false);
        };
        let from = Vec3::new(eye[0], eye[1], eye[2]);
        let max_distance = self.block_reach.max(self.entity_reach);
        let (entity_max_distance, block_distance_sq) = if self.client_pick.ray.is_some() {
            (
                max_distance,
                pick_ray_segment(eye, dir, max_distance)
                    .1
                    .distance_squared(from),
            )
        } else if let Some(block) = self.pick_unfiltered() {
            let d2 =
                from.distance_squared(Vec3::new(block.point[0], block.point[1], block.point[2]));
            (d2.sqrt(), d2)
        } else {
            (
                max_distance,
                pick_ray_segment(eye, dir, max_distance)
                    .1
                    .distance_squared(from),
            )
        };
        let mut pick = PickAccumulator::new(block_distance_sq);
        self.collect_pick_candidates_within(entity_max_distance, include_knots, &mut pick);
        let Some(hit) = pick.hovered else {
            return (None, false);
        };
        if hit.distance_sq >= block_distance_sq {
            return (None, false);
        }
        if hit.distance_sq < self.entity_reach * self.entity_reach {
            (Some(hit), false)
        } else {
            (None, true)
        }
    }

    /// Single pickable-entity query (`Entity::is_pickable` plus attached/special families in join order).
    pub(super) fn collect_pick_candidates_within(
        &self,
        max_distance: f64,
        include_knots: bool,
        pick: &mut PickAccumulator,
    ) {
        let Some((eye, dir)) = self.aim() else {
            return;
        };
        let (from, to) = pick_ray_segment(eye, dir, max_distance);
        let mut hits: Vec<(JoinSeq, PickCandidate)> = Vec::new();
        let mut push_box = |seq: JoinSeq, id: u32, bb: Aabb| {
            if let Some(candidate) = self.test_pick_box(id, from, to, bb) {
                hits.push((seq, candidate));
            }
        };

        let lists = entity_lists_of!(self);
        for_each_entity!(lists, entity => {
            if entity.is_pickable() {
                let radius = entity.pick_radius();
                let bb = if radius == 0.0 {
                    entity.bounds()
                } else {
                    entity.bounds().inflate(Vec3::new(radius, radius, radius))
                };
                push_box(entity.join_seq(), entity.id(), bb);
            }
        });

        for p in &self.paintings {
            push_box(p.join, p.id, aabb_extents(p.bounds()));
        }
        for f in &self.item_frames {
            push_box(f.join, f.id, aabb_extents(f.bounds()));
        }
        for c in &self.cushions {
            push_box(c.join, c.id, aabb_extents(c.bounds()));
        }
        for (&id, i) in &self.interactions {
            if let Ok(uid) = u32::try_from(id) {
                push_box(i.join, uid, i.bounds());
            }
        }
        if let Some(remote) = &self.remote {
            let catalog = crate::game::general::general_type_catalog();
            for (&id, row) in &remote.general.rows {
                if catalog.pickable(row.type_slot)
                    && let Ok(uid) = u32::try_from(id)
                {
                    push_box(row.join, uid, row.bounds());
                }
            }
        }
        if include_knots {
            for (&id, &block) in &self.remote_knots {
                let knot = sunrise_client::mobs::leash::knot_state(block);
                let p = knot.position;
                let half = knot.width / 2.0;
                let bb = Aabb::new(
                    Vec3::new(p.x - half, p.y, p.z - half),
                    Vec3::new(p.x + half, p.y + knot.height, p.z + half),
                );
                push_box(JoinSeq::MAX, id, bb);
            }
        }
        for (i, crystal) in self.crystals.iter().enumerate() {
            let id = if crystal.id != 0 {
                crystal.id
            } else if self.remote.is_none() {
                super::dragon::CRYSTAL_IDS + i as u32
            } else {
                continue;
            };
            push_box(crystal.join, id, crystal.bounds());
        }
        if let Some(d) = self.dragon.as_ref().filter(|d| d.alive()) {
            if d.id != 0 {
                for i in 0..8 {
                    push_box(d.join, d.id + 1 + i as u32, d.part_geometry(i).1);
                }
            } else if self.remote.is_none() {
                for i in 0..8 {
                    push_box(
                        d.join,
                        super::dragon::SPECIAL + i as u32,
                        d.part_geometry(i).1,
                    );
                }
            }
        }

        if hits.len() > 1 {
            hits.sort_by_key(|(seq, _)| *seq);
        }
        for (_, candidate) in hits {
            pick.consider_candidate(self, candidate);
        }
    }

    /// Entity under the crosshair before any block (`Some(None)` when `getPickResult` is empty).
    pub(super) fn pick_entity_hit_at(&self) -> Option<(f64, Option<i32>)> {
        self.crosshair_entity_hit()
            .map(|hit| (hit.distance, hit.pickable.then_some(hit.id)))
    }
    pub(super) fn pick_entity_hit(&self) -> Option<Option<i32>> {
        self.pick_entity_hit_at().map(|hit| hit.1)
    }
    /// Returns the wire entity id under the crosshair before any block.
    #[cfg(test)]
    pub(in crate::game) fn crosshair_entity_id(&self) -> Option<i32> {
        self.crosshair_entity_hit().map(|hit| hit.id)
    }

    /// `ClientSuggestionProvider.getSelectedEntities`: UUID of the crosshair entity hit.
    pub(super) fn crosshair_entity_uuid(&self) -> Option<String> {
        self.crosshair_entity_hit()
            .filter(|hit| hit.suggest_uuid)
            .and_then(|hit| self.persist.uuids.get(&(hit.id as u32)).copied())
            .map(uuid_text)
    }

    /// `Entity.getPickResult` of the entity `id`.
    pub(super) fn entity_pick_result(&self, id: i32) -> Option<ItemStack> {
        let id = u32::try_from(id).ok()?;
        let item = if let Some(m) = self.mobs.iter().find(|m| m.id == id) {
            // Mob.getPickResult: SpawnEggItem.byId.
            let egg = format!("{}_spawn_egg", m.kind.id);
            return self.items().get(&egg).map(|_| ItemStack::new(&egg, 1));
        } else if let Some(b) = self.boats.iter().find(|b| b.id == id) {
            b.entity_type()
        } else if let Some(c) = self.carts.iter().find(|c| c.id == id) {
            c.kind.item().to_owned()
        } else if self.armor_stands.iter().any(|a| a.id == id) {
            "minecraft:armor_stand".to_owned()
        } else if let Some(c) = self.cushions.iter().find(|c| c.id == id) {
            c.item()
        } else if let Some(f) = self.item_frames.iter().find(|f| f.id == id) {
            // ItemFrame.getPickResult: what it frames, else the frame itself.
            return Some(
                f.item
                    .clone()
                    .unwrap_or_else(|| ItemStack::new(f.kind(), 1)),
            );
        } else if self.paintings.iter().any(|p| p.id == id) {
            "minecraft:painting".to_owned()
        } else if self.crystals.iter().any(|c| c.id == id) {
            "minecraft:end_crystal".to_owned()
        } else {
            return None;
        };
        Some(ItemStack::new(&item, 1))
    }

    /// `Player.isWithinEntityInteractionRange(entity, 3.0)`.
    pub(super) fn within_entity_range(&self, id: i32) -> bool {
        let Some(eye) = self.eye_position() else {
            return false;
        };
        let mob = u32::try_from(id)
            .ok()
            .and_then(|id| self.mobs.iter().find(|m| m.id == id))
            .map(|m| m.bounds());
        let Some(b) = self
            .object_target(id)
            .map(|(lo, hi, _)| (lo, hi))
            .or_else(|| {
                mob.or_else(|| self.tracked_special_box(id)).map(|b| {
                    let (lo, hi) = (b.min_position(), b.max_position());
                    ([lo.x, lo.y, lo.z], [hi.x, hi.y, hi.z])
                })
            })
        else {
            return false;
        };
        let d2: f64 = (0..3)
            .map(|a| (eye[a] - eye[a].clamp(b.0[a], b.1[a])).powi(2))
            .sum();
        let range = self.entity_reach + 3.0;
        d2 < range * range
    }
}

#[cfg(test)]
mod tests {
    use super::Game;
    use crate::game::commands::Target;

    #[test]
    fn tnt_and_falling_blocks_have_no_entity_pick_result() {
        assert!(!Game::has_pick_result(Some(Target::Tnt(10))));
        assert!(!Game::has_pick_result(Some(Target::Falling(20))));
        assert!(!Game::has_pick_result(Some(Target::Projectile(30))));
        assert!(!Game::has_pick_result(Some(Target::DragonPart(40))));
        assert!(Game::has_pick_result(Some(Target::Boat(50))));
        assert!(Game::has_pick_result(Some(Target::Mob(60))));
    }
}
