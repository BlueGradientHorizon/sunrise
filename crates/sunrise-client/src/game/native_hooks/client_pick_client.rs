#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::client_pick::ClientPick;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::client_pick::ClientPickEntityAccess;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::client_pick::ResidentClientPickEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::client_pick::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{Dim, view_vector};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::CrosshairTarget;

impl Game<'_> {
    /// The sole mod-output geometry boundary. Occlusion is intentionally not reapplied.
    pub(in crate::game::client_side) fn accept_client_pick(&self, result: CrosshairTarget) -> bool {
        let (point, reach) = match result {
            CrosshairTarget::Block { hit, .. } => {
                if !matches!(
                    hit.face,
                    [1, 0, 0] | [-1, 0, 0] | [0, 1, 0] | [0, -1, 0] | [0, 0, 1] | [0, 0, -1]
                ) {
                    return false;
                }
                (hit.point, self.block_reach)
            }
            CrosshairTarget::Entity { point, .. } => (point, self.client.client_pick.entity_reach),
            _ => return false,
        };
        point.iter().all(|v| v.is_finite())
            && self.pick_distance(point) <= reach
            && self.pick_target_live(result)
    }
    /// How many picks (a tick's or a frame's) have been taken: an agent command that turned
    /// waits for the count to move before it reads the target.
    pub(in crate::game::client_side) fn client_picks(&self) -> u64 {
        self.client.client_pick.taken
    }
    pub(in crate::game) fn pick_distance(&self, point: [f64; 3]) -> f64 {
        (0..3)
            .map(|i| (point[i] - self.client.client_pick.origin[i]).powi(2))
            .sum::<f64>()
            .sqrt()
    }
    pub(in crate::game::client_side) fn publish_client_pick(
        &mut self,
        result: Option<CrosshairTarget>,
    ) {
        self.client.client_pick.result = result;
        self.client.client_pick.entity_metadata = match result {
            Some(CrosshairTarget::Entity { entity, .. }) => (
                !self.client.interactions.contains_key(&(entity.id() as i32))
                    && Self::has_pick_result(self.native_client_entity_target(entity)),
                self.resident_dragon_part(entity.id()).is_none(),
            ),
            _ => (false, false),
        };
    }
    pub(in crate::game) fn refresh_client_pick(&mut self, partial_tick: f32) {
        if !self.native_window()
            || self.title.0.is_some()
            || self.player.is_none()
            || self.native_lifecycle.epoch().is_none()
        {
            return;
        }
        let Some(camera) = self
            .spectated_camera_snapshot()
            .or_else(|| self.player_camera_entity())
        else {
            return;
        };
        let origin = std::array::from_fn(|i| {
            camera.previous[i]
                + f64::from(partial_tick) * (camera.position[i] - camera.previous[i])
                + if i == 1 {
                    f64::from(camera.eye_height)
                } else {
                    0.0
                }
        });
        let direction = view_vector(
            camera.view_y_rot(partial_tick),
            camera.view_x_rot(partial_tick),
        );
        self.client.client_pick.ray = Some((origin, direction));
        self.client.client_pick.origin = origin;
        self.client.client_pick.epoch = self.native_lifecycle.epoch();
        self.client.client_pick.dimension = Some(self.dim);
        let from = sunrise_core::geometry::Vec3::new(origin[0], origin[1], origin[2]);
        let max_distance = self.block_reach.max(self.entity_reach);
        let block_hit = self.pick_unfiltered();
        // LocalPlayer.pick: on a block hit, blockDistanceSq shortens both the ray and search bound;
        // on a block miss, search uses nominal maxDistance^2 while final comparison uses to.distanceToSqr(from).
        let (entity_max_distance, search_max_distance_sq, block_distance_sq) = match block_hit {
            Some(hit) => {
                let d2 = from.distance_squared(sunrise_core::geometry::Vec3::new(
                    hit.point[0],
                    hit.point[1],
                    hit.point[2],
                ));
                (d2.sqrt(), d2, d2)
            }
            None => {
                let to = sunrise_core::geometry::Vec3::new(
                    from.x + direction[0] * max_distance,
                    from.y + direction[1] * max_distance,
                    from.z + direction[2] * max_distance,
                );
                (
                    max_distance,
                    max_distance * max_distance,
                    to.distance_squared(from),
                )
            }
        };
        let block_candidate = block_hit
            .filter(|hit| {
                from.distance_squared(sunrise_core::geometry::Vec3::new(
                    hit.point[0],
                    hit.point[1],
                    hit.point[2],
                )) < self.block_reach * self.block_reach
            })
            .and_then(|hit| self.crosshair_block_target(hit));
        let camera_id = self.pick_camera_id();
        let attack_result = self
            .active_attack_range()
            .map(|range| self.attack_range_pick_result(&range));
        let mut entity_in_range = None;
        let original = match attack_result {
            Some(crate::game::pick_entity::AttackRangePickResult::Entity(hit)) => self
                .native_lifecycle
                .epoch()
                .map(|epoch| CrosshairTarget::Entity {
                    entity: EntityRef::new(epoch, hit.id),
                    point: hit.point,
                }),
            Some(crate::game::pick_entity::AttackRangePickResult::Block(hit)) => {
                self.crosshair_block_target(hit)
            }
            None | Some(crate::game::pick_entity::AttackRangePickResult::Miss) => {
                let raw_entity_hit =
                    self.pick_entity_candidate_within(entity_max_distance, search_max_distance_sq);
                // Refuse the pick if a rider listener changed camera, epoch, or dimension.
                if camera_id != self.pick_camera_id()
                    || self.client.client_pick.epoch != self.native_lifecycle.epoch()
                    || self.client.client_pick.dimension != Some(self.dim)
                {
                    self.client.client_pick.ray = None;
                    self.publish_client_pick(None);
                    return;
                }
                entity_in_range = raw_entity_hit.filter(|target| match target {
                    CrosshairTarget::Entity { point, .. } => {
                        from.distance_squared(sunrise_core::geometry::Vec3::new(
                            point[0], point[1], point[2],
                        )) < self.entity_reach * self.entity_reach
                    }
                    _ => false,
                });
                match raw_entity_hit {
                    Some(CrosshairTarget::Entity { point, .. })
                        if from.distance_squared(sunrise_core::geometry::Vec3::new(
                            point[0], point[1], point[2],
                        )) < block_distance_sq =>
                    {
                        entity_in_range
                    }
                    _ => block_candidate,
                }
            }
        };
        self.publish_client_pick(original);
        self.client.client_pick.taken += 1;
        if !mod_api::__private::listened(mod_api::HookId::ClientPick) {
            self.client.client_pick.ray = None;
            return;
        }
        // Supply block and entity candidates independently only when a ClientPick listener is active.
        let entity_candidate = if matches!(
            attack_result,
            Some(crate::game::pick_entity::AttackRangePickResult::Entity(_))
        ) {
            original
        } else if entity_in_range.is_some() {
            entity_in_range
        } else if block_hit.is_some()
            || matches!(
                attack_result,
                Some(crate::game::pick_entity::AttackRangePickResult::Block(_))
            )
        {
            let independent = self.pick_entity_candidate_within(
                self.entity_reach,
                self.entity_reach * self.entity_reach,
            );
            if camera_id != self.pick_camera_id()
                || self.client.client_pick.epoch != self.native_lifecycle.epoch()
                || self.client.client_pick.dimension != Some(self.dim)
            {
                self.client.client_pick.ray = None;
                self.publish_client_pick(None);
                return;
            }
            independent.filter(|target| match target {
                CrosshairTarget::Entity { point, .. } => {
                    from.distance_squared(sunrise_core::geometry::Vec3::new(
                        point[0], point[1], point[2],
                    )) < self.entity_reach * self.entity_reach
                }
                _ => false,
            })
        } else {
            None
        };
        self.client.client_pick.ray = None;
        // Capture the same forward-movement envelope as the vanilla item ray,
        // before listeners can change the player or held item.
        self.client.client_pick.entity_reach = self.client_pick_entity_reach(direction);
        self.storage_scope(|game, _| {
            game.client.client_pick.dispatching = true;
            let mut result = original;
            game.native_client_pick(mod_api::__private::ClientPickCall {
                partial_tick,
                origin,
                original,
                block_candidate,
                entity_candidate,
                result: &mut result,
            });
            game.client.client_pick.dispatching = false;
            let valid_context = game.client.client_pick.epoch == game.native_lifecycle.epoch()
                && game.client.client_pick.dimension == Some(game.dim);
            let result = result.filter(|&result| {
                valid_context && (Some(result) == original || game.accept_client_pick(result))
            });
            game.publish_client_pick(result);
            ((), true)
        });
    }
    /// Consumers read the stored hit as-is until the next pick. In particular,
    /// a block broken earlier in a tick is still that tick's use target.
    pub(in crate::game) fn stored_client_pick(&self) -> Option<CrosshairTarget> {
        if self.client.client_pick.dispatching {
            return self.client.client_pick.result;
        }
        (self.client.client_pick.epoch == self.native_lifecycle.epoch()
            && self.client.client_pick.dimension == Some(self.dim))
        .then_some(())?;
        self.client.client_pick.result
    }
    pub(in crate::game) fn uses_stored_pick(&self) -> bool {
        self.native_window() && self.client.client_pick.ray.is_none()
    }
}

#[cfg(test)]
#[path = "client_pick/tests_client.rs"]
pub(in crate::game) mod tests;
