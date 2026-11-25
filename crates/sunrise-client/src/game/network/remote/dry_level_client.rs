use super::*;
use crate::game::mod_api_host::{FireOddsLevel, PredictedLevel};
use crate::game::remote::dry_level::DryLevel;
use crate::game::remote::dry_level::PredictionBase;
use std::collections::HashSet;

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
