//! Bridge the shared host's standalone upgrade to the world's existing datafix chain.
use super::Game;
use mod_api::{Compound, NbtDataType, NbtUpgradeError};
use sunrise_world::datafix::{self, CompoundType, UpgradeError};

impl Game<'_> {
    pub(in crate::game) fn upgrade_nbt(
        kind: NbtDataType,
        data_version: i32,
        nbt: Compound,
    ) -> Result<Compound, NbtUpgradeError> {
        let kind = match kind {
            NbtDataType::ItemStack => CompoundType::ItemStack,
            NbtDataType::BlockState => CompoundType::BlockState,
            NbtDataType::BlockEntity => CompoundType::BlockEntity,
            NbtDataType::Entity => CompoundType::Entity,
            NbtDataType::EntityTree => CompoundType::EntityTree,
        };
        datafix::upgrade_compound(kind, data_version, nbt).map_err(|error| match error {
            UpgradeError::NegativeVersion(v) | UpgradeError::UnsupportedVersion(v) => {
                NbtUpgradeError::UnsupportedVersion(v)
            }
            UpgradeError::FutureVersion(v) => NbtUpgradeError::FutureVersion(v),
            UpgradeError::MalformedCompound => NbtUpgradeError::MalformedCompound,
            UpgradeError::LegacyTablesUnavailable => NbtUpgradeError::LegacyTablesUnavailable,
            UpgradeError::TooDeep => NbtUpgradeError::TooDeep,
            UpgradeError::UpgradedTooDeep => NbtUpgradeError::UpgradedTooDeep,
        })
    }
}
