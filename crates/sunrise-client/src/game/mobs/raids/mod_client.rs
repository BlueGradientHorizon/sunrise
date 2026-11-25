#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Dim, Game, creatures::Finalized};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::raids::Lvl;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::raids::PatrolCx;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::raids::Raiding;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::raids::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::{HashMap, HashSet};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::villager as mobs_villager;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    effects::{EffectInstance, OmenTick},
    lan::playerdata::ints_uuid,
    mobs::{
        Mob, Species, raider,
        spawn_rules::{self, SpawnLevel},
        spawn_selection::{Heightmap, NaturalLevel},
    },
    patrol::{GameplayAttributes, Member, PatrolLevel, PatrolSpawner},
    raid::{self, Action, BarName, RaidLevel, RaidPlayer, RaiderInfo, RaiderType, Raids, Spawn},
    random_source,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::Vec3,
    random::{LegacyRandom, LevelRandom, RandomSource},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::custom_spawner::{
    section, sections_to_village,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::village_siege::{SiegeLevel, VillageSiege};

impl Game<'_> {
    /// A raider's ominous banner (drop chance 2.0: always dropped) and a
    /// patrol captain's ominous bottle (`entities/pillager`).
    /// `OminousBottleAmplifier.onConsume`: Bad Omen for 100 minutes at the
    /// bottle's amplifier. The consumable list owns the dispose sound.
    pub(in crate::game::client_side) fn drink_ominous_bottle(
        &mut self,
        stack: &sunrise_client::items::ItemStack,
    ) {
        let amplifier = stack.data().and_then(|d| d.ominous_amplifier).unwrap_or(0);
        let mut effect =
            sunrise_client::effects::EffectInstance::new("bad_omen", 120_000, i32::from(amplifier));
        effect.visible = false;
        let new = self.potions.effects.get("bad_omen").is_none();
        self.apply_player_effect(effect);
        // MobEffect.onEffectAdded: Bad Omen's sound when it is new, at the
        // player's feet.
        if new && let Some(p) = self.player.as_ref() {
            let at = [p.position.x, p.position.y, p.position.z];
            self.audio
                .0
                .play("minecraft:event.mob_effect.bad_omen", Some(at), 1.0, 1.0);
        }
    }
    /// The Raid Omen sound when the effect is first added
    /// (`MobEffect.onEffectAdded`: at the player's feet).
    pub(in crate::game::client_side) fn raid_omen_added(&mut self) {
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let at = [p.position.x, p.position.y, p.position.z];
        self.audio
            .0
            .play("minecraft:event.mob_effect.raid_omen", Some(at), 1.0, 1.0);
    }
}

#[cfg(test)]
#[path = "tests/bar_players_client.rs"]
pub(in crate::game) mod bar_players_tests;
