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
        } else if is_builtin_consume_effect(&id.to_string())
            || self.consume_effects.iter().any(|effect| effect.id == id)
        {
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
