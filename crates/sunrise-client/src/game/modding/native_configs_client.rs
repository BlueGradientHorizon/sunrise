#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_configs::CHECK_EVERY;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_configs::NativeConfigs;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_configs::Tracked;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_configs::locate;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_configs::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::ConfigPath;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::game::ConfigFile;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::fs;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::path::{Path, PathBuf};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::time::{Duration, Instant};

impl Game<'_> {
    /// The window's frame step: changed config files reload once a second.
    pub(in crate::game) fn native_configs_tick_client(&mut self) {
        self.native_configs_poll();
    }
}
