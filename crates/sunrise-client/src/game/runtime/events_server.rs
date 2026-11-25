#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::events::Condition;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::events::Events;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::events::HELP;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::events::Hit;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::events::KEPT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::events::Snapshot;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::events::Wait;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::events::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use serde_json::{Value, json};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    collections::{BTreeMap, VecDeque},
    sync::mpsc::Sender,
    time::{Duration, Instant},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::Result;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn combat_last_hit(
        &self,
    ) -> Option<(i64, String, Option<f32>, Option<String>)> {
        self.combat.0.last_hit()
    }
}
