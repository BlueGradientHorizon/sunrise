#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::beehives::GameHive;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::beehives::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{Game, item_abilities::ability};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    Result,
    beehive::{self, Beehive, BeehiveExt, HiveHost, Release},
    containers::BlockEntity,
    items::ItemStack,
    mobs::{Mob, Who, bee},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::{LegacyRandom, RandomSource},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use {
    sunrise_core::nbt::Tag, sunrise_world::registry::BlockStateId,
};

impl HiveHost for GameHive<'_, '_> {
    fn random(&mut self) -> &mut LegacyRandom {
        &mut self.game.world.random
    }
    fn bees_stay_in_hive(&self, _at: [i32; 3]) -> bool {
        if let Some(stay) = self.stay.get() {
            return stay;
        }
        let stay = bee::bees_stay_in_hive(&self.game.mob_world_base(&[]));
        self.stay.set(Some(stay));
        stay
    }
    fn blocked(&self, at: [i32; 3]) -> bool {
        !self
            .game
            .mob_world_base(&[])
            .block_collision_boxes(at[0], at[1], at[2])
            .is_empty()
    }
    fn set_honey_level(&mut self, at: [i32; 3], level: i32) {
        let state = self.game.world.block(at[0], at[1], at[2]);
        if let Some(state) = self.game.hive_with_honey(state, level) {
            let _ = self.game.level_set(at, state);
        }
    }
    fn load(&mut self, tag: &Tag) -> Option<Mob> {
        let kind = sunrise_client::entity_nbt::kind(tag)?;
        if !self.game.items().entity_type_tags.has_flag(
            &kind,
            sunrise_client::items::EntityTypeTags::BEEHIVE_INHABITORS,
        ) {
            return None;
        }
        let id = self.game.load_mob(tag.clone(), None)?;
        let i = self.game.mobs.iter().position(|m| m.id == id)?;
        Some(self.game.mobs.remove(i))
    }
    fn add(&mut self, m: Mob) -> bool {
        self.game.mobs.push(m);
        true
    }
    fn sound(&mut self, name: &'static str, at: [f64; 3]) {
        self.game
            .server_play_sound(&format!("minecraft:{name}"), at, 1.0, 1.0);
    }
    fn set_changed(&mut self, at: [i32; 3]) {
        self.game.world.modified.insert((at[0] >> 4, at[2] >> 4));
    }
}
