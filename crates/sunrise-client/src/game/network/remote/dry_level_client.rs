#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mod_api_host::{FireOddsLevel, PredictedLevel};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::dry_level::DryLevel;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::dry_level::PredictionBase;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::dry_level::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{cell::RefCell, collections::HashSet};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    connection_kind::ConnectionKind, geometry::Direction, position::BlockPos,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::level::{
    BlockId, LevelAccess, PlacementContext, Ticks,
};

impl PredictionBase for crate::game::GameLevel<'_> {
    fn fire_odds_level<'a>(
        &'a self,
        prediction: &'a dyn PredictedLevel,
    ) -> Option<FireOddsLevel<'a>> {
        Some(FireOddsLevel {
            base: self,
            prediction: Some(prediction),
        })
    }
}

impl<'w> DryLevel<'w> {
    pub(in crate::game) fn new(base: &'w dyn PredictionBase) -> Self {
        Self {
            base,
            changed: HashMap::new(),
            removed_entities: HashSet::new(),
            notice_order: None,
            random: sunrise_core::random::LegacyRandom::new(0),
            sounds: Vec::new(),
            filled_container: false,
            reads: None,
            asks_mods: true,
        }
    }
}
