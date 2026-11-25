#[allow(unused_imports)]
pub(in crate::game::server_side) use super::event_loop::EventLoopCtl;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::owner_link::{ControlRequest, OwnerRequest};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Screen};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::Agent;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::Args;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::AttackAim;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::Break;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::FACES;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::Glide;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::Goto;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::HELP;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::Handled;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::MOVE_KEYS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::SneakClick;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::Task;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::TaskKind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::VALUE_FLAGS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::agent::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use navigation::{Cell, Node, Plan, Progress};
#[allow(unused_imports)]
pub(in crate::game::server_side) use serde_json::{Value, json};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::mpsc::Sender,
    time::{Duration, Instant},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::held::Source;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    Result,
    block_entity_anim::Ticker,
    items::ItemStack,
    mobs::{Species, render::VILLAGER_TYPES},
    widgets,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client_gameplay::agent_navigation as navigation;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::registry::BlockStateId;
#[allow(unused_imports)]
pub(in crate::game::server_side) use winit::{
    event::{DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent},
    keyboard::KeyCode,
};

impl Game<'_, crate::game::server_side::ServerState> {
    #[inline(always)]
    pub(in crate::game) fn shield_cooldown(&self) -> u32 {
        self.side.shield_cooldown
    }
}
