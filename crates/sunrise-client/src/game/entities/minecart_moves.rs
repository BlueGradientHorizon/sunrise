//! The cart's `Entity.move` — `AbstractMinecart.move`'s own tick move through
//! the move gate (#2414) — in its own module so `minecarts.rs` keeps its size.
use super::Game;
use super::minecarts::{CartKind, Minecart};
use crate::game::collision_source::Mover;
use sunrise_client::collisions::SceneCollisions;
use sunrise_core::geometry::Vec3;
impl Game<'_> {
    /// Entity.move for a cart: collisions zero the blocked components, the world border's walls
    /// among them (`CollisionGetter.borderCollision`, asked for every entity).
    pub(super) fn move_cart(&mut self, c: &mut Minecart, delta: Vec3) -> bool {
        // The cart's own move is a self move (#2414: vanilla's
        // `AbstractMinecart.move` passes SELF for its own tick); PR 1a
        // records the movement unchanged.
        let mut gate = sunrise_client::move_gate::armed(
            &self.loop_context,
            c.id,
            c.kind.id(),
            sunrise_world::entity_move::MoverType::SelfMovement,
            delta,
        );
        let mut delta = gate.delta();
        // Entity.move: a stuck multiplier scales this move and zeroes the velocity.
        if c.stuck.length_squared() > 1.0e-7 {
            delta = delta.multiply(c.stuck);
            c.stuck = Vec3::ZERO;
            c.velocity = Vec3::ZERO;
        }
        let scene = SceneCollisions {
            world: &self.world,
            binding: self.collision,
            subject: None,
        };
        // A zero move stays zero whatever surrounds it (`Entity.collide`, and the
        // step-up needs a blocked horizontal move): a cart standing on its rails,
        // as most do, gathers no entity boxes.
        let entities = if delta.length_squared() == 0.0 {
            Vec::new()
        } else {
            self.entity_collisions(Mover::Cart(c))
        };
        let mut scene = sunrise_client::collisions::EntityCollisions {
            blocks: scene,
            entities: &entities,
            border: Some((self.border.geometry(), [c.position.x, c.position.z])),
        };
        if let Ok(moved) = sunrise_client::move_gate::entities_resolve(
            &mut gate,
            &mut scene,
            c.bounds(),
            delta,
            0.0,
            c.ground.on_ground,
        ) {
            c.fall_distance = self.vehicle_fall_reset(c.fall_distance, c.position, moved);
            c.position = Vec3::new(
                c.position.x + moved.x,
                c.position.y + moved.y,
                c.position.z + moved.z,
            );
            let side = game_side!(self);
            let motion = sunrise_client::motion::BodyMotion {
                world: &self.world,
                collision: self.collision,
                facts: self.res.facts(),
                blocks: Some(&self.motion.blocks),
                context: &self.loop_context,
                side: Some(&side),
            };
            if !gate.skipped() {
                motion.record_move(
                    c.position,
                    c.bounds(),
                    delta,
                    moved,
                    true,
                    sunrise_client::motion::BodyMotion::entity_context(c.id, c.position),
                    None,
                    &mut c.ground,
                );
            }
            let effect = motion.legacy_contact(c.position, &c.ground);
            // Entity.checkFallDamage: the move's drop counts (not in water, as of
            // the tick before: `isInWater` is `wasTouchingWater`),
            // and a landing hands the whole fall to causeFallDamage, which
            // sets a TNT minecart off at (fall / 10) squared.
            if moved.y < 0.0 && !c.in_water {
                c.fall_distance -= f64::from(moved.y as f32);
            }
            if c.ground.on_ground {
                let distance = c.fall_distance;
                c.fall_distance = 0.0;
                if distance > 0.0 {
                    self.nonliving_fall_on(c.id, c.position, effect);
                    let removed = if c.kind == CartKind::Tnt && distance >= 3.0 {
                        let power = distance / 10.0;
                        self.explode_cart(c, power * power)
                    } else {
                        false
                    };
                    self.post_landing_event(c.position, c.bounds(), false);
                    if removed {
                        return false;
                    }
                }
            }
            let entity = (c.id, c.kind.id());
            let response = self.body_motion(&game_side!(self)).restitute(
                entity,
                sunrise_client::motion::BodyMove {
                    wanted: delta,
                    moved,
                    effect,
                },
                sunrise_client::entity_rules::EntityRules::get_gravity(c),
                0.95_f32,
                c.velocity,
                c.fall_distance,
            );
            c.velocity = self.finish_body_restitution(entity, c.position, response);
        }
        // OldMinecartBehavior applies block effects here and at tick end.
        // NewMinecartBehavior applies them after recording each track step.
        if !self.minecart_improvements() {
            self.cart_block_effects(c);
        }
        true
    }
}
