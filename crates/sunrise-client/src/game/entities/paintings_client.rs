#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, Hit, entity_culling::default_range};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::paintings::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    entities::Painting, items::ItemStack, mesher::Vertex, random_source,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::painting_variants::{
    PaintingVariants, Variant, frame_coexists, painting_coexists, painting_facing,
};

impl Game<'_> {
    /// `Painting.dropItem`: break sound, then item unless drops off or creative.
    pub(in crate::game) fn drop_painting(&mut self, p: Painting, at: [f64; 3], creative: bool) {
        if !self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::ENTITY_DROPS)
        {
            return;
        }
        self.audio
            .0
            .play("minecraft:entity.painting.break", Some(at), 1.0, 1.0);
        if creative {
            return;
        }
        let stack = ItemStack {
            item: Arc::from("minecraft:painting"),
            count: 1,
            damage: 0,
            extra: None,
        };
        self.spawn_item(
            stack,
            super::frames::hanging_drop_position(at, p.step()),
            [0.0; 3],
            10,
        );
    }
}
