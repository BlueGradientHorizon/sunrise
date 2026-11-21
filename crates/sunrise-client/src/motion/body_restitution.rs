//! The restitution phase of `Entity.move` for bodies without a full movement provider.
use crate::{
    block_contact::{self, Contact},
    blocks::BlockFacts,
    collisions::{PowderSnowSubject, SceneCollisions},
    motion::{FrictionLevel, SideLevel, block_support_at},
    world::World,
};
use std::convert::Infallible;
use sunrise_core::{
    geometry::{Aabb, Vec3},
    position::BlockPos,
};
use sunrise_world::{
    block_collision::BlockCollisionRegistry,
    collision_context::CollisionContext,
    entity_move::different,
    ground_support::{GroundState, find_supporting_block},
    movement_properties::BoundMovementProperties,
    movement_restitution::{RestitutionInput, RestitutionWorld, restitute_movement},
    movement_support::{LEGACY_SUPPORT_OFFSET, MOVEMENT_SUPPORT_OFFSET},
    native_load_context::LoopContext,
    registry::BlockStateId,
};

/// The level data used by an entity that does not keep a movement world of its own.
#[derive(Clone, Copy)]
pub struct BodyMotion<'a> {
    pub world: &'a World,
    pub collision: &'a BlockCollisionRegistry<'a>,
    pub facts: &'a BlockFacts,
    pub blocks: Option<&'a BoundMovementProperties>,
    pub context: &'a LoopContext,
    /// Standalone simulation tests do not have a mod level host.
    pub side: Option<&'a dyn SideLevel>,
}

/// The legacy effect pair captured from the post-move footprint, and its supporting block.
#[derive(Clone, Copy)]
pub struct BodyContact {
    pub position: BlockPos,
    pub state: BlockStateId,
    feet: Vec3,
    support: Option<BlockPos>,
}

impl BodyMotion<'_> {
    /// `Entity.setOnGroundWithMovement`, then capture the one legacy position/state pair.
    /// Use the moved footprint, including the original horizontal footprint on a support miss.
    pub fn effect_block(
        self,
        position: Vec3,
        bounds: Aabb,
        on_ground: bool,
        moved: Vec3,
        context: CollisionContext<'_>,
        subject: Option<PowderSnowSubject<'_>>,
    ) -> BodyContact {
        let mut ground = GroundState {
            on_ground,
            ..GroundState::default()
        };
        self.update_support(&mut ground, position, bounds, Some(moved), context, subject);
        self.legacy_contact(position, &ground)
    }

    /// Record the collision flags and support selected at the end of `Entity.move`.
    #[allow(clippy::too_many_arguments)]
    pub fn record_move(
        self,
        feet: Vec3,
        bounds: Aabb,
        wanted: Vec3,
        moved: Vec3,
        authoritative: bool,
        context: CollisionContext<'_>,
        subject: Option<PowderSnowSubject<'_>>,
        ground: &mut GroundState,
    ) {
        ground.horizontal_collision = different(wanted.x, moved.x) || different(wanted.z, moved.z);
        if authoritative || wanted.y.abs() > 0.0 {
            ground.on_ground = wanted.y < 0.0 && wanted.y != moved.y;
            self.update_support(ground, feet, bounds, Some(moved), context, subject);
        }
    }

    /// Update the moving body's retained support using the same shapes as its move.
    pub fn update_support(
        self,
        ground: &mut GroundState,
        feet: Vec3,
        bounds: Aabb,
        movement: Option<Vec3>,
        context: CollisionContext<'_>,
        subject: Option<PowderSnowSubject<'_>>,
    ) {
        let mut scene = SceneCollisions {
            world: self.world,
            binding: self.collision,
            subject,
        };
        let _ = ground.check_support(ground.on_ground, bounds, movement, |bounds| {
            find_supporting_block(&mut scene, context, feet, bounds, 1_000_000)
        });
    }

    /// Read the legacy effect pair from the retained support, with no new collision scan.
    pub fn legacy_contact(self, feet: Vec3, ground: &GroundState) -> BodyContact {
        let support = ground.supporting_block;
        let position = block_support_at(
            self.world,
            self.blocks,
            feet,
            support,
            LEGACY_SUPPORT_OFFSET,
        );
        BodyContact {
            position,
            state: self.world.block(position.x, position.y, position.z),
            feet,
            support,
        }
    }

    /// The `canSimulateMovement` decision belongs to the caller. Returns whether vanilla
    /// marked the position for immediate sync, so the owning tracker can observe the bounce.
    #[allow(clippy::too_many_arguments)]
    pub fn restitute(
        self,
        entity: (u32, &str),
        living: bool,
        wanted: Vec3,
        moved: Vec3,
        effect: BodyContact,
        gravity: f64,
        drag: f32,
        velocity: &mut Vec3,
    ) -> bool {
        let collided_x = different(wanted.x, moved.x);
        let collided_z = different(wanted.z, moved.z);
        let vertical_collision = wanted.y != moved.y;
        if !(collided_x || collided_z || (wanted.y.abs() > 0.0 && vertical_collision)) {
            return false;
        }
        let mut body = BodyRestitution {
            motion: self,
            effect,
            entity,
            gravity,
            drag,
            velocity,
            sync: false,
        };
        let Ok(()) = restitute_movement(
            &mut body,
            RestitutionInput {
                block: effect.state,
                effect_position: effect.position,
                collided_x,
                collided_z,
                vertical_collision,
                collision_below: vertical_collision && wanted.y < 0.0,
                living,
                resolved_movement: moved,
                fall_distance: 0.0,
            },
        );
        body.sync
    }
}

struct BodyRestitution<'a, 'v> {
    motion: BodyMotion<'a>,
    effect: BodyContact,
    entity: (u32, &'a str),
    gravity: f64,
    drag: f32,
    velocity: &'v mut Vec3,
    sync: bool,
}

impl RestitutionWorld for BodyRestitution<'_, '_> {
    type Error = Infallible;

    fn suppressing_bounce(&mut self) -> Result<bool, Infallible> {
        Ok(false)
    }
    fn entity_bounciness(&mut self) -> Result<f64, Infallible> {
        Ok(0.0)
    }
    fn velocity(&mut self) -> Result<Vec3, Infallible> {
        Ok(*self.velocity)
    }
    fn effective_gravity(&mut self) -> Result<f64, Infallible> {
        Ok(self.gravity)
    }
    fn air_drag(&mut self) -> Result<f32, Infallible> {
        Ok(self.drag)
    }

    fn block_suppresses_bounce(&mut self, block: BlockStateId) -> Result<bool, Infallible> {
        Ok(self
            .motion
            .blocks
            .and_then(|blocks| blocks.get(block).ok())
            .is_some_and(|p| p.suppresses_bounce))
    }

    fn block_bounce_restitution(
        &mut self,
        state: BlockStateId,
        position: BlockPos,
    ) -> Result<f32, Infallible> {
        let own = self
            .motion
            .blocks
            .and_then(|blocks| blocks.get(state).ok())
            .map_or(0.0, |p| p.coefficients.bounce_restitution);
        let Some(side) = self.motion.side else {
            return Ok(own);
        };
        let facts = self.motion.facts;
        Ok(block_contact::bounce_restitution(
            FrictionLevel {
                world: self.motion.world,
                registry: self.motion.collision.registry(),
                context: self.motion.context,
                side,
                coefficient: &|id| facts.states.get(id.0 as usize).map(|p| p.friction),
            },
            Contact {
                entity: self.entity.0 as i32,
                kind: self.entity.1,
                state,
                position,
            },
            own,
        ))
    }

    // Nonliving ticks do not yet expose the separate Block.bounceOn and BOUNCE events.
    fn movement_support_position(&mut self) -> Result<BlockPos, Infallible> {
        Ok(block_support_at(
            self.motion.world,
            self.motion.blocks,
            self.effect.feet,
            self.effect.support,
            MOVEMENT_SUPPORT_OFFSET,
        ))
    }
    fn bounce_on(
        &mut self,
        _: BlockStateId,
        _: BlockPos,
        _: f64,
        movement: Vec3,
    ) -> Result<Vec3, Infallible> {
        Ok(movement)
    }
    fn bounce_event(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
    fn mark_position_for_sync(&mut self) {
        self.sync = true;
    }
    fn set_velocity(&mut self, velocity: Vec3) -> Result<(), Infallible> {
        *self.velocity = velocity;
        Ok(())
    }
}
