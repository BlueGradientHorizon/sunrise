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
    let model_manager = VanillaClientListeners::model_manager();
    assert_eq!(VanillaClientListeners::first(), model_manager);
    assert_eq!(VanillaClientListeners::last(), model_manager);
    assert_eq!(listeners.get_registry(), [model_manager.clone()]);
    assert!(listeners.get_graph().is_empty());

    let first = Arc::new(Boundary);
    let second = Arc::new(Boundary);
    let after = Arc::new(Boundary);
    listeners
        .add_listener(id("test_mod:first"), files(), first)
        .unwrap();
    listeners
        .add_listener(id("test_mod:second"), files(), second)
        .unwrap();
    listeners
        .add_listener(id("test_mod:after"), files(), after)
        .unwrap();
    assert_eq!(listeners.get_last_vanilla_listener(), model_manager);
    assert_eq!(
        listeners.get_registry(),
        [
            model_manager.clone(),
            id("test_mod:first"),
            id("test_mod:second"),
            id("test_mod:after"),
        ]
    );
    listeners
        .add_dependency(id("test_mod:second"), VanillaClientListeners::first())
        .unwrap();
    listeners
        .add_dependency(VanillaClientListeners::last(), id("test_mod:after"))
        .unwrap();
    assert_eq!(
        listeners.get_graph(),
        [
            (id("test_mod:second"), model_manager.clone()),
            (model_manager.clone(), id("test_mod:after")),
        ]
    );
    let sorted = listeners.finish().unwrap();
    assert_eq!(
        sorted.into_iter().map(|entry| entry.id).collect::<Vec<_>>(),
        [
            id("test_mod:second"),
            model_manager,
            id("test_mod:first"),
            id("test_mod:after"),
        ]
    );
}

#[test]
fn mixed_graph_leaves_minecraft_and_connected_listeners_in_registration_order() {
    let mut listeners = ClientListenerRegistry::new();
    let model_manager = VanillaClientListeners::model_manager();
    let extra = id("minecraft:extra");
    let after_extra = id("demo:after_extra");
    let before = id("demo:before");
    let dangling = id("demo:dangling");
    for name in [&extra, &after_extra, &before, &dangling] {
        listeners
            .add_listener(name.clone(), files(), Arc::new(Boundary))
            .unwrap();
    }
    listeners
        .add_dependency(extra.clone(), after_extra.clone())
        .unwrap();
    listeners
        .add_dependency(before.clone(), model_manager.clone())
        .unwrap();
    let sorted = listeners.finish().unwrap();
    assert_eq!(
        sorted.into_iter().map(|entry| entry.id).collect::<Vec<_>>(),
        [extra, after_extra, before, model_manager, dangling]
    );
}

#[test]
fn handled_registration_error_does_not_poison_finish() {
    let mut listeners = ClientListenerRegistry::new();
    let ok = id("test_mod:ok");
    listeners
        .add_listener(ok.clone(), files(), Arc::new(Boundary))
        .unwrap();
    assert_eq!(
        listeners
            .add_dependency(
                id("mistyped:listener"),
                VanillaClientListeners::model_manager()
            )
            .unwrap_err(),
        ReloadOrderError::UnknownListener {
            id: id("mistyped:listener")
        }
    );
    assert_eq!(
        listeners
            .add_listener(ok.clone(), files(), Arc::new(Boundary))
            .unwrap_err(),
        ReloadOrderError::DuplicateListener { id: ok.clone() }
    );
    assert!(listeners.get_graph().is_empty());
    let sorted = listeners.finish().unwrap();
    assert_eq!(
        sorted.into_iter().map(|entry| entry.id).collect::<Vec<_>>(),
        [VanillaClientListeners::model_manager(), ok]
    );
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
