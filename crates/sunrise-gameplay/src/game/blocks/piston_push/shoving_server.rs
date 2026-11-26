//! Server-side directional push of mod moving colliders (`MoveWho::Entity(_)` only).
use super::move_gate::MoveWho;
use sunrise_gameplay_common::game::piston_push::{StepHeights, carrying::Rider};
use sunrise_core::geometry::Vec3;
use sunrise_world::moving_colliders::{self, Placed, Prepared, Shove};
use sunrise_world::{entity_move::MoverType, piston_push};

impl crate::game::ServerGame<'_> {
    pub(in crate::game::server_side) fn server_shove_pass(
        &mut self,
        start: &Prepared,
        steps: &StepHeights,
        riders: &[Rider],
    ) {
        let end = self.placed_moving_colliders();
        let starts: sunrise_level::fast_hash::FxHashMap<u32, &Placed> =
            start.placed().map(|p| (p.id, p)).collect();
        let mut sweeps: Vec<_> = end
            .placed()
            .filter_map(|after| {
                let before = &starts.get(&after.id)?.collider;
                let delta = after.collider.frame.origin.subtract(before.frame.origin);
                if delta == Vec3::ZERO || !before.frame.same_turn(&after.collider.frame) {
                    return None;
                }
                let swept = before.world_bounds()?.expand_towards(delta);
                Some((after, before, delta, swept))
            })
            .collect();
        if sweeps.is_empty() {
            return;
        }
        sweeps.sort_unstable_by_key(|(after, ..)| after.id);

        let targets: Vec<_> = self
            .push_targets()
            .filter(|target| matches!(target, MoveWho::Entity(_)))
            .filter_map(|target| self.stable_push_target(target))
            .collect();
        let mut current = end.clone();
        let mut stand_grid = None;
        let mut shoved = false;
        for (after, before, delta, swept) in sweeps {
            let source = after.id;
            let first = riders.partition_point(|rider| rider.support < source);
            let supported = &riders[first..];
            let direction = delta.scale(1.0 / delta.length());
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
                let (_, no_physics, rides) = self.server_carry_facts(target);
                if no_physics || rides {
                    continue;
                }
                let fraction =
                    before.translation_push_fraction(&after.collider, body.bounds, delta);
                if fraction > 0.0 {
                    let movement = delta
                        .scale(fraction)
                        .add_vector(direction.scale(piston_push::PUSH_OFFSET));
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
                    if current.placed().any(|p| p.exempt.contains(&selected.id)) {
                        // This body owns a collider, or is the vehicle of a holder. Its
                        // new pose is an obstacle to later shoves, not another sweep.
                        self.position_riders();
                        current = self.placed_moving_colliders();
                    }
                }
            }
        }
        if shoved {
            self.position_riders();
        }
    }
}

use sunrise_gameplay_common::game::passengers_common::PassengersCommonGameExt1 as _;
