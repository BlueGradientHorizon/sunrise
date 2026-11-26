//! The running state of `ProjectileUtil.getEntityHitResult`.
use super::Game;

pub(in crate::game) use crate::game::pick_ray::{PickAccumulator, PickCandidate};

impl PickAccumulator {
    /// Reuses the caller's candidate buffer when `CanRiderInteract` is subscribed.
    pub(in crate::game::client_side) fn recording(
        max_distance_sq: f64,
        mut candidates: Vec<PickCandidate>,
    ) -> Self {
        candidates.clear();
        Self::recording_with(max_distance_sq, candidates)
    }

    pub(in crate::game::client_side) fn dispatch(
        &mut self,
        game: &mut Game<'_, crate::game::client_side::ClientState>,
    ) -> Vec<PickCandidate> {
        let candidates = self.take_candidates();
        let camera = game.pick_camera_id();
        for &candidate in &candidates {
            if game.is_pick_camera(candidate.id) {
                continue;
            }
            let same_vehicle = game.pick_shares_vehicle(candidate.id);
            let mut allowed = false;
            if !candidate.inside
                && (candidate.distance_sq < self.nearest_sq() || self.nearest_sq() == 0.0)
                && same_vehicle
                && let Some(epoch) = game.native_lifecycle.epoch()
                && let Some(wire_id) = game.wire_entity_id(candidate.id)
            {
                let entity = mod_api::EntityRef::new(epoch, wire_id as u32);
                game.ask_can_rider_interact(entity, &mut allowed);
                if game.pick_camera_id() != camera {
                    // Camera changed during hook; refuse the pick rather than recasting it.
                    self.hovered = None;
                    break;
                }
            }
            self.accept(candidate, same_vehicle, allowed);
        }
        candidates
    }
}

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn pick_camera_id(&self) -> Option<u32> {
        if let Some(remote) = self.remote.as_ref() {
            if let Some(id) = remote.camera {
                remote.detached_camera.is_none().then_some(id as u32)
            } else {
                Some(remote.entity_id as u32)
            }
        } else {
            self.player
                .as_ref()
                .map(|_| self.own_entity_id() as u32)
                .filter(|&id| id != 0)
        }
    }

    /// True if `id` (or the wire entity behind a synthetic special-target id) is the camera entity.
    pub(in crate::game::client_side) fn is_pick_camera(&self, id: u32) -> bool {
        let Some(camera) = self.pick_camera_id() else {
            return false;
        };
        id == camera || self.wire_entity_id(id) == Some(camera as i32)
    }

    #[inline(always)]
    pub(in crate::game) fn client_pick_camera_id(&self) -> Option<u32> {
        self.pick_camera_id()
    }

    #[inline(always)]
    pub(in crate::game) fn client_is_pick_camera(&self, id: u32) -> bool {
        self.is_pick_camera(id)
    }

    fn pick_vehicle_step(&self, entity: u32) -> Option<u32> {
        if self.pick_camera_id() == Some(entity) && self.remote_camera_id().is_none() {
            return self.riding.or_else(|| {
                self.remote
                    .as_ref()
                    .and_then(|r| r.passenger_vehicle(entity))
            });
        }
        if let Some(remote) = &self.remote {
            return remote.passenger_vehicle(entity);
        }
        None
    }

    /// Walks the full vehicle chain (including retained off-level entities) with cycle detection.
    fn pick_vehicle_root(&self, id: u32) -> Option<u32> {
        let start = self.wire_entity_id(id).map_or(id, |wire| wire as u32);
        let mut slow = start;
        let mut fast = start;
        loop {
            let Some(next) = self.pick_vehicle_step(fast) else {
                return Some(fast);
            };
            let Some(next2) = self.pick_vehicle_step(next) else {
                return Some(next);
            };
            fast = next2;
            slow = self.pick_vehicle_step(slow)?;
            if slow == fast {
                return None;
            }
        }
    }

    pub(in crate::game::client_side) fn pick_shares_vehicle(&self, id: u32) -> bool {
        let camera_root = self
            .pick_camera_id()
            .and_then(|camera| self.pick_vehicle_root(camera))
            .or_else(|| {
                self.controls_camera()
                    .then(|| self.riding.and_then(|v| self.pick_vehicle_root(v)))
                    .flatten()
            });
        camera_root.is_some() && self.pick_vehicle_root(id) == camera_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ProjectileUtil.getEntityHitResult` with `SulfurCube.canBePickedFromInside`: from inside its
    /// box, a cube is picked without a body item and not with one; from outside, it is picked
    /// either way.
    #[test]
    #[ignore = "daily"]
    fn a_sulfur_cube_holding_a_block_is_not_picked_from_inside() {
        use sunrise_core::geometry::Vec3;
        use sunrise_sim::motion::PlayerMotion;
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.build_registries();
            game.set_allow_commands(true);
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            // A monster can't be summoned on peaceful.
            assert!(game.run_command("difficulty normal"));
            assert!(game.run_command("forceload add 0 0"));
            assert!(game.run_command(
                "summon minecraft:sulfur_cube 4.5 200 4.5 {NoAI:1b,NoGravity:1b,Silent:1b,PersistenceRequired:1b,Size:1}"
            ));
            let cube = game
                .mobs
                .iter()
                .find(|m| m.species() == sunrise_sim::mobs::Species::SulfurCube)
                .expect("the cube");
            let (id, bounds) = (cube.id, cube.bounds());
            let (min, max) = (bounds.min_position(), bounds.max_position());
            let centre = Vec3::new(
                (min.x + max.x) / 2.0,
                (min.y + max.y) / 2.0,
                (min.z + max.z) / 2.0,
            );
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(0.0, 64.0, 0.0)).unwrap());
            let eye = game.aim().unwrap().0[1] - 64.0;
            // Yaw 0 looks along +z.
            (game.yaw, game.pitch) = (0.0, 0.0);
            let eyes_at = |game: &mut crate::game::ServerGame<'_>, at: Vec3| {
                game.player = Some(
                    PlayerMotion::new(game.motion, Vec3::new(at.x, at.y - eye, at.z)).unwrap(),
                );
            };
            let picked = |game: &crate::game::ServerGame<'_>| {
                game.mob_hit().map(|(i, _, _)| game.mobs[i].id)
            };
            eyes_at(game, centre);
            assert_eq!(
                picked(game),
                Some(id),
                "an empty cube is picked from inside"
            );
            let cube = game.mobs.iter_mut().find(|m| m.id == id).unwrap();
            cube.extra.set_body(Some("minecraft:oak_planks".into()));
            assert_eq!(picked(game), None, "a cube holding a block is not");
            eyes_at(game, Vec3::new(centre.x, centre.y, min.z - 1.0));
            assert_eq!(picked(game), Some(id), "from outside it still is");
        });
    }

    #[test]
    fn same_vehicle_uses_the_running_nearest_not_the_candidates_distance() {
        let mut pick = PickAccumulator::new(25.0);
        assert!(!pick.accept(
            PickCandidate {
                id: 2,
                point: [0.0; 3],
                distance_sq: 1.0,
                inside: false,
            },
            true,
            false
        ));
        assert!(pick.accept(
            PickCandidate {
                id: 1,
                point: [0.0; 3],
                distance_sq: 0.0,
                inside: true,
            },
            true,
            false
        ));
        assert!(pick.accept(
            PickCandidate {
                id: 2,
                point: [0.0; 3],
                distance_sq: 9.0,
                inside: false,
            },
            true,
            false
        ));
        assert_eq!(pick.hovered.unwrap().id, 2);
        assert_eq!(pick.nearest_sq(), 0.0);
        assert!(pick.accept(
            PickCandidate {
                id: 3,
                point: [0.0; 3],
                distance_sq: 16.0,
                inside: false,
            },
            false,
            false
        ));
        assert_eq!(pick.nearest_sq(), 16.0);
        assert!(!pick.accept(
            PickCandidate {
                id: 4,
                point: [0.0; 3],
                distance_sq: 20.25,
                inside: false,
            },
            false,
            false
        ));
    }
}
