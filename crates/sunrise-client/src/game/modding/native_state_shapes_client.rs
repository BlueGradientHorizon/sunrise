#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, perspective};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_state_shapes::Outline;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_state_shapes::collision_boxes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_state_shapes::outline_boxes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_state_shapes::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::BlockShapeKind;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    block_entity_anim::BlockEntityAnims, collisions::SceneCollisions, world::World,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{geometry::VoxelShape, position::BlockPos};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::{
    block_collision::BlockCollisionRegistry,
    collision::ShapeChannel,
    collision_context::{CollisionContext, EntityContextKind},
    registry::BlockStateId,
};

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn outline_edges_iter(
        &self,
        state: BlockStateId,
        pos: [i32; 3],
    ) -> impl Iterator<Item = [f64; 6]> + '_ {
        let (outline, offset) =
            self.selected_outline(&self.world, state, &self.be_anims, pos, true);
        let (owned, cached) = match outline {
            Outline::Published(shape) => (shape.edges(), &[][..]),
            Outline::Declared(_, kind) => (
                Vec::new(),
                self.collision
                    .cached_shapes()
                    .context_outline_edges(state, kind)
                    .unwrap_or_default(),
            ),
            Outline::Dynamic(_) => (sunrise_client::blocks::cube_edges(), &[][..]),
            Outline::Default(facts) => (Vec::new(), facts.outline_edges.as_ref()),
        };
        owned
            .into_iter()
            .chain(cached.iter().copied())
            .map(move |edge| {
                std::array::from_fn(|i| {
                    let axis = i % 3;
                    let local = match outline {
                        Outline::Dynamic(b) => b[axis] + edge[i] * (b[axis + 3] - b[axis]),
                        _ => edge[i],
                    };
                    f64::from(pos[axis]) + offset[axis] + local
                })
            })
    }
}
