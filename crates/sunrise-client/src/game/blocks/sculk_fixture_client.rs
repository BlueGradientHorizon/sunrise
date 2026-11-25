#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, dispense_fixture};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::sculk_fixture::CLIENT_SOUNDS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::sculk_fixture::MobSpec;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::sculk_fixture::Scenario;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::sculk_fixture::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Write as _,
    io::Write as _,
    sync::Arc,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    chunk::{Chunk, MIN_Y},
    command::args::GameMode,
    containers::BlockEntity,
    mobs::{Mob, Who},
    motion::PlayerMotion,
    sculk::{Info, SculkKind, WardenSpawnTracker},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Direction, Vec3},
    position::BlockPos,
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::{
    chunk::BlockState, level::Ticks, registry::BlockStateId,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use winit::keyboard::KeyCode;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn sculk_scenario_drain_capture_client(
        &mut self,
    ) -> Vec<sunrise_client::audio::CapturedSound> {
        self.audio.0.capture.replace(Vec::new()).unwrap_or_default()
    }
    #[inline(always)]
    pub(in crate::game) fn sculk_scenario_set_capture_client(&mut self, enabled: bool) {
        self.audio.0.capture = enabled.then(Vec::new);
    }
}
