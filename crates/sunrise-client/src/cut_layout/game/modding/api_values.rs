
#[path = "api_values/small_values.rs"]
pub(crate) mod small_values;

#[cfg(test)]
pub(crate) fn resolve_cached_block_definitions(
    cached_registry: &'static sunrise_world::registry::BlockRegistry,
) -> Result<&'static sunrise_world::registry::BlockRegistry, String> {
    if !mod_api::__private::parts_installation::parts_installed() {
        mod_platform::__private::part_tests::install_parts_fixture(&[])
            .map_err(|errors| errors.join("\n"))?;
    }
    sunrise_gameplay_common::game::api_values::resolve_cached_block_definitions(cached_registry)
}

#[cfg(test)]
pub(crate) fn install_test_platform() {
    sunrise_gameplay_common::install_test_mod_platform!(mod_platform);
}

#[cfg(test)]
pub(crate) fn register_game_keys(ids: impl IntoIterator<Item = String>) {
    let mod_fluids = sunrise_world::fluid::mod_fluids();
    let mod_fluids: Vec<&str> = mod_fluids
        .iter()
        .flat_map(|(_, fluid)| [fluid.id.as_str(), fluid.flowing_id.as_str()])
        .collect();
    sunrise_gameplay_common::game::api_values::with_rule_schema(|schema| {
        mod_platform::__private::part_tests::register_game_keys(
            ids,
            &mod_fluids,
            schema,
            sunrise_gameplay_common::game::native_dimensions::fixture_valid_shape,
        )
    });
    mod_api::__private::resolve_effects(
        sunrise_game_data::potions::all_effects()
            .map(|def| sunrise_game_data::potions::namespaced(def.id)),
        |id| sunrise_game_data::potions::effect(id).map(|def| def.index),
    )
    .unwrap();
    mod_api::__private::validate_effect_part_owners(|id| {
        sunrise_game_data::potions::effect(id)?.owner
    })
    .unwrap();
    if let Some(types) = sunrise_game_data::block_entity_types::current() {
        // Registration just replaced the mod snapshot, but the process-wide engine
        // table can still belong to an earlier fixture. Include this fixture's
        // declarations before resolving its requested keys, as content loading does.
        let declared = mod_api::__private::block_entity_types();
        mod_api::__private::resolve_block_entity_types(
            types
                .names()
                .chain(declared.iter().map(|(_, kind)| kind.id.as_str())),
            |id| {
                (types.key(id).is_some() || declared.iter().any(|(_, kind)| kind.id == id))
                    .then_some(())
            },
        )
        .unwrap();
    }
}
