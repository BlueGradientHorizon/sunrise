use super::*;

impl ConsumeEffectsRegistrar for PartRegistrar {
    fn register_consume_effect_type(
        &mut self,
        id: crate::Identifier,
        ty: crate::ConsumeEffectType,
    ) -> Result<(), String> {
        let reason = if mod_vocab::__private::parts_installation::loaded_mods()
            .get(self.mod_id)
            .is_some_and(|package| package.side() != crate::Side::Both)
        {
            "consume effect types require a package on both sides"
        } else if matches!(id.namespace(), "minecraft" | "sunrise" | "c")
            || !owns_namespace(self.mod_id, id.namespace())
        {
            "consume effect type must use an owned, non-reserved namespace"
        } else if self.consume_effects.iter().any(|effect| effect.id == id) {
            "duplicate consume effect type"
        } else {
            self.consume_effects.push(RegisteredConsumeEffect {
                owner: self.mod_id,
                id,
                ty,
            });
            return Ok(());
        };
        let error = format!("mod {}: {reason} {id}", self.mod_id);
        self.refused.push(error.clone());
        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_ids_must_use_owned_non_reserved_namespaces() {
        #[derive(mod_vocab::Wire)]
        struct Payload {
            points: u8,
        }
        let codec = || {
            crate::ConsumeEffectType::new::<Payload>(
                |_| Ok(Payload { points: 1 }),
                |_| Ok(Default::default()),
            )
        };
        let mut registrar = PartRegistrar::new("owner");
        for key in [
            "other:new_effect",
            "minecraft:new_effect",
            "sunrise:new_effect",
            "c:new_effect",
        ] {
            assert!(
                registrar
                    .register_consume_effect_type(crate::Identifier::parse(key).unwrap(), codec())
                    .is_err(),
                "accepted {key}"
            );
        }
        assert!(
            registrar
                .register_consume_effect_type(
                    crate::Identifier::parse("owner:new_effect").unwrap(),
                    codec()
                )
                .is_ok()
        );
    }
}
