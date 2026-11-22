//! The running state of `ProjectileUtil.getEntityHitResult`.
use super::Game;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PickTarget {
    Entity(u32),
    DragonPart(usize),
    Crystal(u32),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PickCandidate {
    pub target: PickTarget,
    pub distance: f64,
}

impl PickCandidate {
    /// Wire entity id for `CrosshairTarget::Entity`, falling back to synthetic ids for unnumbered server tests.
    pub(super) fn entity_id(self, game: &Game<'_>) -> u32 {
        game.pick_target_wire_id(self.target)
            .unwrap_or(match self.target {
                PickTarget::Entity(id) | PickTarget::Crystal(id) => id,
                PickTarget::DragonPart(part) => super::dragon::SPECIAL + part as u32,
            })
    }
}

pub(super) struct PickAccumulator {
    pub hovered: Option<PickCandidate>,
    nearest: f64,
    candidates: Option<Vec<PickCandidate>>,
}

impl PickAccumulator {
    pub(super) fn new(reach: f64) -> Self {
        Self {
            hovered: None,
            nearest: reach,
            candidates: None,
        }
    }

    pub(super) fn consider(&mut self, game: &Game<'_>, id: u32, distance: f64) -> bool {
        self.consider_target(game, PickTarget::Entity(id), distance)
    }

    pub(super) fn consider_target(
        &mut self,
        game: &Game<'_>,
        target: PickTarget,
        distance: f64,
    ) -> bool {
        if game.is_pick_camera_target(target) {
            return false;
        }
        // A ray that starts inside the box picks it only when `canBePickedFromInside`.
        if distance == 0.0
            && let PickTarget::Entity(id) = target
            && !game.pickable_from_inside(id)
        {
            return false;
        }
        let candidate = PickCandidate { target, distance };
        if let Some(candidates) = &mut self.candidates {
            candidates.push(candidate);
            return false;
        }
        self.accept(candidate, game.pick_target_shares_vehicle(target), false)
    }

    /// Candidate geometry is copied while the family lists are borrowed; only a
    /// subscribed hook needs this reusable buffer so its Host can borrow Game.
    pub(super) fn recording(reach: f64, mut candidates: Vec<PickCandidate>) -> Self {
        candidates.clear();
        Self {
            candidates: Some(candidates),
            ..Self::new(reach)
        }
    }

    pub(super) fn dispatch(&mut self, game: &mut Game<'_>) -> Vec<PickCandidate> {
        let candidates = self.candidates.take().unwrap_or_default();
        let camera = game.pick_camera_id();
        for &candidate in &candidates {
            if game.is_pick_camera_target(candidate.target) {
                continue;
            }
            let same_vehicle = game.pick_target_shares_vehicle(candidate.target);
            let mut allowed = false;
            if candidate.distance != 0.0
                && (candidate.distance < self.nearest || self.nearest == 0.0)
                && same_vehicle
                && let Some(epoch) = game.native_lifecycle.epoch()
                && let Some(wire_id) = game.pick_target_wire_id(candidate.target)
            {
                let entity = mod_api::EntityRef::new(epoch, wire_id);
                game.ask_can_rider_interact(entity, &mut allowed);
                if game.pick_camera_id() != camera {
                    // The captured ray and vehicle decision no longer belong
                    // to this camera. Refuse the pick, never recast it.
                    self.hovered = None;
                    break;
                }
            }
            self.accept(candidate, same_vehicle, allowed);
        }
        candidates
    }

    fn accept(&mut self, candidate: PickCandidate, same_vehicle: bool, allowed: bool) -> bool {
        let distance = candidate.distance;
        if distance == 0.0 {
            if self.nearest < 0.0 {
                return false;
            }
            self.nearest = 0.0;
        } else if distance < self.nearest || self.nearest == 0.0 {
            if same_vehicle && !allowed {
                if self.nearest != 0.0 {
                    return false;
                }
            } else {
                self.nearest = distance;
            }
        } else {
            return false;
        }
        self.hovered = Some(candidate);
        true
    }
}

impl Game<'_> {
    /// `Entity.canBePickedFromInside`, false only for a sulfur cube holding a body item
    /// (`SulfurCube.canBePickedFromInside`: `!hasBodyItem()`).
    pub(super) fn pickable_from_inside(&self, id: u32) -> bool {
        !self.mobs.iter().any(|m| {
            m.id == id
                && m.species() == sunrise_client::mobs::Species::SulfurCube
                && sunrise_client::mobs::sulfur_cube::has_body(m)
        })
    }

    pub(super) fn pick_camera_id(&self) -> Option<u32> {
        let remote = self.remote.as_ref()?;
        if let Some(id) = remote.camera {
            remote.detached_camera.is_none().then_some(id as u32)
        } else {
            Some(remote.entity_id as u32)
        }
    }

    /// Wire entity id of a pick target, without confusing real ids with synthetic special ranges.
    pub(super) fn pick_target_wire_id(&self, target: PickTarget) -> Option<u32> {
        match target {
            PickTarget::Entity(id) => Some(id),
            PickTarget::DragonPart(part) => self
                .dragon
                .as_ref()
                .filter(|d| d.id != 0)
                .map(|d| super::dragon::dragon_part_wire_id(d.id, part) as u32),
            PickTarget::Crystal(id) => (id != 0).then_some(id),
        }
    }

    /// True if `target` is the live camera entity.
    pub(super) fn is_pick_camera_target(&self, target: PickTarget) -> bool {
        let Some(camera) = self.pick_camera_id() else {
            return false;
        };
        self.pick_target_wire_id(target) == Some(camera)
    }

    fn pick_vehicle_root(&self, id: u32) -> Option<u32> {
        let mut root = id;
        let limit = self
            .remote
            .as_ref()
            .map_or(self.mobs.len(), |r| r.passenger_vehicles.len());
        for _ in 0..=limit + 1 {
            let vehicle =
                if self.pick_camera_id() == Some(root) && self.remote_camera_id().is_none() {
                    self.riding
                } else if let Some(remote) = &self.remote {
                    remote.passenger_vehicle(root)
                } else {
                    self.vehicle_of_any(root)
                };
            let Some(vehicle) = vehicle else {
                return Some(root);
            };
            root = vehicle;
        }
        None
    }

    pub(super) fn pick_target_shares_vehicle(&self, target: PickTarget) -> bool {
        let Some(id) = self.pick_target_wire_id(target) else {
            return false;
        };
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
        use sunrise_client::motion::PlayerMotion;
        use sunrise_core::geometry::Vec3;
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.build_registries();
            game.set_allow_commands(true);
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
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
                .find(|m| m.species() == sunrise_client::mobs::Species::SulfurCube)
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
            let eyes_at = |game: &mut Game<'_>, at: Vec3| {
                game.player = Some(
                    PlayerMotion::new(game.motion, Vec3::new(at.x, at.y - eye, at.z)).unwrap(),
                );
            };
            let picked = |game: &Game<'_>| game.mob_hit().map(|(i, _, _)| game.mobs[i].id);
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
        let mut pick = PickAccumulator::new(5.0);
        assert!(!pick.accept(
            PickCandidate {
                target: PickTarget::Entity(2),
                distance: 1.0
            },
            true,
            false
        ));
        assert!(pick.accept(
            PickCandidate {
                target: PickTarget::Entity(1),
                distance: 0.0
            },
            true,
            false
        ));
        assert!(pick.accept(
            PickCandidate {
                target: PickTarget::Entity(2),
                distance: 3.0
            },
            true,
            false
        ));
        assert_eq!(pick.hovered.unwrap().target, PickTarget::Entity(2));
        assert_eq!(pick.nearest, 0.0);
        assert!(pick.accept(
            PickCandidate {
                target: PickTarget::Entity(3),
                distance: 4.0
            },
            false,
            false
        ));
        assert_eq!(pick.nearest, 4.0);
        assert!(!pick.accept(
            PickCandidate {
                target: PickTarget::Entity(4),
                distance: 4.5
            },
            false,
            false
        ));
    }
}
