#[test]
fn recipe_property_sets_dispatch_sets_listener_mod_id_on_attributed_event() {
    struct Listener(Arc<Mutex<Vec<&'static str>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Listener {
        fn recipe_property_sets(
            &self,
            _: &dyn DataLoadContext,
            event: &mut RecipePropertySetsEvent<'_>,
        ) {
            self.0.lock().unwrap().push(event.mod_id);
            event.additions.push(RecipePropertyAddition);
        }
    }

    let seen = Arc::new(Mutex::new(Vec::new()));
    install(
        vec![
            ModEntry::new("pot", Box::new(Listener(Arc::clone(&seen)))),
            ModEntry::new("extension", Box::new(Listener(Arc::clone(&seen)))),
        ]
        .into(),
    );
    let mut additions = Vec::new();
    dispatch_recipe_property_sets(&Load("server"), &mut additions);
    assert_eq!(*seen.lock().unwrap(), ["pot", "extension"]);
    assert_eq!(additions.len(), 2);
}
