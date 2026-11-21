mod api {
    #[derive(Debug, PartialEq, Eq)]
    pub struct RenderPipeline(pub &'static str);
}

#[test]
fn register_render_pipelines_dispatch_sets_listener_mod_id_on_attributed_event() {
    struct Listener;
    #[mod_api::__subscriptions]
    impl Hooks for Listener {
        fn register_render_pipelines(
            &self,
            _: &RegistrationContext,
            event: &mut RegisterRenderPipelinesEvent<'_>,
        ) {
            event
                .pipelines
                .push(mod_api::mod_api::RenderPipeline(event.mod_id));
        }
    }

    install(
        vec![
            ModEntry::new("first", Box::new(Listener)),
            ModEntry::new("second", Box::new(Listener)),
        ]
        .into(),
    );
    let mut pipelines = Vec::new();
    dispatch_register_render_pipelines(&RegistrationContext("bootstrap"), &mut pipelines);
    assert_eq!(
        pipelines,
        [
            mod_api::mod_api::RenderPipeline("first"),
            mod_api::mod_api::RenderPipeline("second"),
        ]
    );
}
