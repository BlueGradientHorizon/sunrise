//! Scoped insertion and namespace validation for [`crate::RecipePropertySetsEvent`].
use super::RecipePropertyAddition;
use crate::Answered;

impl crate::RecipePropertySetsEvent<'_> {
    /// Add items to an existing recipe property set or create a new custom set in one of this
    /// mod's owned namespaces. The scoped operation follows NeoForge's
    /// `RegisterRecipePropertiesEvent`, but Sunrise unions contributions on each recipe load.
    ///
    /// A bare or `minecraft:` key names a vanilla set (verified by the engine). Any other
    /// namespaced key either extends a set registered earlier without changing who created it,
    /// or creates a new set when its namespace belongs to this listener's manifest.
    /// Invalid or foreign new keys are logged and skipped without altering earlier additions.
    pub fn register(&mut self, addition: RecipePropertyAddition) {
        let mod_id = self.mod_id;
        if addition.set.contains(':') {
            let Some(id) = crate::Identifier::parse_namespaced(&addition.set) else {
                log::warn!(
                    "recipe_property_sets: {mod_id}: invalid set key {}",
                    addition.set
                );
                return;
            };
            if id.namespace() != "minecraft"
                && !self
                    .additions
                    .iter()
                    .any(|old| old.answer.set == addition.set)
                && !crate::parts::owns_namespace(mod_id, id.namespace())
            {
                log::warn!(
                    "recipe_property_sets: {mod_id} cannot create set {} outside its owned namespaces",
                    addition.set
                );
                return;
            }
        }
        self.additions.push(Answered {
            answer: addition,
            mod_id,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PackageInfo, PackageKind, Side};

    #[test]
    fn owned_manifest_namespace_can_create_but_foreign_new_key_cannot() {
        let packages = vec![PackageInfo::new(
            "ported_farmersdelight".into(),
            "1".into(),
            PackageKind::Code,
            Side::Both,
            vec!["farmersdelight".into()],
            vec![],
        )];
        let mut additions = Vec::new();
        let mut event = crate::RecipePropertySetsEvent {
            mod_id: "another",
            additions: &mut additions,
        };
        event.register(RecipePropertyAddition::new("another:known", vec![]));
        event.mod_id = "ported_farmersdelight";
        crate::world::phase::with_test_package_inventory(packages, || {
            for key in [
                "farmersdelight:cooking_pot_input",
                "ported_farmersdelight:input",
                "another:known",
                "another:new",
                "BAD:input",
                "minecraft:furnace_input",
            ] {
                event.register(RecipePropertyAddition::new(key, vec![]));
            }
        });
        assert_eq!(
            additions
                .iter()
                .map(|a| (a.answer.set.as_str(), a.mod_id))
                .collect::<Vec<_>>(),
            [
                ("another:known", "another"),
                ("farmersdelight:cooking_pot_input", "ported_farmersdelight"),
                ("ported_farmersdelight:input", "ported_farmersdelight"),
                ("another:known", "ported_farmersdelight"),
                ("minecraft:furnace_input", "ported_farmersdelight"),
            ]
        );
    }
}
