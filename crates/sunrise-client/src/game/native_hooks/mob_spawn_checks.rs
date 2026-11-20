use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// `MobSpawnEvent.SpawnPlacementCheck`: after `SpawnPlacements.checkSpawnRules`,
    /// `default_result` is the effective predicate result, including native registrations
    /// even for vanilla types. `spawner` is the block that asked, if any. `Default` keeps
    /// that result; `Succeed` and `Fail` override it without undoing any draws. Server-side.
    pub(in crate::game) fn native_mob_spawn_placement_check(
        &self,
        call: mod_api::__private::MobSpawnPlacementCheckCall<'_>,
    ) {
        self.fire_mob_spawn_placement_check(call);
    }
    /// `MobSpawnEvent.PositionCheck`: the freshly created mob of type `kind`
    /// may stand at `pos` in `level` for `reason` (`Mob.checkSpawnRules` and
    /// `checkSpawnObstruction`). `spawner` is the block that asked, if one
    /// did; `mob` is the mob itself, lent for the call. `result` starts as
    /// `Default`: vanilla's two checks decide; `Succeed` or `Fail` skips them.
    /// Server-side.
    pub(in crate::game) fn native_mob_spawn_position_check(
        &self,
        call: mod_api::__private::MobSpawnPositionCheckCall<'_>,
    ) {
        self.fire_mob_spawn_position_check(call);
    }
}
