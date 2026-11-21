//! Concrete authoring extracted verbatim from the vocabulary layer.
use super::*;

/// Vanilla 26.3 `RegistryDataLoader` datapack registries (`WORLD_REGISTRIES`,
/// `DIMENSION_REGISTRIES`, `RELOADABLE_REGISTRIES`).
const BUILTIN_DATAPACK_REGISTRIES: &[&str] = &[
    "minecraft:dimension_type",
    "minecraft:worldgen/biome",
    "minecraft:chat_type",
    "minecraft:worldgen/carver",
    "minecraft:worldgen/feature",
    "minecraft:worldgen/placed_feature",
    "minecraft:worldgen/structure",
    "minecraft:worldgen/structure_set",
    "minecraft:worldgen/processor_list",
    "minecraft:worldgen/template_pool",
    "minecraft:worldgen/noise_settings",
    "minecraft:worldgen/noise",
    "minecraft:worldgen/density_function",
    "minecraft:worldgen/material_rule",
    "minecraft:worldgen/material_condition",
    "minecraft:worldgen/world_preset",
    "minecraft:worldgen/flat_level_generator_preset",
    "minecraft:trim_pattern",
    "minecraft:trim_material",
    "minecraft:trial_spawner",
    "minecraft:wolf_variant",
    "minecraft:wolf_sound_variant",
    "minecraft:pig_variant",
    "minecraft:pig_sound_variant",
    "minecraft:frog_variant",
    "minecraft:cat_variant",
    "minecraft:cat_sound_variant",
    "minecraft:cow_variant",
    "minecraft:cow_sound_variant",
    "minecraft:sulfur_cube_archetype",
    "minecraft:chicken_variant",
    "minecraft:chicken_sound_variant",
    "minecraft:zombie_nautilus_variant",
    "minecraft:painting_variant",
    "minecraft:damage_type",
    "minecraft:worldgen/multi_noise_biome_source_parameter_list",
    "minecraft:banner_pattern",
    "minecraft:enchantment",
    "minecraft:enchantment_provider",
    "minecraft:jukebox_song",
    "minecraft:instrument",
    "minecraft:test_environment",
    "minecraft:test_instance",
    "minecraft:dialog",
    "minecraft:world_clock",
    "minecraft:timeline",
    "minecraft:villager_trade",
    "minecraft:trade_set",
    "minecraft:decorated_pot_pattern",
    "minecraft:block_transformer",
    "minecraft:worldgen/block_state_provider",
    "minecraft:dimension",
    "minecraft:loot_table",
    "minecraft:predicate",
    "minecraft:context_float_provider",
    "minecraft:context_int_provider",
    "minecraft:item_modifier",
    "minecraft:slot_source",
    "minecraft:advancement",
    "minecraft:recipe",
];

impl DatapackRegistriesRegistrar for PartRegistrar {
    fn declare_datapack_registry(
        &mut self,
        path: &str,
        sync: DatapackRegistrySync,
        mut fields: Vec<DatapackField>,
    ) -> Result<(), DatapackRegistrationError> {
        let result = (|| {
            if mod_vocab::__private::parts_installation::loaded_mods()
                .get(self.mod_id)
                .is_some_and(|package| package.side() != crate::Side::Both)
            {
                return Err(DatapackRegistrationError::WrongSide);
            }
            let key = qualified(self.mod_id, path);
            if !mod_vocab::__private::parts_installation::valid_datapack_id(&key) {
                return Err(DatapackRegistrationError::InvalidKey);
            }
            if BUILTIN_DATAPACK_REGISTRIES.contains(&key.as_str())
                || self.has_builtin_registry(&key)
                || self
                    .datapack_registries
                    .iter()
                    .any(|declared| declared.key == key)
            {
                return Err(DatapackRegistrationError::Duplicate);
            }
            for field in &mut fields {
                if matches!(
                    field.kind,
                    crate::DatapackFieldKind::Identifier
                        | crate::DatapackFieldKind::Reference { .. }
                ) && let Some(crate::DatapackValue::Text(value)) = &mut field.default
                {
                    *value = crate::Identifier::parse(value)
                        .map_err(|_| DatapackRegistrationError::InvalidDefault)?
                        .to_string();
                }
            }
            mod_vocab::__private::parts_installation::validate_datapack_schema(&fields)?;
            self.datapack_registries.push(DatapackRegistryDeclaration {
                mod_id: self.mod_id.to_owned(),
                key,
                fields,
                sync,
            });
            Ok(())
        })();
        if let Err(error) = &result {
            self.refused.push(format!(
                "mod {} datapack registry {path:?}: {error:?}",
                self.mod_id
            ));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatapackFieldKind;

    fn power() -> Vec<DatapackField> {
        vec![DatapackField::required(
            "power",
            DatapackFieldKind::Int { min: 0, max: 100 },
        )]
    }

    /// Bare paths qualify with the declaring mod; valid explicit namespaces (including
    /// non-colliding `minecraft:` keys) stand under the declaring mod while malformed keys,
    /// duplicates, built-in registries and bad schemas refuse, naming the mod.
    #[test]
    pub(crate) fn datapack_registry_declarations_validate() {
        use DatapackRegistrationError as E;
        let data = mod_vocab::__private::RegistrationData::new(
            [("minecraft:block".into(), vec!["minecraft:stone".into()])].into(),
            Default::default(),
            Default::default(),
            crate::RuntimeEnvironment::new(
                crate::Dist::Client,
                "26.3".into(),
                "test".into(),
                false,
            ),
        );
        crate::load_state::with_test_registration_data(data, || {
            let mut registrar = PartRegistrar::new("example");
            let sync = DatapackRegistrySync::Required;
            registrar
                .declare_datapack_registry("spell", sync, power())
                .unwrap();
            assert_eq!(
                registrar.datapack_registries[0].key, "example:spell",
                "a bare path is in the declaring mod's namespace"
            );
            assert_eq!(registrar.datapack_registries[0].mod_id, "example");
            registrar
                .declare_datapack_registry("addon:spell", sync, power())
                .unwrap();
            assert_eq!(registrar.datapack_registries[1].key, "addon:spell");
            assert_eq!(registrar.datapack_registries[1].mod_id, "example");
            registrar
                .declare_datapack_registry("minecraft:spell", sync, power())
                .unwrap();
            assert_eq!(registrar.datapack_registries[2].key, "minecraft:spell");
            assert_eq!(registrar.datapack_registries[2].mod_id, "example");
            assert_eq!(
                registrar.declare_datapack_registry("example:spell", sync, power()),
                Err(E::Duplicate)
            );
            assert_eq!(
                registrar.declare_datapack_registry("addon:spell", sync, power()),
                Err(E::Duplicate)
            );
            assert_eq!(
                registrar.declare_datapack_registry("minecraft:damage_type", sync, power()),
                Err(E::Duplicate)
            );
            assert_eq!(
                registrar.declare_datapack_registry("minecraft:block", sync, power()),
                Err(E::Duplicate)
            );
            assert_eq!(
                registrar.declare_datapack_registry("Not A Key", sync, power()),
                Err(E::InvalidKey)
            );
            assert_eq!(
                registrar.declare_datapack_registry("", sync, power()),
                Err(E::InvalidKey)
            );
            let mut bad = power();
            bad.push(bad[0].clone());
            assert_eq!(
                registrar.declare_datapack_registry("other", sync, bad),
                Err(E::InvalidFieldName)
            );
            // A refused schema leaves no half-declared registry behind.
            assert_eq!(registrar.datapack_registries.len(), 3);
            assert_eq!(registrar.refused.len(), 7);
            assert!(
                registrar
                    .refused
                    .iter()
                    .all(|refusal| refusal.contains("mod example"))
            );
        });
    }

    #[test]
    fn datapack_declarations_round_trip_the_wire() {
        use crate::wire::{Reader, Wire, Writer};
        let declaration = DatapackRegistryDeclaration {
            mod_id: "example".into(),
            key: "example:spell".into(),
            fields: power(),
            sync: DatapackRegistrySync::Optional,
        };
        let mut writer = Writer::new();
        declaration.put(&mut writer);
        let bytes = writer.into_bytes();
        assert_eq!(
            DatapackRegistryDeclaration::take(&mut Reader::new(&bytes)).unwrap(),
            declaration
        );
    }
}

#[cfg(test)]
mod default_tests {
    use super::*;
    #[test]
    fn registered_identifier_defaults_match_explicit_values() {
        for kind in [
            crate::DatapackFieldKind::Identifier,
            crate::DatapackFieldKind::Reference {
                registry: "minecraft:item".into(),
            },
        ] {
            let mut registrar = PartRegistrar::new("defaults");
            registrar
                .declare_datapack_registry(
                    "entry",
                    DatapackRegistrySync::default(),
                    vec![DatapackField::with_default(
                        "id",
                        kind,
                        crate::DatapackValue::Text("ice".into()),
                    )],
                )
                .unwrap();
            let declaration = &registrar.datapack_registries[0];
            assert_eq!(
                declaration.decode_entry("{}").unwrap(),
                declaration.decode_entry(r#"{"id":"ice"}"#).unwrap()
            );
        }
    }
}
