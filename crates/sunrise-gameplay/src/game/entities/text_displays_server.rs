#[cfg(any())]
use crate::game::client_side::text_displays::RemoteTextDisplay;
#[cfg(any())]
use crate::game::client_side::text_displays::Shows;

#[cfg(any())]
pub(in crate::game) mod tests {
    use super::*;

    #[test]
    #[ignore = "daily"]
    fn handle_teleport_keeps_display_easing() {
        use crate::game::{commands::OtherEntity, native_entity_refs::tests::with_game};
        use sunrise_core::geometry::Vec3;
        use sunrise_core::nbt::{Compound, Tag};

        with_game(|game| {
            game.server.others.0.push(OtherEntity {
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
                sunrise_sim::text_display_data::display_data(&game.persist.raw[&12], false);
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

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
