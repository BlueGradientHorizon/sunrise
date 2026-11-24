//! The directional push of `PistonMovingBlockEntity.moveCollidedEntities` for mod colliders:
//! the moving boxes meet a body along their sweep, then the deepest advance sets its push.
//! A turn, alone or with a translation, resolves the final pose instead: there is no sweep
//! of the boxes' rotation.
use super::*;
use sunrise_client_render::moving_colliders::{Placed, Shove};

#[cfg(test)]
#[path = "shoving_tests.rs"]
mod tests;

impl Game<'_> {
    /// Shoves the bodies a moving mod collider meets. On a logical server its targets are
    /// entities and the local player is never moved here; on a client the target is its local
    /// player, through the ordinary predicted move. Bodies carried by the collider are not
    /// shoved again.
    pub(super) fn shove_pass(&mut self, start: &Prepared, steps: &StepHeights, riders: &[Rider]) {
        let end = self.placed_moving_colliders();
        let starts: sunrise_client::fast_hash::FxHashMap<u32, &Placed> =
            start.placed().map(|p| (p.id, p)).collect();
        let mut sweeps: Vec<_> = end
            .placed()
            .filter_map(|after| {
                let before = &starts.get(&after.id)?.collider;
                let delta = after.collider.frame.origin.subtract(before.frame.origin);
                let turned = !before.frame.same_turn(&after.collider.frame);
                if delta == Vec3::ZERO && !turned {
                    return None;
                }
                // A turn is resolved at its final pose. The broad phase holds both poses, but
                // no rotational path is inferred between them.
                let swept = if turned {
                    before
                        .world_bounds()?
                        .union_bounds(after.collider.world_bounds()?)
                } else {
                    before.world_bounds()?.expand_towards(delta)
                };
                Some((after, before, delta, swept, turned))
            })
            .collect();
        if sweeps.is_empty() {
            return;
        }
        sweeps.sort_unstable_by_key(|(after, ..)| after.id);

        let server = self.is_logical_server();
        let targets: Vec<_> = self
            .push_targets()
            .filter(|target| match target {
                MoveWho::Entity(_) => server,
                MoveWho::Player => !server,
                MoveWho::Guest(_) => false,
            })
            .filter_map(|target| self.stable_push_target(target))
            .collect();
        let mut current = end.clone();
        let mut stand_grid = None;
        let mut shoved = false;
        for (after, before, delta, swept, turned) in sweeps {
            let source = after.id;
            let first = riders.partition_point(|rider| rider.support < source);
            let supported = &riders[first..];
            for &selected in &targets {
                if after.exempt.contains(&selected.id)
                    || supported
                        .iter()
                        .take_while(|r| r.support == source)
                        .any(|r| r.id == selected.id)
                {
                    continue;
                }
                let Some(target) = self.resolve_push_target(selected) else {
                    continue;
                };
                let Some(body) = self.push_body(target, None, steps) else {
                    continue;
                };
                if !swept.intersects(body.bounds) {
                    continue;
                }
                let (_, no_physics, rides) = self.carry_facts(target);
                if no_physics || rides {
                    continue;
                }
                let movement = if turned {
                    let Some(separation) = after
                        .collider
                        .minimum_separation(body.bounds, piston_push::PUSH_OFFSET)
                    else {
                        continue;
                    };
                    separation
                } else {
                    let fraction =
                        before.translation_push_fraction(&after.collider, body.bounds, delta);
                    if fraction <= 0.0 {
                        continue;
                    }
                    let direction = delta.scale(1.0 / delta.length());
                    // The piston's clearance is added once to its largest box advance.
                    delta
                        .scale(fraction)
                        .add_vector(direction.scale(piston_push::PUSH_OFFSET))
                };
                {
                    let _scope = moving_colliders::Scope::enter_level(
                        self.collider_level(),
                        current.clone(),
                    );
                    let _shove = Shove::enter(selected.id, source);
                    self.move_pushed_body(
                        MoverType::Carried,
                        target,
                        body,
                        movement,
                        false,
                        &mut stand_grid,
                    );
                }
                shoved = true;
                let Some(target) = self.resolve_push_target(selected) else {
                    continue;
                };
                let moved = match target {
                    MoveWho::Player => self
                        .player
                        .as_ref()
                        .map_or(Vec3::ZERO, |player| player.position),
                    MoveWho::Entity(handle) => {
                        with_entity!(self, handle, entity => entity.position())
                    }
                    MoveWho::Guest(_) => Vec3::ZERO,
                }
                .subtract(body.position);
                if moved != Vec3::ZERO {
                    let normal = movement.normalize();
                    let inward = match target {
                        MoveWho::Player => self
                            .player
                            .as_ref()
                            .map_or(0.0, |player| player.velocity.dot(normal)),
                        MoveWho::Entity(handle) => with_entity!(self, handle, entity =>
                            entity.velocity().dot(normal)
                        ),
                        MoveWho::Guest(_) => 0.0,
                    };
                    if inward < 0.0 {
                        match target {
                            MoveWho::Player => {
                                if let Some(player) = self.player.as_mut() {
                                    player.velocity = cancel_shove_velocity(
                                        player.velocity,
                                        normal,
                                        inward,
                                        movement.y,
                                        delta.y,
                                    );
                                }
                            }
                            MoveWho::Entity(handle) => {
                                with_entity_mut!(self, handle, entity => {
                                    let velocity = cancel_shove_velocity(
                                        *entity.velocity_mut(),
                                        normal,
                                        inward,
                                        movement.y,
                                        delta.y,
                                    );
                                    *entity.velocity_mut() = velocity;
                                });
                            }
                            MoveWho::Guest(_) => {}
                        }
                    }
                }
                if current.placed().any(|p| p.exempt.contains(&selected.id)) {
                    // This body owns a collider, or is the vehicle of a holder. Its
                    // new pose is an obstacle to later shoves, not another sweep.
                    self.position_riders();
                    current = self.placed_moving_colliders();
                }
            }
        }
        if shoved {
            self.position_riders();
        }
    }
}

/// Removes the part of `velocity` that points into a shove along `normal`. An upward shove
/// leaves the collider's own vertical speed instead, as Create's moving colliders do.
fn cancel_shove_velocity(
    velocity: Vec3,
    normal: Vec3,
    inward: f64,
    upward: f64,
    collider_rise: f64,
) -> Vec3 {
    let mut velocity = velocity.subtract(normal.scale(inward));
    if upward > 0.0 {
        velocity.y = collider_rise;
    }
    velocity
}
