//! Server-side moving-collider carrying pass (`MoveWho::Entity(_)` only, preserving
//! stacked collider forest depth ordering and `position_riders` synchronization).
use super::{
    Game,
    entity::{Entity, with_entity, with_entity_mut},
    move_gate::MoveWho,
};
use crate::game::piston_push::{
    StepHeights,
    carrying::{Rider, support_of},
};
use sunrise_client_render::moving_colliders::{self, Prepared};
use sunrise_core::geometry::{Axis, Vec3};
use sunrise_world::entity_move::MoverType;

/// What a server carry pass works on, kept from tick to tick so a pass allocates nothing once
/// these have grown to the level's size.
#[derive(Default)]
struct Scratch {
    riders: Vec<Rider>,
    stacked: Vec<(u32, u32)>,
    order: Vec<(u32, u32)>,
    ids: Vec<u32>,
}

thread_local! {
    static SCRATCH: std::cell::RefCell<Scratch> = std::cell::RefCell::new(Scratch::default());
}

impl Game<'_> {
    /// Server arm of [`Game::side_carry_pass`]: carries and shoves server-simulated
    /// entities (`MoveWho::Entity(_)`, excluding players and guests).
    pub(in crate::game) fn server_carry_pass(&mut self, start: &Prepared) {
        if !self.server_any_collider_moved(start) {
            return;
        }
        let mut scratch = SCRATCH.with(|cell| std::mem::take(&mut *cell.borrow_mut()));
        let steps = self.push_step_heights();
        scratch.riders.clear();
        self.server_collider_riders(start, &steps, &mut scratch);
        if !scratch.riders.is_empty() {
            self.server_carry_in_order(start, &steps, &mut scratch);
            // A passenger that owns a collider moves with its vehicle too.
            self.position_riders();
        }
        self.server_shove_pass(start, &steps, &scratch.riders);
        SCRATCH.with(|cell| *cell.borrow_mut() = scratch);
    }

    fn server_any_collider_moved(&self, start: &Prepared) -> bool {
        start.placed().any(|p| {
            self.collider_now_frame(p.id)
                .is_some_and(|now| now != p.collider.frame)
        })
    }

    fn server_carry_in_order(
        &mut self,
        start: &Prepared,
        steps: &StepHeights,
        scratch: &mut Scratch,
    ) {
        scratch.order.clear();
        let mut at = 0;
        while at < scratch.riders.len() {
            let collider = scratch.riders[at].support;
            let mut depth = 0;
            let mut up = collider;
            while let Ok(i) = scratch.stacked.binary_search_by_key(&up, |&(id, _)| id) {
                up = scratch.stacked[i].1;
                depth += 1;
                if depth > start.len() as u32 || up == collider {
                    break;
                }
            }
            scratch.order.push((depth, collider));
            while at < scratch.riders.len() && scratch.riders[at].support == collider {
                at += 1;
            }
        }
        scratch.order.sort_unstable();
        let mut stand_grid = None;
        let mut wave = 0;
        let mut at = 0;
        while at < scratch.order.len() {
            wave = wave.max(scratch.order[at].0);
            let now = self.placed_moving_colliders();
            {
                let _scope = moving_colliders::Scope::enter_level(self.collider_level(), now);
                while at < scratch.order.len() && scratch.order[at].0 <= wave {
                    let collider = scratch.order[at].1;
                    let first = scratch.riders.partition_point(|r| r.support < collider);
                    let count = scratch.riders[first..]
                        .iter()
                        .take_while(|r| r.support == collider)
                        .count();
                    self.server_carry_riders_of(
                        collider,
                        start,
                        steps,
                        &scratch.riders[first..first + count],
                        &mut stand_grid,
                    );
                    at += 1;
                }
            }
            wave += 1;
        }
    }

    fn server_collider_riders(
        &mut self,
        start: &Prepared,
        steps: &StepHeights,
        scratch: &mut Scratch,
    ) {
        scratch.riders.clear();
        scratch.stacked.clear();
        scratch.ids.clear();
        scratch.ids.extend(start.placed().map(|p| p.id));
        scratch.ids.sort_unstable();
        for target in self.push_targets() {
            if !matches!(target, MoveWho::Entity(_)) {
                continue;
            }
            let Some(body) = self.push_body(target, None, steps) else {
                continue;
            };
            if !body.on_ground {
                continue;
            }
            let (id, no_physics, rides) = self.server_carry_facts(target);
            if no_physics || rides {
                continue;
            }
            if let Some(support) = support_of(start, body.bounds, id) {
                scratch.riders.push(Rider {
                    target,
                    id,
                    support,
                });
            }
        }
        scratch.riders.sort_unstable_by_key(|r| r.support);
        for rider in &scratch.riders {
            if scratch.ids.binary_search(&rider.id).is_ok() {
                scratch.stacked.push((rider.id, rider.support));
            }
        }
        scratch.stacked.sort_unstable();
    }

    /// The entity id of a server-carried body, whether it has no physics, and whether it rides.
    pub(in crate::game::server_side) fn server_carry_facts(
        &self,
        target: MoveWho,
    ) -> (u32, bool, bool) {
        match target {
            MoveWho::Entity(handle) => {
                let (id, no_physics) = with_entity!(self, handle, entity => {
                    let stored = self.persist.no_physics.get(&entity.id()).copied();
                    (entity.id(), entity.no_physics(stored))
                });
                (id, no_physics, self.push_root(target) != Some(id))
            }
            MoveWho::Player | MoveWho::Guest(_) => (0, true, false),
        }
    }

    fn server_carry_riders_of(
        &mut self,
        collider: u32,
        start: &Prepared,
        steps: &StepHeights,
        riders: &[Rider],
        stand_grid: &mut Option<sunrise_client::entities::ObstacleGrid>,
    ) {
        let Some(before) = start.placed().find(|p| p.id == collider) else {
            return;
        };
        let Some(after) = self.collider_now_frame(collider) else {
            return;
        };
        for rider in riders {
            let Some(body) = self.push_body(rider.target, None, steps) else {
                continue;
            };
            let centre = body.bounds.center();
            let contact = Vec3::new(centre.x, body.bounds.min(Axis::Y), centre.z);
            let carried = after
                .to_world(before.collider.frame.to_local(contact))
                .subtract(contact);
            if carried.length_squared() < 1.0e-18 {
                continue;
            }
            self.move_pushed_body(
                MoverType::Carried,
                rider.target,
                body,
                carried,
                false,
                stand_grid,
            );
            // Keep the entity grounded on its carrier for the next tick's support check.
            if let MoveWho::Entity(handle) = rider.target {
                with_entity_mut!(self, handle, entity => entity.set_piston_on_ground(true));
            }
        }
    }
}
