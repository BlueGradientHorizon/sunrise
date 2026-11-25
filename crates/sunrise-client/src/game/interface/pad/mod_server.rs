#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::event_loop::EventLoopCtl;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Screen};
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pad::Connection;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pad::Context;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pad::DEATH_CONFIRM_WAIT;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pad::HELP;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pad::Pads;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pad::RADIAL_RUNS_KEPT;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pad::RadialRuns;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pad::UI_CLICK_VOLUME;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pad::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use serde_json::{Value, json};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::time::{Duration, Instant};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::held::Source;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    Result,
    controller_notice::Notice,
    controller_ui::Device,
    gamepad::{
        self, Button, Edges, GlyphOf, GlyphStyle, HealthWatch, MenuJob, PadState, Route, Rumble,
    },
    hud::HudBatch,
    keymapping::SNEAK,
    pad_glyph::{Glyph, Glyphs},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client_input::radial;
#[allow(unused_imports)]
pub(in crate::game::server_side) use winit::event::MouseButton;
#[allow(unused_imports)]
pub(in crate::game::server_side) use winit::keyboard::KeyCode;

#[cfg(feature = "controllers")]
#[path = "hardware_server.rs"]
pub(in crate::game) mod hardware;
