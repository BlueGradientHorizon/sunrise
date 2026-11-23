//! `MobEvent::Convert`: `Mob.convertTo` with `ConversionType.SINGLE` and the
//! `ConversionParams.AfterConversion` step the converting mob asked for.
use super::*;
use sunrise_client::mobs::conversion;

impl Game<'_> {
    /// A mob turned into `kind` in its place (`conversion::single`). The
    /// after-step (`Pig.thunderHit`'s gear and persistence, the finalization of
    /// `Villager.thunderHit` and `Tadpole.ageUp`, the nausea a zombifying
    /// piglin or hoglin leaves behind) runs inside [`Game::convert_to`], before
    /// the new mob joins, by what the mob converted from and into; `after`
    /// only says whether the caller already asked `LivingConversionEvent.Pre`.
    pub(super) fn convert_event(
        &mut self,
        from: &Mob,
        kind: &str,
        params: conversion::Params,
        after: conversion::After,
    ) {
        // A tick conversion is queued from inside the mob's AI step: the new mob takes the
        // place the mob had then, before its travel.
        self.convert_to_moved(
            from,
            kind,
            params,
            after.pre_checked(),
            Some(from),
            |_, _| {},
        );
    }
}
