#![cfg(test)]
use mod_api::{
    Billboard, DisplayKind, DisplayRef, EntityRef, ItemDisplayContext, TextAlignment,
    Transformation,
};
use sunrise_core::nbt::Tag;
use sunrise_game_data::scoreboard::comp_text;
use sunrise_game_data::scoreboard::tag_to_json;
use sunrise_items::items::ItemStack;
use sunrise_protocol::entity_data::Value;
use sunrise_sim::text_display_data::{display_data, text_display_data};
use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

#[cfg(test)]
mod tests {
    use super::super::native_entity_refs::tests::with_game;
    use super::*;
    use mod_api::WorldEpoch;
    use sunrise_core::geometry::Vec3;
    use sunrise_gameplay_common::game::api_values::TryIntoApi;
    use sunrise_gameplay_common::game::commands_common::OtherEntity;

    fn push(game: &mut sunrise_gameplay::game::ServerGame<'_>, id: u32, kind: &str) -> DisplayRef {
        game.options.dedicated = Some(Vec::new());
        game.server.others.0.push(OtherEntity {
            size_answer: None,
            attachment_lifetime: Default::default(),
            knot_check: None,
            eye: None,
            join: Default::default(),
            item_transformation: None,
            interaction: None,
            id,
            kind: kind.into(),
            position: Vec3::new(1.0, 70.0, 1.0),
            yaw: 0.0,
            pitch: 0.0,
        });
        let entity = game.entity(id).unwrap();
        game.display(entity).unwrap()
    }

    #[test]
    #[ignore = "daily"]
    fn displays_narrow_by_kind_and_share_the_display_state() {
        with_game(|game| {
            let text = push(game, 21, "minecraft:text_display");
            let block = push(game, 22, "minecraft:block_display");
            let item = push(game, 23, "minecraft:item_display");
            assert_eq!(text.kind(game), Some(DisplayKind::Text));
            assert_eq!(block.kind(game), Some(DisplayKind::Block));
            assert_eq!(item.kind(game), Some(DisplayKind::Item));
            assert_eq!(text.entity(), game.entity(21).unwrap());
            game.server.others.0.push(OtherEntity {
                size_answer: None,
                attachment_lifetime: Default::default(),
                knot_check: None,
                eye: None,
                join: Default::default(),
                item_transformation: None,
                interaction: None,
                id: 24,
                kind: "minecraft:marker".into(),
                position: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
            });
            assert!(game.display(game.entity(24).unwrap()).is_none());

            assert_eq!(text.transformation(game), Some(Transformation::IDENTITY));
            let moved = Transformation {
                translation: [0.0, 1.5, 0.0],
                left_rotation: [0.0, 1.0, 0.0, 0.0],
                scale: [2.0, 2.0, 2.0],
                right_rotation: [0.0, 0.0, 0.0, 1.0],
            };
            assert!(text.set_transformation(game, moved));
            assert_eq!(text.transformation(game), Some(moved));
            let mut bad = moved;
            bad.scale[1] = f32::NAN;
            assert!(!text.set_transformation(game, bad));
            bad = moved;
            bad.left_rotation = [0.0; 4];
            assert!(!text.set_transformation(game, bad));
            assert_eq!(text.transformation(game), Some(moved));

            assert_eq!(item.billboard(game), Some(Billboard::Fixed));
            assert!(item.set_billboard(game, Billboard::Center));
            assert_eq!(item.billboard(game), Some(Billboard::Center));
            assert_eq!(block.brightness(game), None);
            assert!(block.set_brightness(game, Some((15, 3))));
            assert_eq!(block.brightness(game), Some((15, 3)));
            assert!(!block.set_brightness(game, Some((16, 0))));
            assert!(block.set_brightness(game, None));
            assert_eq!(block.brightness(game), None);
            assert_eq!(block.view_range(game), Some(1.0));
            assert!(block.set_view_range(game, 4.0));
            assert!(!block.set_view_range(game, f32::INFINITY));
            assert_eq!(block.view_range(game), Some(4.0));
            assert!(block.set_shadow_radius(game, 0.5));
            assert!(block.set_shadow_strength(game, 0.25));
            assert!(block.set_width(game, 2.0));
            assert!(block.set_height(game, 3.0));
            assert!(block.set_glow_color_override(game, 0xff00ff));
            assert_eq!(
                (
                    block.shadow_radius(game),
                    block.shadow_strength(game),
                    block.width(game),
                    block.height(game),
                    block.glow_color_override(game)
                ),
                (Some(0.5), Some(0.25), Some(2.0), Some(3.0), Some(0xff00ff))
            );
            assert!(item.set_interpolation_duration(game, 8));
            assert!(item.set_start_interpolation(game, -1));
            assert!(item.set_teleport_duration(game, 99));
            assert_eq!(
                (
                    item.interpolation_duration(game),
                    item.start_interpolation(game),
                    item.teleport_duration(game)
                ),
                (Some(8), Some(-1), Some(59))
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn display_kinds_keep_their_own_state_and_refuse_the_others() {
        with_game(|game| {
            let text = push(game, 21, "minecraft:text_display");
            let block = push(game, 22, "minecraft:block_display");
            let item = push(game, 23, "minecraft:item_display");
            assert_eq!(text.line_width(game), Some(200));
            assert_eq!(text.background(game), Some(0x4000_0000));
            assert_eq!(text.text_opacity(game), Some(-1));
            assert_eq!(text.alignment(game), Some(TextAlignment::Center));
            assert!(
                text.set_text(game, mod_api::Text::literal("hello"))
                    .unwrap()
            );
            assert!(text.set_line_width(game, 80));
            assert!(text.set_background(game, 0x20ff0000));
            assert!(text.set_text_opacity(game, 100));
            assert!(text.set_text_shadow(game, true));
            assert!(text.set_see_through(game, true));
            assert!(text.set_default_background(game, true));
            assert!(text.set_alignment(game, TextAlignment::Right));
            assert_eq!(
                text.text(game).unwrap().unwrap(),
                mod_api::Text::literal("hello")
            );
            assert_eq!(
                (
                    text.line_width(game),
                    text.background(game),
                    text.text_opacity(game),
                    text.text_shadow(game),
                    text.see_through(game),
                    text.default_background(game),
                    text.alignment(game)
                ),
                (
                    Some(80),
                    Some(0x20ff0000),
                    Some(100),
                    Some(true),
                    Some(true),
                    Some(true),
                    Some(TextAlignment::Right)
                )
            );
            assert!(text.set_alignment(game, TextAlignment::Left));
            assert!(text.set_text_shadow(game, false));
            assert_eq!(text.alignment(game), Some(TextAlignment::Left));
            assert_eq!(text.text_shadow(game), Some(false));
            assert_eq!(text.see_through(game), Some(true));

            let stone = game
                .block_state_id("minecraft:stone")
                .unwrap()
                .try_into_api()
                .unwrap();
            let stairs = game
                .block_state_id("minecraft:oak_stairs[facing=east]")
                .unwrap()
                .try_into_api()
                .unwrap();
            let air = game
                .block_state_id("minecraft:air")
                .unwrap()
                .try_into_api()
                .unwrap();
            assert_eq!(block.block_state(game), Some(air));
            assert!(block.set_block_state(game, stone));
            assert_eq!(block.block_state(game), Some(stone));
            assert!(block.set_block_state(game, stairs));
            assert_eq!(block.block_state(game), Some(stairs));
            let invalid = mod_api::__private::block_state_from_ordinal(stairs.block(), u16::MAX);
            assert!(!block.set_block_state(game, invalid));
            assert_eq!(block.block_state(game), Some(stairs));

            let sword = ItemStack {
                item: "minecraft:diamond_sword".into(),
                count: 1,
                damage: 3,
                extra: None,
            };
            assert_eq!(item.item(game), None);
            assert_eq!(item.item_context(game), Some(ItemDisplayContext::None));
            let sword = sword.try_into_api().unwrap();
            assert_eq!(item.set_item(game, Some(sword.clone())), Ok(true));
            assert!(item.set_item_context(game, ItemDisplayContext::Head));
            assert_eq!(item.item(game), Some(sword));
            assert_eq!(item.item_context(game), Some(ItemDisplayContext::Head));
            assert_eq!(item.set_item(game, None), Ok(true));
            assert_eq!(item.item(game), None);

            // The wrong kind reads None and writes nothing.
            assert_eq!(block.text(game).unwrap(), None);
            assert!(!block.set_text(game, mod_api::Text::literal("x")).unwrap());
            assert!(!text.set_block_state(game, stone));
            assert_eq!(text.block_state(game), None);
            assert!(!block.set_item_context(game, ItemDisplayContext::Gui));
            assert!(!item.set_alignment(game, TextAlignment::Left));
            assert_eq!(item.line_width(game), None);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn display_writes_reach_entity_data_and_the_saved_entity() {
        with_game(|game| {
            let text = push(game, 21, "minecraft:text_display");
            assert!(
                text.set_text(game, mod_api::Text::literal("saved"))
                    .unwrap()
            );
            assert!(text.set_billboard(game, Billboard::Vertical));
            assert!(text.set_view_range(game, 2.5));
            let raw = game.persist.raw.get(&21).unwrap().clone();
            // The entity data the tracker sends: the changed entries only.
            let data = display_data(&raw, false);
            assert!(data.iter().any(|e| e.0 == 15 && e.2 == Value::Byte(1)));
            assert!(data.iter().any(|e| e.0 == 17 && e.2 == Value::Float(2.5)));
            assert!(text_display_data(&raw, false).iter().any(|e| e.0 == 23));
            // The saved entity is the compound `/data get entity` shows.
            let o = game.server.others.0.iter().find(|o| o.id == 21).unwrap();
            let saved = game.other_to_nbt(o, [1, 2, 3, 4]);
            assert_eq!(
                saved.get("billboard"),
                Some(&Tag::String("vertical".into()))
            );
            assert_eq!(saved.get("view_range"), Some(&Tag::Float(2.5f32.to_bits())));
            assert_eq!(
                comp_text(&tag_to_json(saved.get("text").unwrap())).plain(),
                "saved"
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn each_start_delay_write_forces_a_send_even_when_equal() {
        with_game(|game| {
            let text = push(game, 21, "minecraft:text_display");
            let other = push(game, 22, "minecraft:item_display");
            assert!(game.persist.forced_meta.is_empty());
            for _ in 0..2 {
                assert!(text.set_start_interpolation(game, 0));
                assert_eq!(text.start_interpolation(game), Some(0));
                assert_eq!(game.persist.forced_meta.get(&21), Some(&[1 << 8, 0, 0, 0]));
                // The tracking pass spends the force; the next write sets it again.
                game.spend_forced_meta();
            }
            // Other writes, and writes that fail, force nothing.
            assert!(other.set_view_range(game, 3.0));
            assert!(game.persist.forced_meta.is_empty());
            assert!(text.entity().discard(game));
            assert!(!text.set_start_interpolation(game, 0));
            assert!(game.persist.forced_meta.is_empty());
        });
    }

    #[test]
    #[ignore = "daily"]
    fn stale_removed_and_client_displays_do_nothing() {
        with_game(|game| {
            let text = push(game, 21, "minecraft:text_display");
            let stale = DisplayRef::new(EntityRef::new(
                WorldEpoch::new(text.entity().epoch().get() + 1),
                21,
            ));
            assert_eq!(stale.kind(game), None);
            assert_eq!(stale.view_range(game), None);
            assert!(!stale.set_view_range(game, 2.0));
            assert!(!stale.set_text(game, mod_api::Text::literal("x")).unwrap());
            assert!(text.entity().discard(game));
            assert_eq!(text.kind(game), None);
            assert!(!text.set_width(game, 1.0));
        });
    }
}
