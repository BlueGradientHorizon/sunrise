//! Validation at the listener boundary, before the next mod sees its edits.
use super::RecipePropertyAddition;

pub(crate) fn validate_recipe_properties(
    owner: &str,
    _previous: &[RecipePropertyAddition],
    additions: &mut Vec<RecipePropertyAddition>,
) {
    additions.retain(|addition| {
        if !addition.set.contains(':') {
            // Bare keys name vanilla sets; the engine checks that they exist.
            return true;
        }
        let valid = crate::Identifier::parse_namespaced(&addition.set).is_some();
        if !valid {
            log::warn!(
                "recipe_property_sets: {owner} supplied invalid set key {}",
                addition.set
            );
        }
        valid
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_valid_namespaced_or_bare_set_key_passes_and_malformed_keys_are_dropped() {
        let mut additions: Vec<_> = [
            "farmersdelight:cooking_pot_input",
            "ported_farmersdelight:input",
            "another:new",
            "minecraft:furnace_input",
            "minecraft:custom_set",
            "furnace_input",
            "BAD:input",
            "bad:",
            ":bad",
        ]
        .into_iter()
        .map(|key| RecipePropertyAddition::new(key, vec![]))
        .collect();
        validate_recipe_properties("ported_farmersdelight", &[], &mut additions);
        assert_eq!(
            additions.iter().map(|a| a.set.as_str()).collect::<Vec<_>>(),
            [
                "farmersdelight:cooking_pot_input",
                "ported_farmersdelight:input",
                "another:new",
                "minecraft:furnace_input",
                "minecraft:custom_set",
                "furnace_input",
            ]
        );
    }
}
