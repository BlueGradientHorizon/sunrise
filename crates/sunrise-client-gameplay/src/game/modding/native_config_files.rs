use crate::game::ClientGame;
use mod_api::ConfigFiles;

impl ClientGame<'_> {
    #[inline(always)]
    pub fn config_files() -> &'static dyn ConfigFiles {
        sunrise_gameplay_common::game::native_config_file_services::config_files()
    }
    #[inline(always)]
    pub fn data_files() -> &'static dyn ConfigFiles {
        sunrise_gameplay_common::game::native_config_file_services::data_files()
    }

    #[inline(always)]
    pub fn is_loaded(id: &str) -> bool {
        sunrise_gameplay_common::game::native_config_file_services::is_loaded(id)
    }
}
