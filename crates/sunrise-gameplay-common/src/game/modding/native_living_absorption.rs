//! A living entity's absorption amount read for native mods (`LivingEntity.getAbsorptionAmount`).
use crate::game::CommonGame;
use mod_api::LivingRef;

impl CommonGame<'_> {
    /// The entity's absorption amount (`LivingEntity.getAbsorptionAmount`).
    pub(in crate::game) fn living_absorption(&self, living: LivingRef) -> Option<f32> {
        self.side_living_absorption(living)
    }
}

use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;
