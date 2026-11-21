use super::*;

#[test]
fn menu_screen_factories_key_by_menu_type_and_refuse_builtins_and_duplicates() {
    const CUSTOM: &str = "alpha:pack";
    const CROSS_MOD: &str = "alpha:shared";
    const MINECRAFT_CUSTOM: &str = "minecraft:custom_pack";
    let factory = || -> Arc<dyn crate::MenuScreenFactory> {
        Arc::new(|_: &dyn ClientHost, _: crate::MenuScreenOpen<'_>| None)
    };
    let mut registrar = PartRegistrar::new("alpha");
    assert!(registrar.register_menu_screen(CUSTOM, factory()).is_ok());
    assert!(
        registrar
            .register_menu_screen("pack", factory())
            .unwrap_err()
            .contains("namespaced")
    );
    assert!(
        registrar
            .register_menu_screen(CUSTOM, factory())
            .unwrap_err()
            .contains("duplicate")
    );
    for builtin in BUILTIN_MENU_SCREENS {
        let error = registrar
            .register_menu_screen(builtin, factory())
            .unwrap_err();
        assert!(
            error.contains("duplicate") && error.contains(builtin),
            "{error}"
        );
    }

    // Another mod may register a screen for another mod's custom menu or a
    // non-built-in `minecraft:*` menu type, while duplicates still fail.
    registrar.mod_id = "beta";
    registrar.mod_index = 1;
    let error = registrar
        .register_menu_screen(CUSTOM, factory())
        .unwrap_err();
    assert!(error.contains("duplicate") && error.contains("beta") && error.contains(CUSTOM));
    assert!(registrar.register_menu_screen(CROSS_MOD, factory()).is_ok());
    assert!(
        registrar
            .register_menu_screen(MINECRAFT_CUSTOM, factory())
            .is_ok()
    );
    assert!(
        registrar
            .register_menu_screen(MINECRAFT_CUSTOM, factory())
            .unwrap_err()
            .contains("duplicate")
    );
    assert_eq!(registrar.menu_screen_factories[CUSTOM].owner, "alpha");
    assert_eq!(registrar.menu_screen_factories[CROSS_MOD].owner, "beta");
    assert_eq!(
        registrar.menu_screen_factories[MINECRAFT_CUSTOM].owner,
        "beta"
    );
    assert_eq!(registrar.refused.len(), 4 + BUILTIN_MENU_SCREENS.len());

    // Checkpoint rollback restores `menu_screen_factories` on refusal.
    let mut clean = PartRegistrar::new("alpha");
    clean.register_menu_screen(CUSTOM, factory()).unwrap();
    let rollback = clean.registration_checkpoint();
    clean.mod_id = "beta";
    clean.mod_index = 1;
    clean.register_menu_screen(CROSS_MOD, factory()).unwrap();
    rollback(&mut clean);
    let schema = super::super::tests::rule_schema();
    install_registrar(clean, &schema).unwrap();
    assert!(has_menu_screen_factory(CUSTOM));
    assert!(!has_menu_screen_factory(CROSS_MOD));
}

#[test]
fn constructing_a_menu_screen_requires_the_client() {
    enum ScreenPolicy {
        Factory,
        NoScreen,
    }
    #[mod_api::__subscriptions]
    impl Hooks for ScreenPolicy {
        fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
            if matches!(self, Self::Factory) {
                r.register_menu_screen(
                    "factory:menu",
                    Arc::new(|_: &dyn ClientHost, _: crate::MenuScreenOpen<'_>| None),
                )
                .unwrap();
            }
        }
    }
    let mods: crate::Mods = vec![
        ModEntry::new("factory", Box::new(ScreenPolicy::Factory)),
        ModEntry::new("unrelated", Box::new(ScreenPolicy::NoScreen)),
    ]
    .into();
    crate::testing::with_mods(mods.clone(), || {
        crate::__private::part_tests::install_parts_fixture(&mods).unwrap();
        assert_eq!(
            crate::__private::client_requirement("factory").as_deref(),
            Some("registers a client menu screen factory")
        );
        assert_eq!(crate::__private::client_requirement("unrelated"), None);
    });
}
