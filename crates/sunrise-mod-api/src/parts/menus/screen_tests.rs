use super::*;

#[test]
fn a_second_factory_for_the_same_menu_and_owner_is_refused() {
    const CUSTOM: &str = "alpha:pack";
    const STOCK: &str = "minecraft:hopper";
    let factory = || -> Arc<dyn crate::MenuScreenFactory> {
        Arc::new(|_: &dyn ClientHost, _: crate::MenuScreenOpen<'_>| None)
    };
    let mut registrar = PartRegistrar::new("alpha");
    assert!(registrar.register_menu_screen(CUSTOM, factory()).is_ok());
    assert!(registrar.register_menu_screen(STOCK, factory()).is_ok());
    assert!(
        registrar
            .register_menu_screen(CUSTOM, factory())
            .unwrap_err()
            .contains("duplicate")
    );
    registrar.mod_id = "beta";
    registrar.mod_index = 1;
    assert!(
        registrar
            .register_menu_screen(CUSTOM, factory())
            .unwrap_err()
            .contains("owned")
    );
    assert!(registrar.register_menu_screen(STOCK, factory()).is_ok());
    let error = registrar
        .register_menu_screen(STOCK, factory())
        .unwrap_err();
    assert!(error.contains("duplicate") && error.contains("beta") && error.contains(STOCK));
    assert_eq!(registrar.menu_screen_factories[CUSTOM].len(), 1);
    let stock = &registrar.menu_screen_factories[STOCK];
    assert_eq!((stock[&0].owner, stock[&1].owner), ("alpha", "beta"));
    assert_eq!(registrar.refused.len(), 3);
}

#[test]
fn constructing_or_requiring_a_menu_screen_requires_the_client() {
    enum ScreenPolicy {
        Factory,
        Mask,
        NoScreen,
    }
    #[mod_api::__subscriptions]
    impl Hooks for ScreenPolicy {
        fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
            match self {
                Self::Factory => r
                    .register_menu_screen(
                        "minecraft:hopper",
                        Arc::new(|_: &dyn ClientHost, _: crate::MenuScreenOpen<'_>| None),
                    )
                    .unwrap(),
                Self::Mask => r
                    .menu_slot_exclusions("minecraft:hopper", Some("masked:machine"), &[4])
                    .unwrap(),
                Self::NoScreen => {}
            }
        }
    }
    let mods: crate::Mods = vec![
        ModEntry::new("factory", Box::new(ScreenPolicy::Factory)),
        ModEntry::new("masked", Box::new(ScreenPolicy::Mask)),
        ModEntry::new("unrelated", Box::new(ScreenPolicy::NoScreen)),
    ]
    .into();
    crate::testing::with_mods(mods.clone(), || {
        crate::__private::part_tests::install_parts_fixture(&mods).unwrap();
        assert_eq!(
            crate::__private::client_requirement("factory").as_deref(),
            Some("registers a client menu screen factory")
        );
        assert_eq!(
            crate::__private::client_requirement("masked").as_deref(),
            Some("excludes menu slots that require a client menu screen")
        );
        assert_eq!(crate::__private::client_requirement("unrelated"), None);
    });
}

#[test]
fn refused_slot_exclusions_cannot_replace_a_scoped_mask() {
    const MENU: &str = "alpha:pack";
    const STOCK: &str = "minecraft:hopper";
    const BLOCK: &str = "alpha:machine";
    let declare = || {
        let mut registrar = PartRegistrar::new("alpha");
        registrar
            .menu_slot_exclusions(MENU, None, &[3, 1, 3])
            .unwrap();
        registrar
            .menu_slot_exclusions(MENU, Some(BLOCK), &[])
            .unwrap();
        registrar
            .menu_slot_exclusions(STOCK, Some(BLOCK), &[4])
            .unwrap();
        registrar
    };
    let mut registrar = declare();
    let good = registrar.menu_slot_exclusions.clone();
    assert!(registrar.menu_slot_exclusions(MENU, None, &[2]).is_err());
    assert!(registrar.menu_slot_exclusions(STOCK, None, &[2]).is_err());
    assert!(
        registrar
            .menu_slot_exclusions(STOCK, Some("beta:machine"), &[2])
            .is_err()
    );
    assert!(
        registrar
            .menu_slot_exclusions(MENU, Some("alpha:"), &[2])
            .is_err()
    );
    registrar.mod_id = "beta";
    assert!(
        registrar
            .menu_slot_exclusions(MENU, Some("beta:machine"), &[2])
            .is_err()
    );
    assert_eq!(registrar.menu_slot_exclusions, good);

    let schema = super::super::tests::rule_schema();
    let errors = install_registrar(registrar, &schema).unwrap_err();
    assert_eq!(errors.len(), 5);
    assert!(menu_slot_exclusions(MENU, None).is_empty());

    let mut registrar = declare();
    let rollback = registrar.registration_checkpoint();
    registrar
        .menu_slot_exclusions(MENU, Some("beta:machine"), &[2])
        .unwrap();
    rollback(&mut registrar);
    install_registrar(registrar, &schema).unwrap();
    assert_eq!(menu_slot_exclusions(MENU, None), &[1, 3]);
    assert!(menu_slot_exclusions(MENU, Some(BLOCK)).is_empty());
    assert_eq!(menu_slot_exclusions(MENU, Some("beta:machine")), &[1, 3]);
    assert_eq!(menu_slot_exclusions(STOCK, Some(BLOCK)), &[4]);
    assert!(menu_slot_exclusions(STOCK, Some("alpha:other")).is_empty());
    assert!(menu_slot_exclusions(STOCK, None).is_empty());
}

#[test]
fn served_content_menu_screen_dispatches_to_serving_host_only() {
    struct HostMod;
    #[mod_api::__subscriptions]
    impl crate::Hooks for HostMod {
        fn register(&self, r: &mut dyn Registrar, _: &crate::RegistrationContext) {
            r.menu_part("runes:altar", Layout);
            r.menu_part("runes_extra:forge", Layout);
            r.menu_part("minecraft:hopper", Layout);
        }
    }
    struct ThirdMod;
    #[mod_api::__subscriptions]
    impl crate::Hooks for ThirdMod {
        fn register(&self, r: &mut dyn Registrar, _: &crate::RegistrationContext) {
            r.menu_part("plain:table", Overlay);
            r.menu_part("minecraft:hopper", Overlay);
        }
    }
    struct ThirdHijack;
    #[mod_api::__subscriptions]
    impl crate::Hooks for ThirdHijack {
        fn register(&self, r: &mut dyn Registrar, _: &crate::RegistrationContext) {
            r.menu_part("runes:altar", Overlay);
        }
    }

    let packages = || {
        vec![
            crate::PackageInfo::new(
                "horizon_java".into(),
                "1".into(),
                crate::PackageKind::Code,
                crate::Side::Both,
                vec!["horizon_java".into()],
                vec![],
            ),
            crate::PackageInfo::new(
                "third".into(),
                "1".into(),
                crate::PackageKind::Code,
                crate::Side::Both,
                vec!["third".into()],
                vec![],
            ),
            crate::PackageInfo::new(
                "runes".into(),
                "1".into(),
                crate::PackageKind::Content,
                crate::Side::Both,
                vec!["runes".into(), "runes_extra".into()],
                vec![],
            )
            .with_served_by(Some("horizon_java".into())),
            crate::PackageInfo::new(
                "plain".into(),
                "1".into(),
                crate::PackageKind::Content,
                crate::Side::Both,
                vec!["plain".into()],
                vec![],
            ),
        ]
    };

    crate::load_state::with_test_package_inventory(packages(), || {
        crate::parts::tests::install_parts_fixture(&[
            crate::ModEntry::new("horizon_java", Box::new(HostMod)),
            crate::ModEntry::new("third", Box::new(ThirdMod)),
        ])
        .unwrap();
        assert_eq!(menu_screen_owner("runes:altar", None), Some("horizon_java"));
        assert_eq!(
            menu_screen_owner("runes_extra:forge", None),
            Some("horizon_java")
        );
        assert_eq!(
            menu_screen_owner("minecraft:hopper", Some("runes:altar")),
            Some("horizon_java")
        );
        assert_eq!(
            menu_screen_owner("minecraft:hopper", Some("minecraft:hopper")),
            None
        );
        assert_eq!(menu_screen_owner("plain:table", None), None);
    });
    // Per-frame screen dispatch reads the pre-indexed table without querying the package
    // inventory again: clearing the test package inventory leaves the resolved owners intact.
    assert_eq!(menu_screen_owner("runes:altar", None), Some("horizon_java"));
    assert_eq!(
        menu_screen_owner("runes_extra:forge", None),
        Some("horizon_java")
    );
    assert_eq!(
        menu_screen_owner("minecraft:hopper", Some("runes:altar")),
        Some("horizon_java")
    );
    assert_eq!(
        menu_screen_owner("minecraft:hopper", Some("runes_extra:forge")),
        Some("horizon_java")
    );
    assert_eq!(
        menu_screen_owner("minecraft:hopper", Some("minecraft:hopper")),
        None
    );
    assert_eq!(menu_screen_owner("plain:table", None), None);

    crate::load_state::with_test_package_inventory(packages(), || {
        crate::parts::tests::install_parts_fixture(&[crate::ModEntry::new(
            "third",
            Box::new(ThirdHijack),
        )])
        .unwrap();
        assert_eq!(menu_screen_owner("runes:altar", None), None);
    });
}
