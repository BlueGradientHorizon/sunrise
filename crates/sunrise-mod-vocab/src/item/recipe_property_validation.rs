//! Scoped insertion and namespace validation for [`crate::RecipePropertySetsEvent`].
use super::RecipePropertyAddition;

impl crate::RecipePropertySetsEvent<'_> {
    /// Add items to an existing recipe property set or create a new custom set in one of this
    /// mod's owned namespaces (NeoForge's `RegisterRecipePropertiesEvent`).
    ///
    /// A bare key names a vanilla set (verified by the engine); a namespaced key in `minecraft`
    /// extends a vanilla set; any other namespaced key either extends a set already registered
    /// earlier or creates a new set when its namespace belongs to this listener's manifest.
    /// Invalid or foreign new keys are logged and skipped without altering earlier additions.
    pub fn register(&mut self, addition: RecipePropertyAddition) {
        self.register_in(&crate::LoadedMods::new(), addition);
    }

    fn register_in(&mut self, packages: &crate::LoadedMods, addition: RecipePropertyAddition) {
        let owner = self.mod_id;
        if let Some((namespace, _)) = addition.set.split_once(':') {
            let existing = self.additions.iter().any(|old| old.set == addition.set);
            // Keep ownership aligned with PartRegistrar::owned_declaration_id (parts/owned_namespaces.rs, #2617).
            let owned = packages.get(owner).map_or(namespace == owner, |package| {
                package.namespaces().any(|owned| owned == namespace)
            });
            let valid = crate::Identifier::parse_namespaced(&addition.set).is_some()
                && (namespace == "minecraft" || existing || owned);
            if !valid {
                log::warn!(
                    "recipe_property_sets: {owner} cannot create set {} outside its owned namespaces",
                    addition.set
                );
                return;
            }
        }
        self.additions.push(addition);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PackageInfo, PackageKind, Side};
    use std::sync::Arc;

    #[test]
    fn owned_manifest_namespace_can_create_but_foreign_new_key_cannot() {
        let packages = crate::LoadedMods {
            entries: Arc::from([PackageInfo::new(
                "ported_farmersdelight".into(),
                "1".into(),
                PackageKind::Code,
                Side::Both,
                vec!["ported_farmersdelight".into(), "farmersdelight".into()],
                vec![],
            )]),
        };
        let mut additions = vec![RecipePropertyAddition::new("another:known", vec![])];
        let mut event = crate::RecipePropertySetsEvent {
            mod_id: "ported_farmersdelight",
            additions: &mut additions,
        };
        for key in [
            "farmersdelight:cooking_pot_input",
            "ported_farmersdelight:input",
            "another:known",
            "another:new",
            "BAD:input",
            "minecraft:furnace_input",
        ] {
            event.register_in(&packages, RecipePropertyAddition::new(key, vec![]));
        }
        assert_eq!(
            additions.iter().map(|a| a.set.as_str()).collect::<Vec<_>>(),
            [
                "another:known",
                "farmersdelight:cooking_pot_input",
                "ported_farmersdelight:input",
                "another:known",
                "minecraft:furnace_input"
            ]
        );
    }
}
