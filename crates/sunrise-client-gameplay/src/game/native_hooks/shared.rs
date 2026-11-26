
#[path = "cut_merged_particle_count_tests.rs"]
pub mod particle_count_tests;
#[cfg(test)]
#[path = "shared/audio_math_tests.rs"]
pub(in crate::game) mod audio_math_tests;
#[cfg(test)]
#[path = "shared/runtime_environment_tests.rs"]
pub(in crate::game) mod runtime_environment_tests;
#[cfg(any(test, feature = "test-support"))]
#[path = "shared/screen_position_tests.rs"]
pub mod screen_position_tests;

#[cfg(feature = "client")]
#[cfg(test)]
use crate::game::client_side::native_hooks_client::posed_camera;