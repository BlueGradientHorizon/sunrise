//! Validate registered destination roles before publishing content menu types.
use mod_api::RecipeBookLayout;
use sunrise_client::recipe_book::{
    menu_layout::{MenuLayoutMetadata, compile},
    slot_layout::LayoutError,
};
use sunrise_mod_package::content::MenuType;

pub(super) fn validate_registered(menus: &[&MenuType]) -> Result<(), String> {
    for menu in RecipeBookLayout::registered_menus() {
        let Some((_, _, layout)) = RecipeBookLayout::registered(&menu) else {
            continue;
        };
        validate(menus, &menu, layout)?;
        if !mod_api::__private::has_menu_screen_factory(&menu) {
            return Err(format!(
                "recipe book menu {menu:?}: a menu screen factory must be registered"
            ));
        }
    }
    Ok(())
}

fn validate(menus: &[&MenuType], id: &str, layout: RecipeBookLayout) -> Result<(), String> {
    let menu = menus
        .iter()
        .find(|menu| menu.id == id)
        .ok_or_else(|| format!("recipe book menu {id:?}: unknown content menu"))?;
    let metadata = MenuLayoutMetadata {
        slots: menu.slots,
        crafting: menu
            .crafting
            .map(|grid| (grid.grid, grid.size, grid.result)),
    };
    compile(layout, metadata)
        .map_err(|error: LayoutError| format!("recipe book menu {id:?}: {error:?}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn menu() -> MenuType {
        MenuType {
            id: "demo:pot".into(),
            slots: 3,
            player_inventory: true,
            data_slots: 0,
            results: vec![2],
            inputs: Vec::new(),
            crafting: None,
            links: None,
        }
    }
    #[test]
    fn actual_content_count_is_required_before_publication() {
        let menu = menu();
        let roles = RecipeBookLayout::Furnace {
            input: 0,
            fuel: 1,
            result: 2,
        };
        assert!(validate(&[&menu], "demo:pot", roles.clone()).is_ok());
        assert!(
            validate(&[], "demo:pot", roles.clone())
                .unwrap_err()
                .contains("unknown content menu")
        );
        let short = MenuType { slots: 2, ..menu };
        assert!(
            validate(&[&short], "demo:pot", roles.clone())
                .unwrap_err()
                .contains("OutOfRange(2)")
        );
        let slotless = MenuType {
            slots: 0,
            player_inventory: false,
            results: vec![],
            ..short
        };
        let error = validate(&[&slotless], "demo:pot", roles).unwrap_err();
        assert!(
            error.contains("recipe book menu \"demo:pot\"") && error.contains("InvalidSlotCount"),
            "{error}"
        );
    }
    #[test]
    fn a_bound_menu_needs_a_registered_screen_factory() {
        struct Demo;
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Demo {
            fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
                let book = r.recipe_book_type("demo:book").unwrap();
                r.menu_recipe_book(
                    "demo:pot",
                    &book,
                    "demo:cooking",
                    RecipeBookLayout::Furnace {
                        input: 0,
                        fuel: 1,
                        result: 2,
                    },
                )
                .unwrap();
            }
        }
        struct ScreenProvider;
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for ScreenProvider {
            fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
                r.register_menu_screen(
                    "demo:pot",
                    std::sync::Arc::new(
                        |_: &dyn mod_api::ClientHost, _: mod_api::MenuScreenOpen<'_>| None,
                    ),
                )
                .unwrap();
            }
        }
        for has_factory in [false, true] {
            let mut entries = vec![mod_api::ModEntry::new("demo", Box::new(Demo))];
            if has_factory {
                entries.push(mod_api::ModEntry::new("other", Box::new(ScreenProvider)));
            }
            let mods: mod_api::Mods = entries.into();
            mod_platform::testing::with_mods(mods.clone(), || {
                mod_platform::__private::part_tests::install_parts_fixture(&mods).unwrap();
                let result = validate_registered(&[&menu()]);
                if has_factory {
                    assert!(result.is_ok());
                } else {
                    assert_eq!(
                        result.unwrap_err(),
                        "recipe book menu \"demo:pot\": a menu screen factory must be registered"
                    );
                }
            });
        }
    }
    #[test]
    fn persistent_grid_roles_are_valid_but_computed_storage_must_agree() {
        let persistent = MenuType {
            slots: 8,
            results: vec![6],
            ..menu()
        };
        let roles = RecipeBookLayout::Crafting {
            width: 3,
            height: 2,
            slots: (0..6).collect(),
            result: 6,
        };
        assert!(validate(&[&persistent], "demo:pot", roles.clone()).is_ok());
        let computed = MenuType {
            crafting: Some(sunrise_mod_package::content::MenuCrafting {
                grid: 0,
                size: 2,
                result: 6,
            }),
            ..persistent
        };
        assert!(
            validate(&[&computed], "demo:pot", roles)
                .unwrap_err()
                .contains("InvalidGrid")
        );
        let matching = RecipeBookLayout::Crafting {
            width: 2,
            height: 2,
            slots: (0..4).collect(),
            result: 6,
        };
        assert!(validate(&[&computed], "demo:pot", matching).is_ok());
    }
}
