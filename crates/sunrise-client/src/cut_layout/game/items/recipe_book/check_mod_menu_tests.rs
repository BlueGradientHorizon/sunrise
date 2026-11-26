use super::*;

#[test]
#[ignore = "daily"]
fn a_remote_non_grid_menu_can_request_a_recipe_check_only_in_an_eligible_screen() {
    let name = "check_fixture:non_grid";
    let menus = [(
        name.into(),
        "test".into(),
        9,
        0,
        Vec::new(),
        Vec::new(),
        None,
        None,
        true,
        None,
    )];
    sunrise_level::containers::with_mod_menu_types(menus, || {
        sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
            let (pipe, _server) = sunrise_protocol::connection::Pipe::pair();
            game.connect_local(pipe, "RecipeCheck").unwrap();
            game.side
                .remote
                .as_mut()
                .unwrap()
                .menu
                .set(Some((7, name.into())));
            game.side.remote.as_mut().unwrap().recipe_viewer.check = true;
            let recipe = "minecraft:stick";
            game.side.screen = Screen::Playing;
            assert!(game.transfer_recipe_check(recipe).is_none());
            assert!(game.side.recipe_ui.0.checks.asked.is_none());
            game.side.screen = Screen::Inventory;
            game.mode = sunrise_command::args::GameMode::Spectator;
            assert!(game.transfer_recipe_check(recipe).is_none());
            assert!(game.side.recipe_ui.0.checks.asked.is_none());
            game.mode = sunrise_command::args::GameMode::Survival;
            assert!(
                !game.mod_recipe_screen(),
                "the fixture has no crafting grid"
            );
            game.side.remote.as_mut().unwrap().recipe_viewer.check = false;
            assert!(game.transfer_recipe_check(recipe).is_none());
            assert!(game.side.recipe_ui.0.checks.asked.is_none());
            game.side.remote.as_mut().unwrap().recipe_viewer.check = true;
            assert!(
                game.transfer_recipe_check(recipe).is_none(),
                "reply is asynchronous"
            );
            assert_eq!(
                (game.side.recipe_ui.0.checks.asked.as_ref()).map(|(_, asked, _)| asked.as_str()),
                Some(recipe)
            );
        });
    });
}

#[test]
#[ignore = "daily"]
fn a_remote_crafting_table_has_the_crafting_book() {
    sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
        let (pipe, _server) = sunrise_protocol::connection::Pipe::pair();
        game.connect_local(pipe, "RecipeCheck").unwrap();
        game.side.screen = Screen::Inventory;
        game.side
            .remote
            .as_mut()
            .unwrap()
            .menu
            .set(Some((7, "minecraft:generic_9x3".into())));
        assert_eq!(game.active_book(), None, "a chest has no crafting book");
        game.side
            .remote
            .as_mut()
            .unwrap()
            .menu
            .set(Some((7, "minecraft:crafting".into())));
        assert_eq!(game.active_book(), Some(BookType::Crafting));
        assert!(
            game.mod_recipe_screen(),
            "a viewer's transfer and its check go into the table's grid"
        );
    });
}
