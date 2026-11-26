#[cfg(any(test, feature = "test-support"))]
pub mod tests {

    /// The pack biome the saved-biome test enables: the fields the generator reads of every
    /// biome (climate, `carvers`; `features` is optional).
    pub const PACK_BIOME: &str = r##"{"temperature":0.8,"downfall":0.4,"has_precipitation":true,"effects":{"water_color":"#2301ef"},"carvers":[],"features":[]}"##;
}
