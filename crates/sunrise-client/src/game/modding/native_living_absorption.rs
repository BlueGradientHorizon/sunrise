//! A living entity's absorption amount read for native mods (`LivingEntity.getAbsorptionAmount`).
use super::Game;
use mod_api::LivingRef;

impl Game<'_> {
    /// The entity's absorption amount (`LivingEntity.getAbsorptionAmount`).
    pub(in crate::game) fn living_absorption(&self, living: LivingRef) -> Option<f32> {
        self.side_living_absorption(living)
    }
}
