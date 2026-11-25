#[allow(unused_imports)]
pub(in crate::game::server_side) use super::event_loop::EventLoopCtl;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::owner_link::{ControlRequest, OwnerRequest};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Screen};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::control::FEATURES;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::control::Flight;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::control::HELP;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::control::PROTOCOL;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::control::gpu_pass_switches;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::control::gpu_passes_reply;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::control::hello;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::control::key_code;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::control::released_modifier;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::control::socket_key;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::control::vendor_name;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::control::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use serde_json::{Value, json};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    sync::mpsc::Sender,
    time::{Duration, Instant},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::gfx::pack_timing;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::held::Source;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{Result, items::ItemStack};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::server_side) use winit::{event::MouseButton, keyboard::KeyCode};

impl Game<'_> {
    /// `sunrise-ctl save` and `save-all`.
    pub(in crate::game) fn control_save(&mut self, all: bool) -> Result<()> {
        if all {
            let keys: Vec<_> = self.world.chunks.keys().copied().collect();
            self.world.modified.extend(keys);
        }
        self.save_game()
    }
}
