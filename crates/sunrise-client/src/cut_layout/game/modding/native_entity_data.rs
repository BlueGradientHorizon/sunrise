

#[cfg(test)]
mod tests {
    use sunrise_gameplay_common::game::api_values::TryIntoApi;
    use super::super::native_entity_refs::tests::with_game;
    use mod_api::{EntityRef, MobCategory};
    use sunrise_core::geometry::Vec3;
    use sunrise_core::nbt::{JavaString, Tag};
    use sunrise_game_data::text::Text;
    use sunrise_sim::{entities::ArmorStand, mobs::Mob};

    fn push_mob(game: &mut sunrise_gameplay::game::ServerGame<'_>, id: u32, kind: &str) -> EntityRef {
        let kind = game.res.mob_types().get(kind).unwrap().clone();
        game.mobs
            .push(Mob::new(id, kind, Vec3::new(0.5, 70.0, 0.5), 0.0));
        game.entity(id).unwrap()
    }

    #[test]
    #[ignore = "daily"]
    fn live_entity_tags_lend_exact_utf16_without_changing_command_text() {
        with_game(|game| {
            let mob = push_mob(game, 11, "minecraft:zombie");
            assert!(mob.tags(game).unwrap().iter_utf16().next().is_none());

            game.put_saved(
                mob.id(),
                "Tags",
                Some(Tag::List(vec![
                    Tag::String("tag".into()),
                    Tag::String("\u{1f680}".into()),
                    Tag::Byte(7),
                    Tag::String(JavaString(vec![0xD800])),
                ])),
            );

            let tags = mob.tags(game).unwrap();
            let expected: [&[u16]; 3] = [&[0x0074, 0x0061, 0x0067], &[0xD83D, 0xDE80], &[0xD800]];
            assert_eq!(tags.iter_utf16().collect::<Vec<_>>(), expected);
            assert!(tags.contains("tag"));
            assert!(tags.contains("\u{1f680}"));
            assert!(!tags.contains("\u{fffd}"));

            // `/tag ... list` still receives the same strings, in the same order.
            let target = game.native_entity_target(mob).unwrap();
            assert_eq!(game.tags_of(target), ["tag", "\u{1f680}", "\u{fffd}"]);

            assert!(mob.discard(game));
            assert!(mob.tags(game).is_none());
        });
    }

    #[test]
    #[ignore = "daily"]
    fn custom_name_and_visibility_are_saved_and_load_again() {
        with_game(|game| {
            let mob = push_mob(game, 12, "minecraft:zombie");
            assert_eq!(mob.custom_name_visible(game), Some(false));
            let name = Text::colored("Grumbles", 0xFF5555);
            assert!(
                mob.set_custom_name(game, Some(name.try_into_api().unwrap()))
                    .unwrap()
            );
            assert!(mob.set_custom_name_visible(game, true));
            assert_eq!(mob.name(game).as_deref(), Some("Grumbles"));
            assert_eq!(mob.custom_name_visible(game), Some(true));
            let saved = game.entity_nbt(mob).unwrap();
            assert_eq!(saved.get(&"CustomNameVisible".into()), Some(&Tag::Byte(1)));

            // Taken away: neither the name nor the flag is saved.
            assert!(mob.set_custom_name(game, None).unwrap());
            assert!(mob.set_custom_name_visible(game, false));
            assert_ne!(mob.name(game).as_deref(), Some("Grumbles"));
            assert_eq!(mob.custom_name_visible(game), Some(false));
            let cleared = game.entity_nbt(mob).unwrap();
            assert!(!cleared.contains_key(&"CustomName".into()));
            assert!(!cleared.contains_key(&"CustomNameVisible".into()));

            // The saved data loads back with the styled name and the flag.
            let t = game.native_entity_target(mob).unwrap();
            game.apply_entity_nbt(t, Tag::Compound(saved)).unwrap();
            let m = game.mobs.iter().find(|m| m.id == 12).unwrap();
            assert_eq!(m.custom_name_text.as_ref(), Some(&name));
            assert_eq!(mob.name(game).as_deref(), Some("Grumbles"));
            assert_eq!(mob.custom_name_visible(game), Some(true));

            // An armor stand keeps its name in its own field.
            game.armor_stands.push(ArmorStand::new(13, Vec3::ZERO, 0.0));
            let stand = game.entity(13).unwrap();
            assert!(
                stand
                    .set_custom_name(game, Some(mod_api::Text::literal("Sign")))
                    .unwrap()
            );
            assert_eq!(stand.name(game).as_deref(), Some("Sign"));
            let saved = game.entity_nbt(stand).unwrap();
            let loaded = saved
                .get(&"CustomName".into())
                .map(sunrise_sim::entity_nbt::name_text);
            assert_eq!(loaded, Some(Text::literal("Sign")));
            assert!(stand.set_custom_name(game, None).unwrap());
            let saved = game.entity_nbt(stand).unwrap();
            assert!(!saved.contains_key(&"CustomName".into()));

            // The dragon keeps no name: refused, and a stale handle too.
            let mut dragon = sunrise_gameplay_common::game::dragon_common::Dragon::new(Vec3::ZERO, 0.0, 0);
            dragon.id = 14;
            game.dragon.replace(dragon);
            let dragon = game.entity(14).unwrap();
            assert!(
                !dragon
                    .set_custom_name(game, Some(mod_api::Text::literal("Puff")))
                    .unwrap()
            );
            assert!(!dragon.set_custom_name_visible(game, true));
            assert!(mob.discard(game));
            assert!(
                !mob.set_custom_name(game, Some(mod_api::Text::literal("Gone")))
                    .unwrap()
            );
            assert_eq!(mob.custom_name_visible(game), None);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn classification_is_the_types_category() {
        with_game(|game| {
            for (id, kind, category) in [
                (20, "minecraft:zombie", MobCategory::Monster),
                (21, "minecraft:cow", MobCategory::Creature),
                (22, "minecraft:bat", MobCategory::Ambient),
                (23, "minecraft:axolotl", MobCategory::Axolotls),
                (
                    24,
                    "minecraft:glow_squid",
                    MobCategory::UndergroundWaterCreature,
                ),
                (25, "minecraft:dolphin", MobCategory::WaterCreature),
                (26, "minecraft:cod", MobCategory::WaterAmbient),
            ] {
                let mob = push_mob(game, id, kind);
                assert_eq!(mob.classification(game), Some(category), "{kind}");
            }
            game.armor_stands.push(ArmorStand::new(27, Vec3::ZERO, 0.0));
            let stand = game.entity(27).unwrap();
            assert_eq!(stand.classification(game), Some(MobCategory::Misc));
            let mut dragon = sunrise_gameplay_common::game::dragon_common::Dragon::new(Vec3::ZERO, 0.0, 0);
            dragon.id = 28;
            game.dragon.replace(dragon);
            let dragon = game.entity(28).unwrap();
            assert_eq!(dragon.classification(game), Some(MobCategory::Monster));
        });
    }

    #[test]
    #[ignore = "daily"]
    fn an_entity_is_in_its_level_from_its_join_until_its_removal() {
        use mod_api::{EntityJoinLevelEvent, Flow, Hooks, Host, ServerHost};
        use std::sync::{Arc, Mutex};

        /// Whether the joining entity was in its level while the hook ran.
        struct Observer(Arc<Mutex<Vec<bool>>>);
        #[mod_api::__subscriptions]
        impl Hooks for Observer {
            fn entity_join_level(
                &self,
                host: &mut dyn ServerHost,
                event: &mut EntityJoinLevelEvent<'_>,
            ) -> Flow {
                let host: &dyn Host = host;
                let entity = host.entity(event.entity).unwrap();
                self.0.lock().unwrap().push(entity.added_to_level(host));
                Flow::Continue
            }
        }
        let during = Arc::new(Mutex::new(Vec::new()));
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "level-membership-observer",
            Box::new(Observer(during.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                game.native_joins_skip();
                game.side.next_entity += 1;
                let id = game.side.next_entity;
                let cow = push_mob(game, id, "minecraft:cow");
                // In the lists, but not yet let in.
                assert!(!cow.added_to_level(game));
                game.native_joins_flush(false, 0);
                assert_eq!(*during.lock().unwrap(), [false]);
                assert!(cow.added_to_level(game));
                assert!(cow.discard(game));
                assert!(!cow.added_to_level(game));
            });
        });
    }
}
use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt5 as _;
