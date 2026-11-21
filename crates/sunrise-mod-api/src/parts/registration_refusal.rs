//! Native registration savepoint and rollback.
use super::*;

impl PartRegistrar {
    pub(crate) fn registration_checkpoint(&self) -> impl FnOnce(&mut Self, bool) + use<> {
        let parts = self.parts.clone();
        let part_states = self.part_states.clone();
        let attachment_types = self.attachment_types.clone();
        let tags = self.tags.clone();
        let keys = self.registry_keys.borrow().clone();
        let menu_screen_factories = self.menu_screen_factories.clone();
        let menu_recipe_books = self.menu_recipe_books.clone();
        let menu_slot_exclusions = self.menu_slot_exclusions.clone();
        let recipe_book_types = self.recipe_book_types.clone();
        let recipe_book_fields = self.recipe_book_fields.clone();
        let recipe_book_filters = self.recipe_book_filters.clone();
        let registered = self.registered;
        let entity_selector_bytes = self.entity_selector_bytes;
        let block_entity_transforms = self.block_entity_transforms.clone();
        // Append-only tables retain previous mods' entries without requiring
        // Clone on runtime callback objects or their eventual frozen caches.
        macro_rules! checkpoint {
            ($($field:ident),* $(,)?) => {{
                $(let $field = self.$field.len();)*
                move |registrar: &mut Self, mut reported| {
                    $(let $field = registrar.$field.split_off($field);)*
                    let retired_transforms = std::mem::replace(&mut registrar.block_entity_transforms, block_entity_transforms);
                    let retired_parts = std::mem::replace(&mut registrar.parts, parts);
                    let retired_tags = std::mem::replace(&mut registrar.tags, tags);
                    let retired_screens = std::mem::replace(&mut registrar.menu_screen_factories, menu_screen_factories);
                    registrar.part_states = part_states;
                    registrar.attachment_types = attachment_types;
                    *registrar.registry_keys.borrow_mut() = keys;
                    registrar.menu_recipe_books = menu_recipe_books;
                    registrar.menu_slot_exclusions = menu_slot_exclusions;
                    registrar.recipe_book_types = recipe_book_types;
                    registrar.recipe_book_fields = recipe_book_fields;
                    registrar.recipe_book_filters = recipe_book_filters;
                    registrar.registered = registered;
                    registrar.entity_selector_bytes = entity_selector_bytes;

                    // The checkpoint is fully restored before destroying even one value.
                    // Drop individual owned entries, never a collection containing multiple
                    // mod destructors that could panic while unwinding the first destructor.
                    let owner = registrar.mod_id;
                    macro_rules! discard {
                        ($value:expr) => {
                            mod_vocab::__private::discard_startup_registration(
                                owner, $value, &mut reported,
                            )
                        };
                    }
                    $(for entry in $field { discard!(entry); })*
                    for entry in retired_transforms.into_values() { discard!(entry); }
                    for entry in retired_screens.into_values().flat_map(HashMap::into_values) {
                        discard!(entry);
                    }
                    for (_, part) in retired_tags.blocks { discard!(part); }
                    for (_, part) in retired_tags.items { discard!(part); }
                    for part in retired_parts.blocks.into_values().flatten() { discard!(part); }
                    for part in retired_parts.block_entities.into_values().flatten() { discard!(part); }
                    for part in retired_parts.block_entity_kinds.into_values().flatten() { discard!(part); }
                    for part in retired_parts.items.into_values().flatten() { discard!(part); }
                    for part in retired_parts.client_item_uses.into_values() { discard!(part); }
                    for part in retired_parts.menus.into_values().flatten() { discard!(part); }
                    for part in retired_parts.effects.into_values().flatten() { discard!(part); }
                    for part in retired_parts.fluids.into_values().flatten() { discard!(part); }
                    for part in retired_parts.entities.into_values().flatten() { discard!(part); }
                    for part in retired_parts.capabilities.into_values().flat_map(HashMap::into_values).flatten() { discard!(part); }
                    for part in retired_parts.block_capabilities.into_values().flat_map(HashMap::into_values).flatten() { discard!(part); }
                    for part in retired_parts.item_capabilities.into_values().flat_map(HashMap::into_values).flatten() { discard!(part); }
                    for part in retired_parts.entity_capabilities.into_values().flat_map(HashMap::into_values).flatten() { discard!(part); }
                    for part in retired_parts.default_item_containers.into_values().flatten() { discard!(part); }
                    for part in retired_parts.place_providers.into_values() { discard!(part); }
                    for part in retired_parts.tooltip_appenders { discard!(part); }
                    for part in retired_parts.crash_sections { discard!(part); }
                    for part in retired_parts.command_arguments { discard!(part); }
                    for part in retired_parts.recipe_serializers.into_values() { discard!(part); }
                    for part in retired_parts.entity_selectors { discard!(part); }
                    if let Some(payloads) = retired_parts.payloads.and_then(Arc::into_inner) {
                        for payload in payloads.phases.into_iter().flat_map(HashMap::into_values) {
                            discard!(payload.server);
                            discard!(payload.client);
                        }
                        for payload in payloads.raw.into_values().flatten() {
                            discard!(payload.server);
                        }
                    }
                }
            }};
        }
        checkpoint!(
            biome_modifiers,
            datapack_registries,
            attribute_widenings,
            game_rule_categories,
            game_rules,
            custom_stats,
            entity_tick_each,
            attachment_uses,
            block_entity_types,
            entity_attributes,
            general_entity_types,
            world_fluids,
            fluid_interactions,
            entity_attribute_modifications,
            attributes,
            spawn_placements,
            entity_part_states,
            biome_regions,
            controllers,
            arm_poses,
            structure_types,
            feature_types,
            loot_modifier_serializers,
            loot_function_types,
            loot_condition_types,
            dimensions,
            chunk_generators,
            biome_sources,
            refused
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{Hooks, ModEntry, Registrar, RegistrationContext};

    struct Callback;
    impl crate::BlockPart for Callback {
        fn block_subscribed(&self) -> &'static [Option<crate::HookId>] {
            &[]
        }
    }
    impl crate::BlockEntityPart for Callback {
        fn block_entity_subscribed(&self) -> &'static [Option<crate::HookId>] {
            &[]
        }
    }

    fn region() -> crate::BiomeRegion {
        crate::BiomeRegion::new(
            "minecraft:overworld".into(),
            5,
            vec![crate::BiomePoint {
                biome: "minecraft:plains".into(),
                temperature: (0.0, 0.0),
                humidity: (0.0, 0.0),
                continentalness: (0.0, 0.0),
                erosion: (0.0, 0.0),
                depth: (0.0, 0.0),
                weirdness: (0.0, 0.0),
                offset: 0.0,
            }],
        )
    }

    struct Refuses;
    impl Hooks for Refuses {
        fn subscribed(&self) -> &'static [Option<crate::HookId>] {
            &[]
        }
        fn register(&self, r: &mut dyn Registrar, cx: &RegistrationContext) {
            r.recipe_book_type_with_fields("broken:book", "reused_open", "reused_filter")
                .unwrap();
            r.declare_fluid("before");
            r.declare_biome_region("rejected", region()).unwrap();
            let kind = r
                .block_entity_type("callback")
                .blocks(&[])
                .transform(|data: &crate::Compound, _: crate::StructureTransform| Ok(data.clone()))
                .build();
            r.block_entity_part(&kind, Callback).unwrap();
            cx.refuse("bootstrap failed: café\nmissing bridge");
            cx.refuse("ignored");
            cx.warn("test.warn.bridge", &["missing"]);
            r.declare_fluid("foreign:after");
        }
    }
    struct Healthy;
    impl Hooks for Healthy {
        fn subscribed(&self) -> &'static [Option<crate::HookId>] {
            &[]
        }
        fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
            r.declare_fluid("soup");
            r.declare_biome_region("kept", region()).unwrap();
        }
    }

    struct Warns;
    impl Hooks for Warns {
        fn subscribed(&self) -> &'static [Option<crate::HookId>] {
            &[]
        }
        fn register(&self, r: &mut dyn Registrar, cx: &RegistrationContext) {
            // Diagnostics follow the borrowed context, not the dispatch thread's scope.
            std::thread::scope(|scope| {
                scope
                    .spawn(|| cx.warn("test.warn.optional", &["jei"]))
                    .join()
                    .unwrap();
            });
            r.declare_fluid("soup");
        }
    }

    #[test]
    fn a_warning_keeps_the_mod_loading_and_is_published_for_it() {
        let mods = [ModEntry::new("warns", Box::new(Warns))];
        let registrar = super::super::register_all(&mods, |_| false);
        assert!(registrar.refused.is_empty());
        assert!(
            registrar
                .registry_keys
                .borrow()
                .declared_fluids
                .contains_key("warns:soup")
        );
        let [(owner, warnings)] = registrar.warnings.as_slice() else {
            panic!("expected warnings from one mod");
        };
        assert_eq!(*owner, "warns");
        let [warning] = warnings.as_slice() else {
            panic!("expected one warning");
        };
        assert_eq!(warning.key(), "test.warn.optional");
        assert_eq!(warning.args(), ["jei"]);
    }

    struct WarnsPastTheCap;
    impl Hooks for WarnsPastTheCap {
        fn subscribed(&self) -> &'static [Option<crate::HookId>] {
            &[]
        }
        fn register(&self, _: &mut dyn Registrar, cx: &RegistrationContext) {
            for i in 0..18 {
                cx.warn("test.warn.many", &[&i.to_string()]);
            }
        }
    }

    #[test]
    fn warnings_are_published_under_the_mod_that_gave_them() {
        let mods = [
            ModEntry::new("warns", Box::new(Warns)),
            ModEntry::new("quiet", Box::new(Healthy)),
            ModEntry::new("many", Box::new(WarnsPastTheCap)),
        ];
        crate::__private::install_parts(&mods, &crate::parts::tests::rule_schema(), |_| false)
            .unwrap();
        let warns = crate::__private::registration_warnings("warns");
        assert_eq!(warns.len(), 1);
        assert_eq!(warns[0].key(), "test.warn.optional");
        assert_eq!(warns[0].args(), ["jei"]);
        assert!(crate::__private::registration_warnings("quiet").is_empty());
        let many = crate::__private::registration_warnings("many");
        assert_eq!(many.len(), 16);
        assert_eq!(many[0].key(), "test.warn.many");
        assert_eq!(many[0].args(), ["0"]);
        assert_eq!(many[15].args(), ["15"]);
    }

    #[test]
    fn refusal_rolls_back_only_its_owner_and_preserves_the_reason() {
        let mods = [
            ModEntry::new("healthy", Box::new(Healthy)),
            ModEntry::new("broken", Box::new(Refuses)),
            ModEntry::new("later", Box::new(Healthy)),
        ];
        let mut registrar = super::super::register_all(&mods, |_| false);
        assert!(!registrar.recipe_book_types.contains_key("broken:book"));
        assert!(registrar.recipe_book_fields.is_empty());
        registrar.mod_id = "later";
        crate::RecipeBooksRegistrar::recipe_book_type_with_fields(
            &mut registrar,
            "later:book",
            "reused_open",
            "reused_filter",
        )
        .unwrap();
        assert_eq!(
            registrar.refused,
            ["mod broken refused registration: bootstrap failed: café\nmissing bridge"]
        );
        assert_eq!(registrar.warnings.len(), 1);
        let (owner, warnings) = &registrar.warnings[0];
        assert_eq!(*owner, "broken");
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].key(), "test.warn.bridge");
        assert_eq!(warnings[0].args(), ["missing"]);
        assert_eq!(
            registrar
                .biome_regions
                .iter()
                .map(|d| (d.mod_id.as_str(), d.id.as_str(), d.region.weight))
                .collect::<Vec<_>>(),
            [("healthy", "healthy:kept", 5), ("later", "later:kept", 5)]
        );
        assert!(registrar.block_entity_types.is_empty());
        assert!(registrar.block_entity_transforms.is_empty());
        assert!(registrar.parts.block_entity_kinds.is_empty());
        let keys = registrar.registry_keys.borrow();
        assert!(keys.declared_fluids.contains_key("healthy:soup"));
        assert!(keys.declared_fluids.contains_key("later:soup"));
        assert!(!keys.declared_fluids.contains_key("broken:before"));
        assert!(!keys.declared_fluids.contains_key("foreign:after"));
    }

    #[test]
    fn panic_in_hooks_register_rolls_back_only_its_owner_and_queues_report() {
        struct Panics;
        impl Hooks for Panics {
            fn subscribed(&self) -> &'static [Option<crate::HookId>] {
                &[]
            }
            fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
                r.declare_fluid("before");
                r.declare_biome_region("aborted", region()).unwrap();
                panic!("boom during registration");
            }
        }
        let mods: crate::Mods = vec![
            ModEntry::new("healthy", Box::new(Healthy)),
            ModEntry::new("broken", Box::new(Panics)),
            ModEntry::new("later", Box::new(Healthy)),
        ]
        .into();
        crate::testing::with_dispatch(mods, || {
            let registrar = super::super::register_all(mod_vocab::__private::mods(), |_| false);
            assert!(registrar.refused.is_empty());
            assert_eq!(
                registrar
                    .biome_regions
                    .iter()
                    .map(|d| (d.mod_id.as_str(), d.id.as_str(), d.region.weight))
                    .collect::<Vec<_>>(),
                [("healthy", "healthy:kept", 5), ("later", "later:kept", 5)]
            );
            let keys = registrar.registry_keys.borrow();
            assert!(keys.declared_fluids.contains_key("healthy:soup"));
            assert!(keys.declared_fluids.contains_key("later:soup"));
            assert!(!keys.declared_fluids.contains_key("broken:before"));
            assert_eq!(
                mod_vocab::__private::take_hook_panics(),
                [mod_vocab::__private::HookPanicReport {
                    mod_id: "broken",
                    hook: "register",
                }]
            );
        });
    }
}
