//! Mod-owned world fluid registration values.
/// Simulation and bucket settings for a declared world fluid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub struct WorldFluid {
    /// Ticks between flow steps (NeoForge's `BaseFlowingFluid.Properties.tickRate`).
    pub tick_delay: i32,
    /// Amount lost per block, 0 or more (`levelDecreasePerBlock`).
    pub drop_off: i32,
    /// Search distance for a way down (`slopeFindDistance`).
    pub slope_find_distance: i32,
    /// Whether two sources can create another source.
    pub source_conversion: bool,
    /// Whether a filled bucket is registered.
    pub bucket: bool,
}
impl Default for WorldFluid {
    fn default() -> Self {
        Self {
            tick_delay: 5,
            drop_off: 1,
            slope_find_distance: 4,
            source_conversion: false,
            bucket: true,
        }
    }
}

impl WorldFluid {
    /// Refuses a negative `drop_off`: the level a spread computes (`FlowingFluid.getNewLiquid`,
    /// the highest neighbour less the drop-off) must stay within `FlowingFluid.LEVEL`'s 1 to 8.
    /// Every other value is NeoForge's `BaseFlowingFluid.Properties` domain, any `int`.
    pub fn check(&self) -> Result<(), String> {
        if self.drop_off < 0 {
            return Err("drop_off is 0 or more".into());
        }
        Ok(())
    }
}
