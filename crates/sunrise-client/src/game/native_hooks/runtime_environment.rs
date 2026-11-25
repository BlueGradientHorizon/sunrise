//! Metadata reads use the bootstrap snapshot, never the world's logical side.
use super::Game;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    pub(in crate::game) fn dist(&self) -> mod_api::Dist {
        sunrise_level::runtime_env::runtime_environment::get().dist()
    }
    pub(in crate::game) fn game_version(&self) -> &'static str {
        sunrise_level::runtime_env::runtime_environment::get().game_version()
    }
    pub(in crate::game) fn server_data_version(&self) -> i32 {
        sunrise_core::version::DATA_VERSION
    }
    pub(in crate::game) fn sunrise_version(&self) -> &'static str {
        sunrise_level::runtime_env::runtime_environment::get().sunrise_version()
    }
    pub(in crate::game) fn game_tests_enabled(&self) -> bool {
        sunrise_level::runtime_env::runtime_environment::get().game_tests_enabled()
    }
}

#[cfg(test)]
#[path = "runtime_environment/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "runtime_environment/server_data_version_tests.rs"]
mod server_data_version_tests;
