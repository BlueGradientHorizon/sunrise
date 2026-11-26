

#[cfg(test)]
mod namespace_review_tests {
    use super::*;

    #[test]
    #[ignore = "daily"]
    fn captured_content_vehicle_type_matches_its_actual_save_id() {
        sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
            let mut kind = game.res.mob_types().get("minecraft:cow").unwrap().clone();
            Arc::make_mut(&mut kind).id = "example:mob".into();
            game.mobs.push(Mob::new(151, kind, Vec3::ZERO, 0.0));
            let tag = game.mob_tree_nbt(0);
            assert_eq!(entity_nbt::kind(&tag).as_deref(), Some("example:mob"));
            let tree = game.capture_vehicle_tree_state(151).unwrap();
            assert_eq!(tree.name, "example:mob");
            assert!(game.prepare_vehicle_transfer(tag, tree).is_some());
        });
    }
}