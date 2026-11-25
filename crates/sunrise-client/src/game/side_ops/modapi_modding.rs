//! Role-dispatched side operations for the `modapi-modding` lane.
use crate::game::{Game, commands::Target, native_server::PlayerRead};
use core::ops::ControlFlow;
use mod_api::{
    BlockCapabilitySite, BlockEntityRef, BlockFace, BlockSide, CollisionFlags, Compound,
    DatapackRegistrySet, DimensionKey, EntityRef, EntityResidency, EquipmentSlot, ItemEntityRef,
    LevelReadError, LevelRef, LivingRef, MainHand, MenuRef, PlayerRef, SupportType,
    client::first_person::Hand,
};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use sunrise_client::{
    items::{ItemStack, Items},
    mobs::finalize::EquipmentRead,
    world::World,
};
use sunrise_world::{native_load_context::LoopContext, registry::BlockStateId};

impl Game<'_> {
    pub(in crate::game) fn side_bind_constructed_entity_attachments(&mut self, id: u32) {
        if self.is_logical_server() {
            self.server_bind_constructed_entity_attachments(id);
        } else {
            self.client_bind_constructed_entity_attachments(id);
        }
    }

    pub(in crate::game) fn side_collision_scan_world(
        &self,
        level: LevelRef,
    ) -> Option<(&World, bool)> {
        if self.is_logical_server() {
            self.server_collision_scan_world(level)
        } else {
            self.client_collision_scan_world(level)
        }
    }

    pub(in crate::game) fn side_crafting_remainder_hook(
        &self,
        stack: &ItemStack,
        remainder: &mut Option<ItemStack>,
    ) {
        if self.is_logical_server() {
            self.server_crafting_remainder_hook(stack, remainder);
        } else {
            self.client_crafting_remainder_hook(stack, remainder);
        }
    }

    pub(in crate::game) fn side_entity_bounding_box(
        &self,
        entity: EntityRef,
    ) -> Option<[[f64; 3]; 2]> {
        if self.is_logical_server() {
            self.server_entity_bounding_box(entity)
        } else {
            self.client_entity_bounding_box(entity)
        }
    }

    pub(in crate::game) fn side_entity_eye_height(&self, entity: EntityRef) -> Option<f64> {
        if self.is_logical_server() {
            self.server_entity_eye_height(entity)
        } else {
            self.client_entity_eye_height(entity)
        }
    }

    pub(in crate::game) fn side_entity_is_shift_key_down(&self, entity: EntityRef) -> Option<bool> {
        if self.is_logical_server() {
            self.server_entity_is_shift_key_down(entity)
        } else {
            self.client_entity_is_shift_key_down(entity)
        }
    }

    pub(in crate::game) fn side_entity_level(&self, entity: EntityRef) -> Option<LevelRef> {
        if self.is_logical_server() {
            self.server_entity_level(entity)
        } else {
            self.client_entity_level(entity)
        }
    }

    pub(in crate::game) fn side_entity_main_arm(&self, entity: EntityRef) -> Option<MainHand> {
        if self.is_logical_server() {
            self.server_entity_main_arm(entity)
        } else {
            self.client_entity_main_arm(entity)
        }
    }

    pub(in crate::game) fn side_entity_residency(&self, entity: EntityRef) -> EntityResidency {
        if self.is_logical_server() {
            self.server_entity_residency(entity)
        } else {
            self.client_entity_residency(entity)
        }
    }

    pub(in crate::game) fn side_entity_velocity(&self, entity: EntityRef) -> Option<[f64; 3]> {
        if self.is_logical_server() {
            self.server_entity_velocity(entity)
        } else {
            self.client_entity_velocity(entity)
        }
    }

    pub(in crate::game) fn side_item_combat_actor(
        &self,
        actor: LivingRef,
    ) -> Option<(bool, bool, f64, bool)> {
        if self.is_logical_server() {
            self.server_item_combat_actor(actor)
        } else {
            self.client_item_combat_actor(actor)
        }
    }

    pub(in crate::game) fn side_item_entity(&self, entity: EntityRef) -> Option<ItemEntityRef> {
        if self.is_logical_server() {
            self.server_item_entity(entity)
        } else {
            self.client_item_entity(entity)
        }
    }

    pub(in crate::game) fn side_level_block_face_sturdy(
        &self,
        level: LevelRef,
        pos: [i32; 3],
        face: BlockFace,
        support: SupportType,
    ) -> Option<bool> {
        if self.is_logical_server() {
            self.server_level_block_face_sturdy(level, pos, face, support)
        } else {
            self.client_level_block_face_sturdy(level, pos, face, support)
        }
    }

    pub(in crate::game) fn side_level_datapack_set(
        &self,
        level: LevelRef,
    ) -> Option<Arc<DatapackRegistrySet>> {
        if self.is_logical_server() {
            self.server_level_datapack_set(level)
        } else {
            self.client_level_datapack_set(level)
        }
    }

    pub(in crate::game) fn side_level_handle_current(&self, level: LevelRef) -> bool {
        if self.is_logical_server() {
            self.server_level_handle_current(level)
        } else {
            self.client_level_handle_current(level)
        }
    }

    pub(in crate::game) fn side_level_is_raining_at(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<bool> {
        if self.is_logical_server() {
            self.server_level_is_raining_at(level, pos)
        } else {
            self.client_level_is_raining_at(level, pos)
        }
    }

    pub(in crate::game) fn side_level_view_world(&self, level: LevelRef) -> Option<&World> {
        if self.is_logical_server() {
            self.server_level_view_world(level)
        } else {
            self.client_level_view_world(level)
        }
    }

    pub(in crate::game) fn side_level_visit_entities_in(
        &self,
        level: LevelRef,
        min: [f64; 3],
        max: [f64; 3],
        visit: &mut dyn FnMut(EntityRef) -> ControlFlow<()>,
    ) -> Result<ControlFlow<()>, LevelReadError> {
        if self.is_logical_server() {
            self.server_level_visit_entities_in(level, min, max, visit)
        } else {
            self.client_level_visit_entities_in(level, min, max, visit)
        }
    }

    pub(in crate::game) fn side_lid_openers(&self) -> Option<&HashMap<(i32, i32, i32), i32>> {
        if self.is_logical_server() {
            self.server_lid_openers()
        } else {
            self.client_lid_openers()
        }
    }

    pub(in crate::game) fn side_lifecycle_open_started(&mut self) {
        if self.is_logical_server() {
            self.server_lifecycle_open_started();
        } else {
            self.client_lifecycle_open_started();
        }
    }

    pub(in crate::game) fn side_lifecycle_world_closed(&mut self) {
        if self.is_logical_server() {
            self.server_lifecycle_world_closed();
        } else {
            self.client_lifecycle_world_closed();
        }
    }

    pub(in crate::game) fn side_lifecycle_world_ready(&mut self) {
        if self.is_logical_server() {
            self.server_lifecycle_world_ready();
        } else {
            self.client_lifecycle_world_ready();
        }
    }

    pub(in crate::game) fn side_living_absorption(&self, living: LivingRef) -> Option<f32> {
        if self.is_logical_server() {
            self.server_living_absorption(living)
        } else {
            self.client_living_absorption(living)
        }
    }

    pub(in crate::game) fn side_living_attribute_value(
        &self,
        living: LivingRef,
        id: &str,
    ) -> Option<f64> {
        if self.is_logical_server() {
            self.server_living_attribute_value(living, id)
        } else {
            self.client_living_attribute_value(living, id)
        }
    }

    pub(in crate::game) fn side_living_equipment_stack(
        &self,
        living: LivingRef,
        slot: EquipmentSlot,
    ) -> Option<EquipmentRead<'_>> {
        if self.is_logical_server() {
            self.server_living_equipment_stack(living, slot)
        } else {
            self.client_living_equipment_stack(living, slot)
        }
    }

    pub(in crate::game) fn side_living_read_target(&self, living: LivingRef) -> Option<Target> {
        if self.is_logical_server() {
            self.server_living_read_target(living)
        } else {
            self.client_living_read_target(living)
        }
    }

    pub(in crate::game) fn side_living_target_alive(&self, id: u32) -> bool {
        if self.is_logical_server() {
            self.server_living_target_alive(id)
        } else {
            self.client_living_target_alive(id)
        }
    }

    pub(in crate::game) fn side_living_use_ticks(&self, living: LivingRef) -> Option<Option<u32>> {
        if self.is_logical_server() {
            self.server_living_use_ticks(living)
        } else {
            self.client_living_use_ticks(living)
        }
    }

    pub(in crate::game) fn side_living_used_item_hand(&self, living: LivingRef) -> Option<Hand> {
        if self.is_logical_server() {
            self.server_living_used_item_hand(living)
        } else {
            self.client_living_used_item_hand(living)
        }
    }

    pub(in crate::game) fn side_menu_carried(&self, menu: MenuRef) -> Option<Option<ItemStack>> {
        if self.is_logical_server() {
            self.server_menu_carried(menu)
        } else {
            self.client_menu_carried(menu)
        }
    }

    pub(in crate::game) fn side_menu_kind(&self, menu: MenuRef) -> Option<Option<String>> {
        if self.is_logical_server() {
            self.server_menu_kind(menu)
        } else {
            self.client_menu_kind(menu)
        }
    }

    pub(in crate::game) fn side_menu_slot(
        &self,
        menu: MenuRef,
        slot: usize,
    ) -> Option<Option<ItemStack>> {
        if self.is_logical_server() {
            self.server_menu_slot(menu, slot)
        } else {
            self.client_menu_slot(menu, slot)
        }
    }

    pub(in crate::game) fn side_menu_slot_count(&self, menu: MenuRef) -> Option<usize> {
        if self.is_logical_server() {
            self.server_menu_slot_count(menu)
        } else {
            self.client_menu_slot_count(menu)
        }
    }

    pub(in crate::game) fn side_pick_section_move(&mut self, id: u32, to: [f64; 3]) {
        if self.is_logical_server() {
            self.server_pick_section_move(id, to);
        } else {
            self.client_pick_section_move(id, to);
        }
    }

    pub(in crate::game) fn side_pick_section_remove(&mut self, id: u32) {
        if self.is_logical_server() {
            self.server_pick_section_remove(id);
        } else {
            self.client_pick_section_remove(id);
        }
    }

    pub(in crate::game) fn side_pick_section_sync_moved_id(&mut self, id: u32) {
        if self.is_logical_server() {
            self.server_pick_section_sync_moved_id(id);
        } else {
            self.client_pick_section_sync_moved_id(id);
        }
    }

    pub(in crate::game) fn side_player(&self, uuid: u128) -> Option<PlayerRef> {
        if self.is_logical_server() {
            self.server_player(uuid)
        } else {
            self.client_player(uuid)
        }
    }

    pub(in crate::game) fn side_player_client_brand(&self, player: PlayerRef) -> Option<&str> {
        if self.is_logical_server() {
            self.server_player_client_brand(player)
        } else {
            self.client_player_client_brand(player)
        }
    }

    pub(in crate::game) fn side_player_collision_flags(
        &self,
        player: PlayerRef,
    ) -> Result<Option<CollisionFlags>, LevelReadError> {
        if self.is_logical_server() {
            self.server_player_collision_flags(player)
        } else {
            self.client_player_collision_flags(player)
        }
    }

    pub(in crate::game) fn side_player_container_menu(&self, player: PlayerRef) -> Option<MenuRef> {
        if self.is_logical_server() {
            self.server_player_container_menu(player)
        } else {
            self.client_player_container_menu(player)
        }
    }

    pub(in crate::game) fn side_player_destroy_progress(
        &mut self,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<f32> {
        if self.is_logical_server() {
            self.server_player_destroy_progress(player, level, pos, state)
        } else {
            self.client_player_destroy_progress(player, level, pos, state)
        }
    }

    pub(in crate::game) fn side_player_dimension_key(
        &self,
        player: PlayerRef,
    ) -> Option<DimensionKey> {
        if self.is_logical_server() {
            self.server_player_dimension_key(player)
        } else {
            self.client_player_dimension_key(player)
        }
    }

    pub(in crate::game) fn side_player_has_correct_tool_for_drops(
        &mut self,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<bool> {
        if self.is_logical_server() {
            self.server_player_has_correct_tool_for_drops(player, level, pos, state)
        } else {
            self.client_player_has_correct_tool_for_drops(player, level, pos, state)
        }
    }

    pub(in crate::game) fn side_player_read_both(
        &self,
        player: PlayerRef,
    ) -> Option<PlayerRead<'_>> {
        if self.is_logical_server() {
            self.server_player_read_both(player)
        } else {
            self.client_player_read_both(player)
        }
    }

    pub(in crate::game) fn side_retire_disabled_instance_groups(
        &self,
        effective: &BTreeSet<String>,
    ) {
        if self.is_logical_server() {
            self.server_retire_disabled_instance_groups(effective);
        } else {
            self.client_retire_disabled_instance_groups(effective);
        }
    }

    pub(in crate::game) fn side_set_audio_loop_context(&mut self, audio: Option<LoopContext>) {
        if self.is_logical_server() {
            self.server_set_audio_loop_context(audio);
        } else {
            self.client_set_audio_loop_context(audio);
        }
    }

    pub(in crate::game) fn side_signal_block_entity(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<BlockEntityRef> {
        if self.is_logical_server() {
            self.server_signal_block_entity(level, pos)
        } else {
            self.client_signal_block_entity(level, pos)
        }
    }

    pub(in crate::game) fn side_signal_block_entity_data(
        &self,
        entity: BlockEntityRef,
    ) -> Option<Compound> {
        if self.is_logical_server() {
            self.server_signal_block_entity_data(entity)
        } else {
            self.client_signal_block_entity_data(entity)
        }
    }

    pub(in crate::game) fn side_signal_capability_site(
        &self,
        at: BlockSide,
    ) -> Option<BlockCapabilitySite> {
        if self.is_logical_server() {
            self.server_signal_capability_site(at)
        } else {
            self.client_signal_capability_site(at)
        }
    }

    pub(in crate::game) fn side_signal_container_size(&self, at: BlockSide) -> Option<usize> {
        if self.is_logical_server() {
            self.server_signal_container_size(at)
        } else {
            self.client_signal_container_size(at)
        }
    }

    pub(in crate::game) fn side_signal_container_stack(
        &self,
        at: BlockSide,
        slot: usize,
    ) -> Option<mod_api::ItemStack> {
        if self.is_logical_server() {
            self.server_signal_container_stack(at, slot)
        } else {
            self.client_signal_container_stack(at, slot)
        }
    }

    pub(in crate::game) fn side_tick_uses_server_sections(&self) -> bool {
        if self.is_logical_server() {
            self.server_tick_uses_server_sections()
        } else {
            self.client_tick_uses_server_sections()
        }
    }

    pub(in crate::game) fn side_world_table_items(&self) -> Option<&Arc<Items>> {
        if self.is_logical_server() {
            self.server_world_table_items()
        } else {
            self.client_world_table_items()
        }
    }
}
