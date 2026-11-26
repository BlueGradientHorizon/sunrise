//! Carrying: role-neutral moving-collider support lookup and dispatcher.
//! Server carry/shove passes live in `carrying_server.rs` / `shoving_server.rs`;
//! client carry/shove prediction lives in `piston_push_client.rs`.
#[path = "shoving.rs"]
mod shoving;
use super::*;
use sunrise_core::geometry::Axis;
#[allow(unused_imports)]
pub(in crate::game) use sunrise_world::moving_colliders::{self, Prepared};

/// How far under its feet a body may have a box and still stand on it.
pub(in crate::game) const SUPPORT_PROBE: f64 = 0.01;

/// A body standing on a collider.
pub struct Rider {
    pub(in crate::game) target: MoveWho,
    pub(in crate::game) id: u32,
    /// The collider entity it stands on.
    pub(in crate::game) support: u32,
}

/// The collider a body with `bounds` and entity `id` stands on: the one box under its feet with
/// the highest top (a tie to the lower collider entity id), found through the level's index.
pub fn support_of(start: &Prepared, bounds: Aabb, id: u32) -> Option<u32> {
    let mut best: Option<(f64, u32)> = None;
    let probe = Aabb::new(
        bounds
            .min_position()
            .subtract(Vec3::new(0.0, SUPPORT_PROBE, 0.0)),
        bounds.max_position(),
    );
    start.meeting(probe, |placed| {
        if placed.exempt.contains(&id) {
            return;
        }
        let allowed = placed.collider.collide(bounds, Axis::Y, -SUPPORT_PROBE);
        if allowed <= -SUPPORT_PROBE {
            return;
        }
        let gap = -allowed;
        let better =
            best.is_none_or(|(g, c)| gap < g - 1.0e-9 || (gap <= g + 1.0e-9 && placed.id < c));
        if better {
            best = Some((gap, placed.id));
        }
    });
    best.map(|(_, support)| support)
}

impl crate::game::CommonGame<'_> {
    /// Carries bodies on colliders that moved this tick, then shoves bodies in the path of a
    /// translating collider. `start` is the colliders as the tick began. Dispatched by role.
    pub fn carry_pass(&mut self, start: &Prepared) {
        self.side_carry_pass(start);
    }
}

use crate::game::side_ops::blocks::BlocksSideGameExt as _;
