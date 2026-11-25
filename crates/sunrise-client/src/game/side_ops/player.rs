use crate::game::{Game, Hit, commands_common::Target, remote::movement::MovePacket};
use mod_api::{EntityRef, LevelRef, PlayerAbilities, PlayerRef};
use sunrise_client::{command::args::GameMode, items::ItemStack};
use sunrise_core::geometry::{Aabb, Vec3};
use sunrise_world::stepped_interpolation::PosRot;

impl Game<'_> {
    // Hands, inventory & equipment (6).
    #[inline(always)]
    pub(in crate::game) fn side_wear_glider(&mut self, points: u32) {
        if self.is_logical_server() {
            self.server_wear_glider(points)
        } else {
            self.client_wear_glider(points)
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_on_inventory_item_added(&mut self, stack: &ItemStack) {
        if self.is_logical_server() {
            self.server_on_inventory_item_added(stack)
        } else {
            self.client_on_inventory_item_added(stack)
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_reset_attack_strength(&mut self) {
        if self.is_logical_server() {
            self.server_reset_attack_strength()
        } else {
            self.client_reset_attack_strength()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_main_hand_attack_speed(&self) -> f64 {
        if self.is_logical_server() {
            self.server_main_hand_attack_speed()
        } else {
            self.client_main_hand_attack_speed()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_known_player_movement(&self) -> Vec3 {
        if self.is_logical_server() {
            self.server_known_player_movement()
        } else {
            self.client_known_player_movement()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_apply_equipment(&mut self) {
        if self.is_logical_server() {
            self.server_apply_equipment()
        } else {
            self.client_apply_equipment()
        }
    }

    // Game mode & abilities (4).
    #[inline(always)]
    pub(in crate::game) fn side_may_build(&self) -> bool {
        if self.is_logical_server() {
            self.server_may_build()
        } else {
            self.client_may_build()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_instabuild(&self) -> bool {
        if self.is_logical_server() {
            self.server_instabuild()
        } else {
            self.client_instabuild()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_player_info_game_mode(&self) -> Option<GameMode> {
        if self.is_logical_server() {
            self.server_player_info_game_mode()
        } else {
            self.client_player_info_game_mode()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_player_abilities(
        &self,
        player: PlayerRef,
    ) -> Option<PlayerAbilities> {
        if self.is_logical_server() {
            self.server_player_abilities(player)
        } else {
            self.client_player_abilities(player)
        }
    }

    // Player identity & residency (6).
    #[inline(always)]
    pub(in crate::game) fn side_own_entity_id(&self) -> i32 {
        if self.is_logical_server() {
            self.server_own_entity_id()
        } else {
            self.client_own_entity_id()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_own_profile_uuid(&self) -> Option<u128> {
        if self.is_logical_server() {
            self.server_own_profile_uuid()
        } else {
            self.client_own_profile_uuid()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_own_profile_name(&self) -> String {
        if self.is_logical_server() {
            self.server_own_profile_name()
        } else {
            self.client_own_profile_name()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_local_player(&self) -> Option<PlayerRef> {
        if self.is_logical_server() {
            self.server_local_player()
        } else {
            self.client_local_player()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_remove_local_player_body(&mut self) {
        if self.is_logical_server() {
            self.server_remove_local_player_body()
        } else {
            self.client_remove_local_player_body()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_bind_local_player_residency(&mut self) {
        if self.is_logical_server() {
            self.server_bind_local_player_residency()
        } else {
            self.client_bind_local_player_residency()
        }
    }

    // Aim, raycast & targeting (16; includes 1 #[cfg(test)]).
    #[inline(always)]
    pub(in crate::game) fn side_aim(&self) -> Option<([f64; 3], [f64; 3])> {
        if self.is_logical_server() {
            self.server_aim()
        } else {
            self.client_aim()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_pick(&self) -> Option<Hit> {
        if self.is_logical_server() {
            self.server_pick()
        } else {
            self.client_pick()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_pick_unfiltered(&self) -> Option<Hit> {
        if self.is_logical_server() {
            self.server_pick_unfiltered()
        } else {
            self.client_pick_unfiltered()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_mob_hit(&self) -> Option<(usize, f64, [f64; 3])> {
        if self.is_logical_server() {
            self.server_mob_hit()
        } else {
            self.client_mob_hit()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_block_distance(&self) -> Option<f64> {
        if self.is_logical_server() {
            self.server_block_distance()
        } else {
            self.client_block_distance()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_frame_hit(&self) -> Option<(usize, f64)> {
        if self.is_logical_server() {
            self.server_frame_hit()
        } else {
            self.client_frame_hit()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_boat_hit(&self) -> Option<(usize, f64)> {
        if self.is_logical_server() {
            self.server_boat_hit()
        } else {
            self.client_boat_hit()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_cart_hit(&self) -> Option<(usize, f64)> {
        if self.is_logical_server() {
            self.server_cart_hit()
        } else {
            self.client_cart_hit()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_armor_stand_hit(&self) -> Option<(usize, f64, [f64; 3])> {
        if self.is_logical_server() {
            self.server_armor_stand_hit()
        } else {
            self.client_armor_stand_hit()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_painting_hit(&self) -> Option<(usize, f64)> {
        if self.is_logical_server() {
            self.server_painting_hit()
        } else {
            self.client_painting_hit()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_dragon_hit(&self) -> Option<(u32, f64)> {
        if self.is_logical_server() {
            self.server_dragon_hit()
        } else {
            self.client_dragon_hit()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_uses_stored_pick(&self) -> bool {
        if self.is_logical_server() {
            self.server_uses_stored_pick()
        } else {
            self.client_uses_stored_pick()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_pick_shares_vehicle(&self, id: u32) -> bool {
        if self.is_logical_server() {
            self.server_pick_shares_vehicle(id)
        } else {
            self.client_pick_shares_vehicle(id)
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_refresh_client_pick(&mut self, partial: f32) {
        if self.is_logical_server() {
            self.server_refresh_client_pick(partial)
        } else {
            self.client_refresh_client_pick(partial)
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_release_tick_targets(&mut self) {
        if self.is_logical_server() {
            self.server_release_tick_targets()
        } else {
            self.client_release_tick_targets()
        }
    }
    #[cfg(test)]
    #[inline(always)]
    pub(in crate::game) fn side_set_test_forced_hit(&mut self, hit: Option<Hit>) {
        if self.is_logical_server() {
            self.server_set_test_forced_hit(hit)
        } else {
            self.client_set_test_forced_hit(hit)
        }
    }

    // Client item-use hooks from common remote handlers (2).
    #[inline(always)]
    pub(in crate::game) fn side_reset_client_item_use(&mut self) {
        if self.is_logical_server() {
            self.server_reset_client_item_use()
        } else {
            self.client_reset_client_item_use()
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_tick_client_item_use(&mut self) {
        if self.is_logical_server() {
            self.server_tick_client_item_use()
        } else {
            self.client_tick_client_item_use()
        }
    }

    // Dual-role entity & level lookups (6).
    #[inline(always)]
    pub(in crate::game) fn side_client_display_position(
        &self,
        entity: EntityRef,
    ) -> Option<[f64; 3]> {
        if self.is_logical_server() {
            self.server_client_display_position(entity)
        } else {
            self.client_client_display_position(entity)
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_entity_position(&self, entity: EntityRef) -> Option<[f64; 3]> {
        if self.is_logical_server() {
            self.server_entity_position(entity)
        } else {
            self.client_entity_position(entity)
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_client_copy_box(
        &self,
        id: u32,
        position: Vec3,
        vanilla: Aabb,
    ) -> Aabb {
        if self.is_logical_server() {
            self.server_client_copy_box(id, position, vanilla)
        } else {
            self.client_client_copy_box(id, position, vanilla)
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_native_entity_read_target(
        &self,
        entity: EntityRef,
    ) -> Option<Target> {
        if self.is_logical_server() {
            self.server_native_entity_read_target(entity)
        } else {
            self.client_native_entity_read_target(entity)
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_level(&self, dimension: &str) -> Option<LevelRef> {
        if self.is_logical_server() {
            self.server_level(dimension)
        } else {
            self.client_level(dimension)
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_native_current_level(&self) -> Option<LevelRef> {
        if self.is_logical_server() {
            self.server_native_current_level()
        } else {
            self.client_native_current_level()
        }
    }

    // Client-only entity motion helpers called by common remote/motion.rs (3).
    #[inline(always)]
    pub(in crate::game) fn side_move_remote_interaction(
        &mut self,
        id: i32,
        position: [f64; 3],
        has_path: bool,
        rotation: Option<(f32, f32)>,
        packet: MovePacket,
        snap_old: Option<PosRot>,
    ) -> bool {
        if self.is_logical_server() {
            self.server_move_remote_interaction(id, position, has_path, rotation, packet, snap_old)
        } else {
            self.client_move_remote_interaction(id, position, has_path, rotation, packet, snap_old)
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_move_remote_hook(
        &mut self,
        id: u32,
        position: [f64; 3],
        on_ground: bool,
    ) -> bool {
        if self.is_logical_server() {
            self.server_move_remote_hook(id, position, on_ground)
        } else {
            self.client_move_remote_hook(id, position, on_ground)
        }
    }
    #[inline(always)]
    pub(in crate::game) fn side_has_remote_text_display(&self, id: i32) -> bool {
        if self.is_logical_server() {
            self.server_has_remote_text_display(id)
        } else {
            self.client_has_remote_text_display(id)
        }
    }
}
