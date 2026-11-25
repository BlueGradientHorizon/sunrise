#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::text_displays::RemoteTextDisplay;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::text_displays::SeeThroughCollector;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::text_displays::Shows;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::text_displays::TranslucentDisplayCollector;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::text_displays::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_assets::item_model::DisplayContext;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    gfx::{
        TEXT_POLYGON_OFFSET_OVERLAY, TEXT_POLYGON_OFFSET_SOLID_OVERLAY, TRANSLUCENT_DRAW_FLIP,
        TRANSLUCENT_GLINT_ITEM_OVERLAY, TRANSLUCENT_ITEM_OVERLAY,
    },
    items::ItemStack,
    mesher::Vertex,
    text::Text,
    text_display::{self, CachedQuads, TextDisplayData},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::entity_data::Value;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::registry::BlockStateId;

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::client_side::text_displays::tests::*;

    #[test]
    #[ignore = "daily"]
    fn handle_teleport_keeps_display_easing() {
        use crate::game::{commands::OtherEntity, native_entity_refs::tests::with_game};
        use sunrise_core::geometry::Vec3;
        use sunrise_core::nbt::{Compound, Tag};

        with_game(|game| {
            game.side.others.0.push(OtherEntity {
                size_answer: None,
                attachment_lifetime: Default::default(),
                knot_check: None,
                eye: None,
                join: Default::default(),
                item_transformation: None,
                interaction: None,
                id: 12,
                kind: "minecraft:text_display".into(),
                position: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
            });
            let mut tag = Compound::new();
            tag.insert("teleport_duration".into(), Tag::Int(4));
            game.persist.raw.insert(12, Tag::Compound(tag));
            let entity = game.entity(12).unwrap();
            assert!(entity.teleport_to(game, [8.0, 0.0, 0.0], None));
            let metadata =
                sunrise_client::text_display::display_data(&game.persist.raw[&12], false);
            let mut client = RemoteTextDisplay::new(Shows::Text, [0.0; 3], 0.0);
            client.metadata(&metadata);
            client.move_to(entity.position(game).unwrap(), None);
            client.tick();
            assert_eq!(client.position.x, 2.0);
            for _ in 0..3 {
                client.tick();
            }
            assert_eq!(client.position.x, 8.0);
        });
    }
}
