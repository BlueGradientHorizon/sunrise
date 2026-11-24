#[allow(unused_imports)]
pub(in crate::game::server_side) use super::event_loop::EventLoopCtl;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::input_trace::{InputOrigin, TraceEvent, TraceKind};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::owner_link::{ControlRequest, OwnerRequest};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Screen};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::ENTITIES_TICKS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::Export;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::FORMAT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::HELP;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::KEPT_RECORDINGS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::KEYFRAME_TICKS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::MAX_INPUT_EVENTS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::MAX_RECORDING_SECS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::PendingReplay;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::REPLAY_COMMANDS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::REPLAY_TIMEOUT_SECS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::Recording;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::Replay;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::Repro;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::SessionMeta;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::add_bytes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::add_text_file;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::bundle_scrubber;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::bundle_status;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::bundles_dir;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::copy_tree;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::extract_zip;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::home;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::keyframe_state;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::newest_recording;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::prune_dir;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::read_events;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::readme;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::recording_meta;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::recording_started_at;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::recordings_dir;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::resolve_recording;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::saves_dir;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::slug;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::spawn_lines;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::stamp;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::unique_dest;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::write_bundle;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::repro::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use serde::Deserialize;
#[allow(unused_imports)]
pub(in crate::game::server_side) use serde_json::{Value, json};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    fs,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
    time::{Duration, Instant, SystemTime},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    Result, gamepad::Button, keybinds, text::Text, title::world_management,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_system::scrub::Scrubber;
#[allow(unused_imports)]
pub(in crate::game::server_side) use winit::{
    dpi::PhysicalPosition,
    event::{DeviceId, ElementState, MouseScrollDelta, TouchPhase, WindowEvent},
    window::WindowId,
};

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn start_recording_world_root_server(&self) -> Option<PathBuf> {
        if !self.server.world_root.as_os_str().is_empty() {
            Some(self.server.world_root.clone())
        } else {
            None
        }
    }
}
