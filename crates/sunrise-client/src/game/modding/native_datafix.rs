//! Bridge the generated game Host implementation to the standalone data fixer.
use super::Game;
use mod_api::{Compound, NbtDataType, NbtUpgradeError};

impl Game<'_> {    pub(in crate::game) fn upgrade_nbt(
        kind: mod_api::NbtFixKind,
        from_version: i32,
        nbt: sunrise_core::nbt::Compound,
    ) -> Result<sunrise_core::nbt::Compound, mod_api::NbtFixError> {
        upgrade_nbt(kind, from_version, nbt)
    }
}


pub(in crate::game) fn upgrade_nbt(
    kind: NbtDataType,
    data_version: i32,
    nbt: Compound,
) -> Result<Compound, NbtUpgradeError> {
    sunrise_world::datafix::upgrade_compound(kind, data_version, nbt)
}

