//! Role-dispatched presentation, registry, camera, memory, and test-instance side operations.
use crate::game::{
    Game,
    memory_common::{MemoryInputs, MemoryReport},
};
use sunrise_client::first_person::{Hand, SwingAnimation};
use sunrise_world::{collision_context::EntityContextKind, worldgen::structure::Rotation};

impl Game<'_> {
    /// `LivingEntity.swing` with `getModifiedSwingDuration` after the item's `on_entity_swing` Part.
    pub(in crate::game) fn swing_hand(&mut self, hand: Hand, animation: SwingAnimation) {
        self.try_swing_hand(hand, animation);
    }

    /// `swing(hand)` for the hand that acts, with the default animation.
    pub(in crate::game) fn swing_arm(&mut self) {
        self.swing_hand(self.hand, SwingAnimation::DEFAULT);
    }

    #[inline(always)]
    pub(in crate::game) fn try_swing_hand(
        &mut self,
        hand: Hand,
        animation: SwingAnimation,
    ) -> bool {
        self.side_swing_hand(hand, animation)
    }

    #[inline(always)]
    pub(in crate::game) fn side_swing_hand(
        &mut self,
        hand: Hand,
        animation: SwingAnimation,
    ) -> bool {
        if self.is_logical_server() {
            self.server_swing_hand(hand, animation)
        } else {
            self.client_swing_hand(hand, animation)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_sound_event_names(&self) -> Vec<String> {
        if self.is_logical_server() {
            self.server_sound_event_names()
        } else {
            self.client_sound_event_names()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_sync_pack_tag_ids(&mut self) {
        if self.is_logical_server() {
            self.server_sync_pack_tag_ids();
        } else {
            self.client_sync_pack_tag_ids();
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_sync_hub_clocks(&mut self) {
        if self.is_logical_server() {
            self.server_sync_hub_clocks();
        } else {
            self.client_sync_hub_clocks();
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_has_singleplayer_server(&self) -> bool {
        if self.is_logical_server() {
            self.server_has_singleplayer_server()
        } else {
            self.client_has_singleplayer_server()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_is_published(&self) -> bool {
        if self.is_logical_server() {
            self.server_is_lan_published()
        } else {
            self.client_is_published()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_clear_lod(&mut self) {
        if self.is_logical_server() {
            self.server_clear_lod();
        } else {
            self.client_clear_lod();
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_clear_render_caches(&mut self) {
        if self.is_logical_server() {
            self.server_clear_render_caches();
        } else {
            self.client_clear_render_caches();
        }
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub(in crate::game) fn side_expire_cracks(&mut self) {
        if self.is_logical_server() {
            self.server_expire_cracks();
        } else {
            self.client_expire_cracks();
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_update_spawner_display(&mut self, key: (i32, i32, i32)) -> bool {
        if self.is_logical_server() {
            self.server_update_spawner_display(key)
        } else {
            self.client_update_spawner_display(key)
        }
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub(in crate::game) fn side_memory_inputs(&self) -> MemoryInputs {
        if self.is_logical_server() {
            self.server_memory_inputs()
        } else {
            self.client_memory_inputs()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_memory_report(&self) -> MemoryReport {
        if self.is_logical_server() {
            self.server_memory_report()
        } else {
            self.client_memory_report()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_test_holder_padding(&self, pos: (i32, i32, i32)) -> i32 {
        if self.is_logical_server() {
            self.server_test_holder_padding(pos)
        } else {
            self.client_test_holder_padding(pos)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_test_rotation(&self, pos: (i32, i32, i32)) -> Rotation {
        if self.is_logical_server() {
            self.server_test_rotation(pos)
        } else {
            self.client_test_rotation(pos)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_pick_camera_id(&self) -> Option<u32> {
        if self.is_logical_server() {
            self.server_pick_camera_id()
        } else {
            self.client_pick_camera_id()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_live_remote_camera_id(&self) -> Option<u32> {
        if self.is_logical_server() {
            self.server_live_remote_camera_id()
        } else {
            self.client_live_remote_camera_id()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_is_pick_camera(&self, id: u32) -> bool {
        if self.is_logical_server() {
            self.server_is_pick_camera(id)
        } else {
            self.client_is_pick_camera(id)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_outline_camera_kind(&self) -> EntityContextKind {
        if self.is_logical_server() {
            self.server_outline_camera_kind()
        } else {
            self.client_outline_camera_kind()
        }
    }
}
