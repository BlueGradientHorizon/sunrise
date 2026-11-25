use super::Game;
use crate::game::native_state_shapes::Outline;
use sunrise_world::registry::BlockStateId;

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
