use super::*;
fn id(value: &str) -> Identifier {
    Identifier::parse(value).unwrap()
}
fn files() -> FileToIdConverter {
    FileToIdConverter::json("machines").unwrap()
}

#[test]
fn registry_views_and_explicit_order_before_vanilla() {
    let mut listeners = ClientListenerRegistry::new();
    let first = Arc::new(Boundary);
    let second = Arc::new(Boundary);
    listeners
        .add_listener(id("test_mod:first"), files(), first.clone())
        .unwrap();
    listeners
        .add_listener(id("test_mod:second"), files(), second.clone())
        .unwrap();
    assert_eq!(
        listeners.get_last_vanilla_listener(),
        VanillaClientListeners::model_manager()
    );
    assert_eq!(
        listeners.get_registry(),
        [
            VanillaClientListeners::first(),
            VanillaClientListeners::model_manager(),
            VanillaClientListeners::last(),
            id("test_mod:first"),
            id("test_mod:second"),
        ]
    );
    listeners
        .add_dependency(
            id("test_mod:second"),
            VanillaClientListeners::model_manager(),
        )
        .unwrap();
    assert!(listeners.get_graph().contains(&(
        id("test_mod:second"),
        VanillaClientListeners::model_manager()
    )));
    let sorted = listeners.finish().unwrap();
    assert_eq!(
        sorted.into_iter().map(|entry| entry.id).collect::<Vec<_>>(),
        [
            VanillaClientListeners::first(),
            id("test_mod:second"),
            VanillaClientListeners::model_manager(),
            id("test_mod:first"),
            VanillaClientListeners::last(),
        ]
    );
}

#[test]
fn reload_registration_reports_each_refusal_without_latching_but_a_cycle_fails() {
    thread_local! {
        static WARNINGS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    struct RefusalLog;
    impl log::Log for RefusalLog {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool {
            true
        }
        fn log(&self, record: &log::Record<'_>) {
            if record.level() == log::Level::Warn
                && record
                    .args()
                    .to_string()
                    .contains("unknown reload listener test_mod:repeated_missing")
            {
                WARNINGS.set(WARNINGS.get() + 1);
            }
        }
        fn flush(&self) {}
    }
    log::set_logger(&RefusalLog).unwrap();
    log::set_max_level(log::LevelFilter::Warn);

    let model = VanillaClientListeners::model_manager();
    let missing = id("test_mod:repeated_missing");
    let valid = id("test_mod:valid");
    let mut listeners = ClientListenerRegistry::new();
    for _ in 0..2 {
        assert_eq!(
            listeners.add_dependency(missing.clone(), model.clone()),
            Err(ReloadOrderError::UnknownListener {
                id: missing.clone()
            })
        );
    }
    assert_eq!(WARNINGS.get(), 2);
    listeners
        .add_listener(valid.clone(), files(), Arc::new(Boundary))
        .unwrap();
    listeners
        .add_dependency(model.clone(), valid.clone())
        .unwrap();
    let ordered = listeners.finish().unwrap();
    let mod_entry = ordered.iter().find(|entry| entry.id == valid).unwrap();
    let apply = mod_entry
        .listener
        .prepare(&ClientReloadContext::new(ReloadFiles::new(
            Default::default(),
        )))
        .unwrap();
    apply();

    let mut cyclic = ClientListenerRegistry::new();
    cyclic
        .add_listener(valid.clone(), files(), Arc::new(Boundary))
        .unwrap();
    cyclic.add_dependency(model.clone(), valid.clone()).unwrap();
    cyclic.add_dependency(valid, model).unwrap();
    assert!(matches!(
        cyclic.finish(),
        Err(ReloadOrderError::Cycle { .. })
    ));
}

#[test]
fn duplicate_objects_cannot_make_the_reverse_lookup_ambiguous() {
    let mut listeners = ClientListenerRegistry::new();
    let listener = Arc::new(Boundary);
    listeners
        .add_listener(id("one:listener"), files(), listener.clone())
        .unwrap();
    assert_eq!(
        listeners.add_listener(id("two:listener"), files(), listener),
        Err(ReloadOrderError::DuplicateObject {
            id: id("two:listener"),
            registered_as: id("one:listener"),
        })
    );
}
