#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::{
    Screen,
    native_entity_refs::tests::{run_fresh_cases, with_client_game},
    native_hooks::particle_count_tests::frame_scene,
};
#[allow(unused_imports)]
use mod_api::{ClientHost, Hooks, Identifier, ModEntry, ResolvePlayerSkinEvent};
#[allow(unused_imports)]
use mod_platform::testing::with_mods;
#[allow(unused_imports)]
use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
use sunrise_client::{
    camera::CameraType,
    crash::{ModCall, enter_native_mod},
    hud::Rect,
};

#[derive(Default)]
pub(in crate::game) struct Choice {
    pub(in crate::game) selection: Option<PlayerSkinOverride>,
    pub(in crate::game) calls: usize,
}

pub(in crate::game) struct SkinPicker(pub(in crate::game) Arc<Mutex<Choice>>);

#[mod_api::__subscriptions]
impl Hooks for SkinPicker {
    fn resolve_player_skin(&self, _: &dyn ClientHost, event: &mut ResolvePlayerSkinEvent<'_>) {
        assert!(
            event.selection.is_none(),
            "each frame starts with no selection"
        );
        let mut choice = self.0.lock().unwrap();
        choice.calls += 1;
        event.selection.clone_from(&choice.selection);
    }
}
