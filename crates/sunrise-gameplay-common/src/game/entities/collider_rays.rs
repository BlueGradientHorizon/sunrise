use crate::game::ray_shapes::{ray_interaction_shape, ray_shape_from_boxes};
use std::io;
use sunrise_core::{
    geometry::{Vec3, VoxelShape},
    position::BlockPos,
};
use sunrise_level::collisions::SceneCollisions;
use sunrise_world::{
    collision_context::CollisionContext,
    registry::BlockStateId,
    world_ray::{RayBlockMode, RayWorld},
};

/// `Level.clip` with `ClipContext.Block.COLLIDER`, no fluids and the
/// collision context of the entity whose exposure is measured.
pub(in crate::game) struct ClipData<'a, 'r> {
    pub facts: &'a sunrise_items::blocks::BlockFacts,
    pub behaviours: &'a sunrise_world::level::Behaviours,
    pub registry: &'r sunrise_world::registry::BlockRegistry,
}
pub struct ColliderRays<'a, 'r> {
    pub scene: SceneCollisions<'a, 'r>,
    pub context: CollisionContext<'static>,
    pub clip_data: Option<ClipData<'a, 'r>>,
}
impl RayWorld for ColliderRays<'_, '_> {
    type Error = io::Error;
    type Block = BlockStateId;
    type Fluid = ();
    fn block_state(&mut self, p: BlockPos) -> io::Result<BlockStateId> {
        let world = self.scene.world;
        if self.clip_data.is_some()
            && (!(world.bottom_y()..world.top_y()).contains(&p.y)
                || !world.chunks.contains_key(&(p.x >> 4, p.z >> 4)))
        {
            return Err(io::Error::other("ray entered an unloaded block"));
        }
        Ok(world.block(p.x, p.y, p.z))
    }
    fn fluid_state(&mut self, _: BlockPos) -> io::Result<()> {
        Ok(())
    }
    fn block_shape(
        &mut self,
        state: BlockStateId,
        p: BlockPos,
        mode: RayBlockMode,
    ) -> io::Result<VoxelShape> {
        if let Some(data) = &self.clip_data {
            // A block entity's published shapes stand over the declared ones, a
            // dynamic template's included (#1666): the collider below reads them
            // through the binding, the outline here.
            let at = [p.x, p.y, p.z];
            let published = sunrise_level::collisions::published_outline(
                self.scene.world,
                self.scene.binding,
                state,
                at,
            );
            if published.is_none() && data.behaviours.data(state).dynamic_shape {
                return Err(io::Error::other("dynamic ray shape unsupported"));
            }
            if mode == RayBlockMode::Outline {
                let facts = data.facts.get(state);
                let offset = crate::game::native_state_shapes::outline_offset(
                    data.facts,
                    state,
                    p.x,
                    p.z,
                    facts.outline_offset,
                );
                let offset = Vec3::new(offset[0], offset[1], offset[2]);
                if let Some(shape) = published {
                    return Ok(if offset == Vec3::ZERO {
                        shape.clone()
                    } else {
                        shape.moved(offset)
                    });
                }
                let cached = self.scene.binding.cached_shapes();
                if let Some(shape) = cached.context_outline_shape(state, self.context.entity_kind())
                {
                    return Ok(cached.positioned_shape(
                        state,
                        sunrise_world::collision::ShapeChannel::ContextOutline,
                        at,
                        shape,
                    ));
                }
                let shape = ray_shape_from_boxes(&facts.outline)?;
                return Ok(if offset == Vec3::ZERO {
                    shape
                } else {
                    shape.moved(offset)
                });
            }
        }
        let binding = self.scene.binding;
        binding
            .collision_shape_with_context(&mut self.scene, state, p, &self.context)
            .map_err(|e| io::Error::other(format!("{e:?}")))
    }
    fn interaction_shape(&mut self, state: BlockStateId, p: BlockPos) -> io::Result<VoxelShape> {
        let cached = self.scene.binding.cached_shapes();
        if let Some(shape) = cached.interaction_shape(state) {
            return Ok(cached.positioned_shape(
                state,
                sunrise_world::collision::ShapeChannel::Interaction,
                [p.x, p.y, p.z],
                shape,
            ));
        }
        match &self.clip_data {
            Some(data) => ray_interaction_shape(data.registry, state),
            None => Ok(VoxelShape::empty()),
        }
    }
    fn fluid_is_source(&mut self, _: ()) -> io::Result<bool> {
        Ok(false)
    }
    fn fluid_is_empty(&mut self, _: ()) -> io::Result<bool> {
        Ok(true)
    }
    fn fluid_is_water(&mut self, _: ()) -> io::Result<bool> {
        Ok(false)
    }
    fn fluid_shape(&mut self, _: (), _: BlockPos) -> io::Result<VoxelShape> {
        Ok(VoxelShape::empty())
    }
}

use crate::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
