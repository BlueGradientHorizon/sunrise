#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game,
    dispenser::{Slot, body_equipment_species, mob_set_slot, mob_slot_item},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispense_fixture::EXACT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispense_fixture::LOG;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispense_fixture::Target;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispense_fixture::event_sound;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispense_fixture::floor_pos;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispense_fixture::num;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispense_fixture::record;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispense_fixture::record_events;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispense_fixture::start_recording;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispense_fixture::stop_recording;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispense_fixture::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use serde_json::Value;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{
    cell::RefCell, collections::HashMap, fmt::Write as _, io::Write as _, sync::Arc,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    chunk::Chunk,
    command::args::GameMode,
    containers::BlockEntity,
    gamerules::Value as RuleValue,
    item_data::{canon_stack, parse_stack},
    items::ItemStack,
    mobs::{
        Mob, Species,
        spawner::{SpawnDataExt, SpawnerExt},
    },
    motion::PlayerMotion,
    projectiles::ProjectileKind,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{position::BlockPos, random::RandomSource};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::{level::Ticks, registry::BlockStateId};

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn dispense_case_client_finish(&mut self, out: &mut Vec<String>) {
        for sound in self.audio.0.capture.take().unwrap_or_default() {
            out.push(format!("S {} {}", floor_pos(sound.position), sound.event));
        }
    }
}
