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

#[test]
fn served_content_menu_screen_dispatches_to_serving_host_only() {
    use std::sync::{Arc, Mutex};

    struct HostPart(Arc<Mutex<Option<&'static str>>>);
    #[mod_api::sunrise_part]
    impl crate::MenuPart for HostPart {
        fn menu_button_valid(
            &self,
            _: &dyn crate::Host,
            _: &mut crate::MenuButtonValidEvent<'_>,
        ) -> crate::Flow {
            *self.0.lock().unwrap() = Some("horizon_java");
            crate::Flow::Continue
        }
    }
    struct ThirdPart(Arc<Mutex<Option<&'static str>>>);
    #[mod_api::sunrise_part]
    impl crate::MenuPart for ThirdPart {
        fn menu_button_valid(
            &self,
            _: &dyn crate::Host,
            _: &mut crate::MenuButtonValidEvent<'_>,
        ) -> crate::Flow {
            *self.0.lock().unwrap() = Some("third");
            crate::Flow::Continue
        }
    }
    struct HostMod {
        seen: Arc<Mutex<Option<&'static str>>>,
    }
    #[mod_api::__subscriptions]
    impl crate::Hooks for HostMod {
        fn register(&self, r: &mut dyn Registrar, _: &crate::RegistrationContext) {
            r.menu_part("runes:altar", HostPart(self.seen.clone()));
            r.menu_part("runes_extra:forge", HostPart(self.seen.clone()));
            r.menu_part("minecraft:hopper", HostPart(self.seen.clone()));
            let f = || -> Arc<dyn crate::MenuScreenFactory> {
                Arc::new(|_: &dyn ClientHost, _: crate::MenuScreenOpen<'_>| None)
            };
            r.register_menu_screen("runes:altar", f()).unwrap();
            r.register_menu_screen("runes_extra:forge", f()).unwrap();
            r.register_menu_screen("minecraft:hopper", f()).unwrap();
        }
    }
    struct ThirdMod {
        seen: Arc<Mutex<Option<&'static str>>>,
    }
    #[mod_api::__subscriptions]
    impl crate::Hooks for ThirdMod {
        fn register(&self, r: &mut dyn Registrar, _: &crate::RegistrationContext) {
            r.menu_part("plain:table", ThirdPart(self.seen.clone()));
            r.menu_part("minecraft:hopper", ThirdPart(self.seen.clone()));
            let f = || -> Arc<dyn crate::MenuScreenFactory> {
                Arc::new(|_: &dyn ClientHost, _: crate::MenuScreenOpen<'_>| None)
            };
            r.register_menu_screen("minecraft:hopper", f()).unwrap();
        }
    }
    struct ThirdHijack {
        seen: Arc<Mutex<Option<&'static str>>>,
    }
    #[mod_api::__subscriptions]
    impl crate::Hooks for ThirdHijack {
        fn register(&self, r: &mut dyn Registrar, _: &crate::RegistrationContext) {
            r.menu_part("runes:altar", ThirdPart(self.seen.clone()));
            let f = || -> Arc<dyn crate::MenuScreenFactory> {
                Arc::new(|_: &dyn ClientHost, _: crate::MenuScreenOpen<'_>| None)
            };
            assert!(r.register_menu_screen("runes:altar", f()).is_err());
        }
    }

    let host = crate::testing::FakeHost::new();
    let seen: Arc<Mutex<Option<&'static str>>> = Arc::new(Mutex::new(None));

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

    let mods: crate::Mods = vec![
        crate::ModEntry::new("horizon_java", Box::new(HostMod { seen: seen.clone() })),
        crate::ModEntry::new("third", Box::new(ThirdMod { seen: seen.clone() })),
    ]
    .into();
    crate::load_state::with_test_package_inventory(packages(), || {
        crate::testing::with_mods(mods.clone(), || {
            let menu_opener_owner = |menu: &str, block: Option<&str>| -> Option<&'static str> {
                *seen.lock().unwrap() = None;
                let mut accepted = false;
                crate::dispatch_menu_button_valid(&host, menu, block, 1, &mut accepted);
                *seen.lock().unwrap()
            };
            crate::parts::tests::install_parts_fixture(&mods).unwrap();
            assert_eq!(menu_opener_owner("runes:altar", None), Some("horizon_java"));
            assert_eq!(
                menu_opener_owner("runes_extra:forge", None),
                Some("horizon_java")
            );
            assert_eq!(
                menu_opener_owner("minecraft:hopper", Some("runes:altar")),
                Some("horizon_java")
            );
            assert_eq!(
                menu_opener_owner("minecraft:hopper", Some("minecraft:hopper")),
                None
            );
            assert_eq!(menu_opener_owner("plain:table", None), None);
            // Per-frame opener dispatch reads the pre-indexed table without querying the package
            // inventory again: clearing the test package inventory leaves the resolved owners
            // intact.
            crate::load_state::with_test_package_inventory(Vec::new(), || {
                assert_eq!(menu_opener_owner("runes:altar", None), Some("horizon_java"));
                assert_eq!(
                    menu_opener_owner("runes_extra:forge", None),
                    Some("horizon_java")
                );
                assert_eq!(
                    menu_opener_owner("minecraft:hopper", Some("runes:altar")),
                    Some("horizon_java")
                );
                assert_eq!(
                    menu_opener_owner("minecraft:hopper", Some("runes_extra:forge")),
                    Some("horizon_java")
                );
                assert_eq!(
                    menu_opener_owner("minecraft:hopper", Some("minecraft:hopper")),
                    None
                );
                assert_eq!(menu_opener_owner("plain:table", None), None);
            });
        });
    });

    let hijack_mods: crate::Mods = vec![crate::ModEntry::new(
        "third",
        Box::new(ThirdHijack { seen: seen.clone() }),
    )]
    .into();
    crate::load_state::with_test_package_inventory(packages(), || {
        crate::testing::with_mods(hijack_mods.clone(), || {
            let errors = crate::parts::tests::install_parts_fixture(&hijack_mods).unwrap_err();
            assert!(
                errors.iter().any(|e| e.contains("runes:altar")),
                "{errors:?}"
            );
        });
    });
}
