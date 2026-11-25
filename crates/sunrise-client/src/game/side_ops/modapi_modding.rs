//! Role-dispatched side operations for the `modapi-modding` lane.
use crate::game::{Game, commands::Target, native_server::PlayerRead};
use core::ops::ControlFlow;
use mod_api::{
    BlockCapabilitySite, BlockEntityRef, BlockFace, BlockSide, CollisionFlags, Compound,
    DatapackRegistrySet, DimensionKey, EntityRef, EntityResidency, EntityTypeKey, EquipmentSlot,
    ItemEntityRef, LevelReadError, LevelRef, LivingRef, MainHand, MenuRef, PlayerRef, SupportType,
    WorldEpoch, client::first_person::Hand,
};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use sunrise_client::{
    items::{ItemStack, Items},
    lan::PlayerSender,
    mobs::finalize::EquipmentRead,
    world::World,
};
use sunrise_world::{native_load_context::LoopContext, registry::BlockStateId};

impl<__Side: crate::game::Side> Game<'_, __Side> {
    pub(in crate::game) fn side_bind_constructed_entity_attachments(&mut self, id: u32) {
        <__Side as SideOps>::side_bind_constructed_entity_attachments(self, id)
    }

    pub(in crate::game) fn side_collision_scan_world(
        &self,
        level: LevelRef,
    ) -> Option<(&World, bool)> {
        <__Side as SideOps>::side_collision_scan_world(self, level)
    }

    pub(in crate::game) fn side_crafting_remainder_hook(
        &self,
        stack: &ItemStack,
        remainder: &mut Option<ItemStack>,
    ) {
        <__Side as SideOps>::side_crafting_remainder_hook(self, stack, remainder)
    }

    pub(in crate::game) fn side_entity_bounding_box(
        &self,
        entity: EntityRef,
    ) -> Option<[[f64; 3]; 2]> {
        <__Side as SideOps>::side_entity_bounding_box(self, entity)
    }

    pub(in crate::game) fn side_entity_eye_height(&self, entity: EntityRef) -> Option<f64> {
        <__Side as SideOps>::side_entity_eye_height(self, entity)
    }

    pub(in crate::game) fn side_entity_is_shift_key_down(&self, entity: EntityRef) -> Option<bool> {
        <__Side as SideOps>::side_entity_is_shift_key_down(self, entity)
    }

    pub(in crate::game) fn side_entity_kind(&self, entity: EntityRef) -> Option<EntityTypeKey> {
        <__Side as SideOps>::side_entity_kind(self, entity)
    }

    pub(in crate::game) fn side_entity_level(&self, entity: EntityRef) -> Option<LevelRef> {
        <__Side as SideOps>::side_entity_level(self, entity)
    }

    pub(in crate::game) fn side_entity_main_arm(&self, entity: EntityRef) -> Option<MainHand> {
        <__Side as SideOps>::side_entity_main_arm(self, entity)
    }

    pub(in crate::game) fn side_entity_residency(&self, entity: EntityRef) -> EntityResidency {
        <__Side as SideOps>::side_entity_residency(self, entity)
    }

    pub(in crate::game) fn side_entity_velocity(&self, entity: EntityRef) -> Option<[f64; 3]> {
        <__Side as SideOps>::side_entity_velocity(self, entity)
    }

    pub(in crate::game) fn side_item_combat_actor(
        &self,
        actor: LivingRef,
    ) -> Option<(bool, bool, f64, bool)> {
        <__Side as SideOps>::side_item_combat_actor(self, actor)
    }

    pub(in crate::game) fn side_item_entity(&self, entity: EntityRef) -> Option<ItemEntityRef> {
        <__Side as SideOps>::side_item_entity(self, entity)
    }

    pub(in crate::game) fn side_level_block_face_sturdy(
        &self,
        level: LevelRef,
        pos: [i32; 3],
        face: BlockFace,
        support: SupportType,
    ) -> Option<bool> {
        <__Side as SideOps>::side_level_block_face_sturdy(self, level, pos, face, support)
    }

    pub(in crate::game) fn side_level_datapack_set(
        &self,
        level: LevelRef,
    ) -> Option<Arc<DatapackRegistrySet>> {
        <__Side as SideOps>::side_level_datapack_set(self, level)
    }

    pub(in crate::game) fn side_level_handle_current(&self, level: LevelRef) -> bool {
        <__Side as SideOps>::side_level_handle_current(self, level)
    }

    pub(in crate::game) fn side_level_is_raining_at(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<bool> {
        <__Side as SideOps>::side_level_is_raining_at(self, level, pos)
    }

    pub(in crate::game) fn side_level_view_world(&self, level: LevelRef) -> Option<&World> {
        <__Side as SideOps>::side_level_view_world(self, level)
    }

    pub(in crate::game) fn side_level_visit_entities_in(
        &self,
        level: LevelRef,
        min: [f64; 3],
        max: [f64; 3],
        visit: &mut dyn FnMut(EntityRef) -> ControlFlow<()>,
    ) -> Result<ControlFlow<()>, LevelReadError> {
        <__Side as SideOps>::side_level_visit_entities_in(self, level, min, max, visit)
    }

    pub(in crate::game) fn side_lid_openers(&self) -> Option<&HashMap<(i32, i32, i32), i32>> {
        <__Side as SideOps>::side_lid_openers(self)
    }

    pub(in crate::game) fn side_lifecycle_open_started(&mut self) {
        <__Side as SideOps>::side_lifecycle_open_started(self)
    }

    pub(in crate::game) fn side_lifecycle_world_closed(&mut self) {
        <__Side as SideOps>::side_lifecycle_world_closed(self)
    }

    pub(in crate::game) fn side_lifecycle_world_ready(&mut self) {
        <__Side as SideOps>::side_lifecycle_world_ready(self)
    }

    pub(in crate::game) fn side_list_native_mods(&mut self) {
        <__Side as SideOps>::side_list_native_mods(self)
    }

    pub(in crate::game) fn side_living_absorption(&self, living: LivingRef) -> Option<f32> {
        <__Side as SideOps>::side_living_absorption(self, living)
    }

    pub(in crate::game) fn side_living_attribute_value(
        &self,
        living: LivingRef,
        id: &str,
    ) -> Option<f64> {
        <__Side as SideOps>::side_living_attribute_value(self, living, id)
    }

    pub(in crate::game) fn side_living_equipment_stack(
        &self,
        living: LivingRef,
        slot: EquipmentSlot,
    ) -> Option<EquipmentRead<'_>> {
        <__Side as SideOps>::side_living_equipment_stack(self, living, slot)
    }

    pub(in crate::game) fn side_living_read_target(&self, living: LivingRef) -> Option<Target> {
        <__Side as SideOps>::side_living_read_target(self, living)
    }

    pub(in crate::game) fn side_living_target_alive(&self, id: u32) -> bool {
        <__Side as SideOps>::side_living_target_alive(self, id)
    }

    pub(in crate::game) fn side_living_use_ticks(&self, living: LivingRef) -> Option<Option<u32>> {
        <__Side as SideOps>::side_living_use_ticks(self, living)
    }

    pub(in crate::game) fn side_living_used_item_hand(&self, living: LivingRef) -> Option<Hand> {
        <__Side as SideOps>::side_living_used_item_hand(self, living)
    }

    pub(in crate::game) fn side_menu_carried(&self, menu: MenuRef) -> Option<Option<ItemStack>> {
        <__Side as SideOps>::side_menu_carried(self, menu)
    }

    pub(in crate::game) fn side_menu_kind(&self, menu: MenuRef) -> Option<Option<String>> {
        <__Side as SideOps>::side_menu_kind(self, menu)
    }

    pub(in crate::game) fn side_menu_slot(
        &self,
        menu: MenuRef,
        slot: usize,
    ) -> Option<Option<ItemStack>> {
        <__Side as SideOps>::side_menu_slot(self, menu, slot)
    }

    pub(in crate::game) fn side_menu_slot_count(&self, menu: MenuRef) -> Option<usize> {
        <__Side as SideOps>::side_menu_slot_count(self, menu)
    }

    pub(in crate::game) fn side_pick_section_move(&mut self, id: u32, to: [f64; 3]) {
        <__Side as SideOps>::side_pick_section_move(self, id, to)
    }

    pub(in crate::game) fn side_pick_section_remove(&mut self, id: u32) {
        <__Side as SideOps>::side_pick_section_remove(self, id)
    }

    pub(in crate::game) fn side_pick_section_sync_moved_id(&mut self, id: u32) {
        <__Side as SideOps>::side_pick_section_sync_moved_id(self, id)
    }

    pub(in crate::game) fn side_player(&self, uuid: u128) -> Option<PlayerRef> {
        <__Side as SideOps>::side_player(self, uuid)
    }

    pub(in crate::game) fn side_player_client_brand(&self, player: PlayerRef) -> Option<&str> {
        <__Side as SideOps>::side_player_client_brand(self, player)
    }

    pub(in crate::game) fn side_player_collision_flags(
        &self,
        player: PlayerRef,
    ) -> Result<Option<CollisionFlags>, LevelReadError> {
        <__Side as SideOps>::side_player_collision_flags(self, player)
    }

    pub(in crate::game) fn side_player_container_menu(&self, player: PlayerRef) -> Option<MenuRef> {
        <__Side as SideOps>::side_player_container_menu(self, player)
    }

    pub(in crate::game) fn side_player_destroy_progress(
        &mut self,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<f32> {
        <__Side as SideOps>::side_player_destroy_progress(self, player, level, pos, state)
    }

    pub(in crate::game) fn side_player_dimension_key(
        &self,
        player: PlayerRef,
    ) -> Option<DimensionKey> {
        <__Side as SideOps>::side_player_dimension_key(self, player)
    }

    pub(in crate::game) fn side_player_has_correct_tool_for_drops(
        &mut self,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<bool> {
        <__Side as SideOps>::side_player_has_correct_tool_for_drops(self, player, level, pos, state)
    }

    pub(in crate::game) fn side_player_read_both(
        &self,
        player: PlayerRef,
    ) -> Option<PlayerRead<'_>> {
        <__Side as SideOps>::side_player_read_both(self, player)
    }

    pub(in crate::game) fn side_retire_disabled_instance_groups(
        &self,
        effective: &BTreeSet<String>,
    ) {
        <__Side as SideOps>::side_retire_disabled_instance_groups(self, effective)
    }

    pub(in crate::game) fn side_set_audio_loop_context(&mut self, audio: Option<LoopContext>) {
        <__Side as SideOps>::side_set_audio_loop_context(self, audio)
    }

    pub(in crate::game) fn side_signal_block_entity(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<BlockEntityRef> {
        <__Side as SideOps>::side_signal_block_entity(self, level, pos)
    }

    pub(in crate::game) fn side_signal_block_entity_data(
        &self,
        entity: BlockEntityRef,
    ) -> Option<Compound> {
        <__Side as SideOps>::side_signal_block_entity_data(self, entity)
    }

    pub(in crate::game) fn side_signal_capability_site(
        &self,
        at: BlockSide,
    ) -> Option<BlockCapabilitySite> {
        <__Side as SideOps>::side_signal_capability_site(self, at)
    }

    pub(in crate::game) fn side_signal_container_size(&self, at: BlockSide) -> Option<usize> {
        <__Side as SideOps>::side_signal_container_size(self, at)
    }

    pub(in crate::game) fn side_signal_container_stack(
        &self,
        at: BlockSide,
        slot: usize,
    ) -> Option<mod_api::ItemStack> {
        <__Side as SideOps>::side_signal_container_stack(self, at, slot)
    }

    pub(in crate::game) fn side_tick_uses_server_sections(&self) -> bool {
        <__Side as SideOps>::side_tick_uses_server_sections(self)
    }

    pub(in crate::game) fn side_world_table_items(&self) -> Option<&Arc<Items>> {
        <__Side as SideOps>::side_world_table_items(self)
    }

    pub(in crate::game) fn side_level_ref(&self, epoch: WorldEpoch, key: DimensionKey) -> LevelRef {
        <__Side as SideOps>::side_level_ref(self, epoch, key)
    }

    pub(in crate::game) fn side_online_player_sender(
        &self,
        player: PlayerRef,
    ) -> Option<PlayerSender> {
        <__Side as SideOps>::side_online_player_sender(self, player)
    }
}

// Generated by Shape G codemod from side_* role dispatchers.
pub(in crate::game) trait SideOps: Sized {
    fn side_bind_constructed_entity_attachments(game: &mut crate::game::Game<'_, Self>, id: u32);
    fn side_collision_scan_world<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        level: LevelRef,
    ) -> Option<(&'__side_g World, bool)>;
    fn side_crafting_remainder_hook(
        game: &crate::game::Game<'_, Self>,
        stack: &ItemStack,
        remainder: &mut Option<ItemStack>,
    );
    fn side_entity_bounding_box(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<[[f64; 3]; 2]>;
    fn side_entity_eye_height(game: &crate::game::Game<'_, Self>, entity: EntityRef)
    -> Option<f64>;
    fn side_entity_is_shift_key_down(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<bool>;
    fn side_entity_kind(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<EntityTypeKey>;
    fn side_entity_level(game: &crate::game::Game<'_, Self>, entity: EntityRef)
    -> Option<LevelRef>;
    fn side_entity_main_arm(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<MainHand>;
    fn side_entity_residency(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> EntityResidency;
    fn side_entity_velocity(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<[f64; 3]>;
    fn side_item_combat_actor(
        game: &crate::game::Game<'_, Self>,
        actor: LivingRef,
    ) -> Option<(bool, bool, f64, bool)>;
    fn side_item_entity(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<ItemEntityRef>;
    fn side_level_block_face_sturdy(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
        pos: [i32; 3],
        face: BlockFace,
        support: SupportType,
    ) -> Option<bool>;
    fn side_level_datapack_set(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
    ) -> Option<Arc<DatapackRegistrySet>>;
    fn side_level_handle_current(game: &crate::game::Game<'_, Self>, level: LevelRef) -> bool;
    fn side_level_is_raining_at(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<bool>;
    fn side_level_view_world<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        level: LevelRef,
    ) -> Option<&'__side_g World>;
    fn side_level_visit_entities_in(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
        min: [f64; 3],
        max: [f64; 3],
        visit: &mut dyn FnMut(EntityRef) -> ControlFlow<()>,
    ) -> Result<ControlFlow<()>, LevelReadError>;
    fn side_lid_openers<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
    ) -> Option<&'__side_g HashMap<(i32, i32, i32), i32>>;
    fn side_lifecycle_open_started(game: &mut crate::game::Game<'_, Self>);
    fn side_lifecycle_world_closed(game: &mut crate::game::Game<'_, Self>);
    fn side_lifecycle_world_ready(game: &mut crate::game::Game<'_, Self>);
    fn side_list_native_mods(game: &mut crate::game::Game<'_, Self>);
    fn side_living_absorption(game: &crate::game::Game<'_, Self>, living: LivingRef)
    -> Option<f32>;
    fn side_living_attribute_value(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
        id: &str,
    ) -> Option<f64>;
    fn side_living_equipment_stack<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        living: LivingRef,
        slot: EquipmentSlot,
    ) -> Option<EquipmentRead<'__side_g>>;
    fn side_living_read_target(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
    ) -> Option<Target>;
    fn side_living_target_alive(game: &crate::game::Game<'_, Self>, id: u32) -> bool;
    fn side_living_use_ticks(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
    ) -> Option<Option<u32>>;
    fn side_living_used_item_hand(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
    ) -> Option<Hand>;
    fn side_menu_carried(
        game: &crate::game::Game<'_, Self>,
        menu: MenuRef,
    ) -> Option<Option<ItemStack>>;
    fn side_menu_kind(game: &crate::game::Game<'_, Self>, menu: MenuRef) -> Option<Option<String>>;
    fn side_menu_slot(
        game: &crate::game::Game<'_, Self>,
        menu: MenuRef,
        slot: usize,
    ) -> Option<Option<ItemStack>>;
    fn side_menu_slot_count(game: &crate::game::Game<'_, Self>, menu: MenuRef) -> Option<usize>;
    fn side_pick_section_move(game: &mut crate::game::Game<'_, Self>, id: u32, to: [f64; 3]);
    fn side_pick_section_remove(game: &mut crate::game::Game<'_, Self>, id: u32);
    fn side_pick_section_sync_moved_id(game: &mut crate::game::Game<'_, Self>, id: u32);
    fn side_player(game: &crate::game::Game<'_, Self>, uuid: u128) -> Option<PlayerRef>;
    fn side_player_client_brand<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<&'__side_g str>;
    fn side_player_collision_flags(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Result<Option<CollisionFlags>, LevelReadError>;
    fn side_player_container_menu(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<MenuRef>;
    fn side_player_destroy_progress(
        game: &mut crate::game::Game<'_, Self>,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<f32>;
    fn side_player_dimension_key(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<DimensionKey>;
    fn side_player_has_correct_tool_for_drops(
        game: &mut crate::game::Game<'_, Self>,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<bool>;
    fn side_player_read_both<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<PlayerRead<'__side_g>>;
    fn side_retire_disabled_instance_groups(
        game: &crate::game::Game<'_, Self>,
        effective: &BTreeSet<String>,
    );
    fn side_set_audio_loop_context(
        game: &mut crate::game::Game<'_, Self>,
        audio: Option<LoopContext>,
    );
    fn side_signal_block_entity(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<BlockEntityRef>;
    fn side_signal_block_entity_data(
        game: &crate::game::Game<'_, Self>,
        entity: BlockEntityRef,
    ) -> Option<Compound>;
    fn side_signal_capability_site(
        game: &crate::game::Game<'_, Self>,
        at: BlockSide,
    ) -> Option<BlockCapabilitySite>;
    fn side_signal_container_size(
        game: &crate::game::Game<'_, Self>,
        at: BlockSide,
    ) -> Option<usize>;
    fn side_signal_container_stack(
        game: &crate::game::Game<'_, Self>,
        at: BlockSide,
        slot: usize,
    ) -> Option<mod_api::ItemStack>;
    fn side_tick_uses_server_sections(game: &crate::game::Game<'_, Self>) -> bool;
    fn side_world_table_items<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
    ) -> Option<&'__side_g Arc<Items>>;
    fn side_level_ref(
        game: &crate::game::Game<'_, Self>,
        epoch: WorldEpoch,
        key: DimensionKey,
    ) -> LevelRef;
    fn side_online_player_sender(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<PlayerSender>;
}
#[allow(unused_variables)]
impl SideOps for crate::game::server_side::ServerState {
    fn side_bind_constructed_entity_attachments(game: &mut crate::game::Game<'_, Self>, id: u32) {
        {
            game.server_bind_constructed_entity_attachments(id);
        }
    }
    fn side_collision_scan_world<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        level: LevelRef,
    ) -> Option<(&'__side_g World, bool)> {
        { game.server_collision_scan_world(level) }
    }
    fn side_crafting_remainder_hook(
        game: &crate::game::Game<'_, Self>,
        stack: &ItemStack,
        remainder: &mut Option<ItemStack>,
    ) {
        {
            game.server_crafting_remainder_hook(stack, remainder);
        }
    }
    fn side_entity_bounding_box(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<[[f64; 3]; 2]> {
        { game.server_entity_bounding_box(entity) }
    }
    fn side_entity_eye_height(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<f64> {
        { game.server_entity_eye_height(entity) }
    }
    fn side_entity_is_shift_key_down(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<bool> {
        { game.server_entity_is_shift_key_down(entity) }
    }
    fn side_entity_kind(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<EntityTypeKey> {
        { game.server_entity_kind(entity) }
    }
    fn side_entity_level(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<LevelRef> {
        { game.server_entity_level(entity) }
    }
    fn side_entity_main_arm(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<MainHand> {
        { game.server_entity_main_arm(entity) }
    }
    fn side_entity_residency(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> EntityResidency {
        { game.server_entity_residency(entity) }
    }
    fn side_entity_velocity(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<[f64; 3]> {
        { game.server_entity_velocity(entity) }
    }
    fn side_item_combat_actor(
        game: &crate::game::Game<'_, Self>,
        actor: LivingRef,
    ) -> Option<(bool, bool, f64, bool)> {
        { game.server_item_combat_actor(actor) }
    }
    fn side_item_entity(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<ItemEntityRef> {
        { game.server_item_entity(entity) }
    }
    fn side_level_block_face_sturdy(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
        pos: [i32; 3],
        face: BlockFace,
        support: SupportType,
    ) -> Option<bool> {
        { game.server_level_block_face_sturdy(level, pos, face, support) }
    }
    fn side_level_datapack_set(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
    ) -> Option<Arc<DatapackRegistrySet>> {
        { game.server_level_datapack_set(level) }
    }
    fn side_level_handle_current(game: &crate::game::Game<'_, Self>, level: LevelRef) -> bool {
        { game.server_level_handle_current(level) }
    }
    fn side_level_is_raining_at(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<bool> {
        { game.server_level_is_raining_at(level, pos) }
    }
    fn side_level_view_world<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        level: LevelRef,
    ) -> Option<&'__side_g World> {
        { game.server_level_view_world(level) }
    }
    fn side_level_visit_entities_in(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
        min: [f64; 3],
        max: [f64; 3],
        visit: &mut dyn FnMut(EntityRef) -> ControlFlow<()>,
    ) -> Result<ControlFlow<()>, LevelReadError> {
        { game.server_level_visit_entities_in(level, min, max, visit) }
    }
    fn side_lid_openers<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
    ) -> Option<&'__side_g HashMap<(i32, i32, i32), i32>> {
        { game.server_lid_openers() }
    }
    fn side_lifecycle_open_started(game: &mut crate::game::Game<'_, Self>) {
        {
            game.server_lifecycle_open_started();
        }
    }
    fn side_lifecycle_world_closed(game: &mut crate::game::Game<'_, Self>) {
        {
            game.server_lifecycle_world_closed();
        }
    }
    fn side_lifecycle_world_ready(game: &mut crate::game::Game<'_, Self>) {
        {
            game.server_lifecycle_world_ready();
        }
    }
    fn side_list_native_mods(game: &mut crate::game::Game<'_, Self>) {
        {}
    }
    fn side_living_absorption(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
    ) -> Option<f32> {
        { game.server_living_absorption(living) }
    }
    fn side_living_attribute_value(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
        id: &str,
    ) -> Option<f64> {
        { game.server_living_attribute_value(living, id) }
    }
    fn side_living_equipment_stack<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        living: LivingRef,
        slot: EquipmentSlot,
    ) -> Option<EquipmentRead<'__side_g>> {
        { game.server_living_equipment_stack(living, slot) }
    }
    fn side_living_read_target(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
    ) -> Option<Target> {
        { game.server_living_read_target(living) }
    }
    fn side_living_target_alive(game: &crate::game::Game<'_, Self>, id: u32) -> bool {
        { game.server_living_target_alive(id) }
    }
    fn side_living_use_ticks(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
    ) -> Option<Option<u32>> {
        { game.server_living_use_ticks(living) }
    }
    fn side_living_used_item_hand(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
    ) -> Option<Hand> {
        { game.server_living_used_item_hand(living) }
    }
    fn side_menu_carried(
        game: &crate::game::Game<'_, Self>,
        menu: MenuRef,
    ) -> Option<Option<ItemStack>> {
        { game.server_menu_carried(menu) }
    }
    fn side_menu_kind(game: &crate::game::Game<'_, Self>, menu: MenuRef) -> Option<Option<String>> {
        { game.server_menu_kind(menu) }
    }
    fn side_menu_slot(
        game: &crate::game::Game<'_, Self>,
        menu: MenuRef,
        slot: usize,
    ) -> Option<Option<ItemStack>> {
        { game.server_menu_slot(menu, slot) }
    }
    fn side_menu_slot_count(game: &crate::game::Game<'_, Self>, menu: MenuRef) -> Option<usize> {
        { game.server_menu_slot_count(menu) }
    }
    fn side_pick_section_move(game: &mut crate::game::Game<'_, Self>, id: u32, to: [f64; 3]) {
        {
            game.server_pick_section_move(id, to);
        }
    }
    fn side_pick_section_remove(game: &mut crate::game::Game<'_, Self>, id: u32) {
        {
            game.server_pick_section_remove(id);
        }
    }
    fn side_pick_section_sync_moved_id(game: &mut crate::game::Game<'_, Self>, id: u32) {
        {
            game.server_pick_section_sync_moved_id(id);
        }
    }
    fn side_player(game: &crate::game::Game<'_, Self>, uuid: u128) -> Option<PlayerRef> {
        { game.server_player(uuid) }
    }
    fn side_player_client_brand<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<&'__side_g str> {
        { game.server_player_client_brand(player) }
    }
    fn side_player_collision_flags(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Result<Option<CollisionFlags>, LevelReadError> {
        { game.server_player_collision_flags(player) }
    }
    fn side_player_container_menu(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<MenuRef> {
        { game.server_player_container_menu(player) }
    }
    fn side_player_destroy_progress(
        game: &mut crate::game::Game<'_, Self>,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<f32> {
        { game.server_player_destroy_progress(player, level, pos, state) }
    }
    fn side_player_dimension_key(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<DimensionKey> {
        { game.server_player_dimension_key(player) }
    }
    fn side_player_has_correct_tool_for_drops(
        game: &mut crate::game::Game<'_, Self>,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<bool> {
        { game.server_player_has_correct_tool_for_drops(player, level, pos, state) }
    }
    fn side_player_read_both<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<PlayerRead<'__side_g>> {
        { game.server_player_read_both(player) }
    }
    fn side_retire_disabled_instance_groups(
        game: &crate::game::Game<'_, Self>,
        effective: &BTreeSet<String>,
    ) {
        {
            game.server_retire_disabled_instance_groups(effective);
        }
    }
    fn side_set_audio_loop_context(
        game: &mut crate::game::Game<'_, Self>,
        audio: Option<LoopContext>,
    ) {
        {
            game.server_set_audio_loop_context(audio);
        }
    }
    fn side_signal_block_entity(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<BlockEntityRef> {
        { game.server_signal_block_entity(level, pos) }
    }
    fn side_signal_block_entity_data(
        game: &crate::game::Game<'_, Self>,
        entity: BlockEntityRef,
    ) -> Option<Compound> {
        { game.server_signal_block_entity_data(entity) }
    }
    fn side_signal_capability_site(
        game: &crate::game::Game<'_, Self>,
        at: BlockSide,
    ) -> Option<BlockCapabilitySite> {
        { game.server_signal_capability_site(at) }
    }
    fn side_signal_container_size(
        game: &crate::game::Game<'_, Self>,
        at: BlockSide,
    ) -> Option<usize> {
        { game.server_signal_container_size(at) }
    }
    fn side_signal_container_stack(
        game: &crate::game::Game<'_, Self>,
        at: BlockSide,
        slot: usize,
    ) -> Option<mod_api::ItemStack> {
        { game.server_signal_container_stack(at, slot) }
    }
    fn side_tick_uses_server_sections(game: &crate::game::Game<'_, Self>) -> bool {
        { game.server_tick_uses_server_sections() }
    }
    fn side_world_table_items<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
    ) -> Option<&'__side_g Arc<Items>> {
        { game.server_world_table_items() }
    }
    fn side_level_ref(
        game: &crate::game::Game<'_, Self>,
        epoch: WorldEpoch,
        key: DimensionKey,
    ) -> LevelRef {
        { game.level_ref(epoch, key) }
    }
    fn side_online_player_sender(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<PlayerSender> {
        { game.online_player_record(player).map(|g| g.sender.clone()) }
    }
}
#[allow(unused_variables)]
impl SideOps for crate::game::client_side::ClientState {
    fn side_bind_constructed_entity_attachments(game: &mut crate::game::Game<'_, Self>, id: u32) {
        {
            game.client_bind_constructed_entity_attachments(id);
        }
    }
    fn side_collision_scan_world<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        level: LevelRef,
    ) -> Option<(&'__side_g World, bool)> {
        { game.client_collision_scan_world(level) }
    }
    fn side_crafting_remainder_hook(
        game: &crate::game::Game<'_, Self>,
        stack: &ItemStack,
        remainder: &mut Option<ItemStack>,
    ) {
        {
            game.client_crafting_remainder_hook(stack, remainder);
        }
    }
    fn side_entity_bounding_box(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<[[f64; 3]; 2]> {
        { game.client_entity_bounding_box(entity) }
    }
    fn side_entity_eye_height(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<f64> {
        { game.client_entity_eye_height(entity) }
    }
    fn side_entity_is_shift_key_down(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<bool> {
        { game.client_entity_is_shift_key_down(entity) }
    }
    fn side_entity_kind(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<EntityTypeKey> {
        { game.client_entity_kind(entity) }
    }
    fn side_entity_level(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<LevelRef> {
        { game.client_entity_level(entity) }
    }
    fn side_entity_main_arm(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<MainHand> {
        { game.client_entity_main_arm(entity) }
    }
    fn side_entity_residency(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> EntityResidency {
        { game.client_entity_residency(entity) }
    }
    fn side_entity_velocity(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<[f64; 3]> {
        { game.client_entity_velocity(entity) }
    }
    fn side_item_combat_actor(
        game: &crate::game::Game<'_, Self>,
        actor: LivingRef,
    ) -> Option<(bool, bool, f64, bool)> {
        { game.client_item_combat_actor(actor) }
    }
    fn side_item_entity(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<ItemEntityRef> {
        { game.client_item_entity(entity) }
    }
    fn side_level_block_face_sturdy(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
        pos: [i32; 3],
        face: BlockFace,
        support: SupportType,
    ) -> Option<bool> {
        { game.client_level_block_face_sturdy(level, pos, face, support) }
    }
    fn side_level_datapack_set(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
    ) -> Option<Arc<DatapackRegistrySet>> {
        { game.client_level_datapack_set(level) }
    }
    fn side_level_handle_current(game: &crate::game::Game<'_, Self>, level: LevelRef) -> bool {
        { game.client_level_handle_current(level) }
    }
    fn side_level_is_raining_at(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<bool> {
        { game.client_level_is_raining_at(level, pos) }
    }
    fn side_level_view_world<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        level: LevelRef,
    ) -> Option<&'__side_g World> {
        { game.client_level_view_world(level) }
    }
    fn side_level_visit_entities_in(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
        min: [f64; 3],
        max: [f64; 3],
        visit: &mut dyn FnMut(EntityRef) -> ControlFlow<()>,
    ) -> Result<ControlFlow<()>, LevelReadError> {
        { game.client_level_visit_entities_in(level, min, max, visit) }
    }
    fn side_lid_openers<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
    ) -> Option<&'__side_g HashMap<(i32, i32, i32), i32>> {
        { game.client_lid_openers() }
    }
    fn side_lifecycle_open_started(game: &mut crate::game::Game<'_, Self>) {
        {
            game.client_lifecycle_open_started();
        }
    }
    fn side_lifecycle_world_closed(game: &mut crate::game::Game<'_, Self>) {
        {
            game.client_lifecycle_world_closed();
        }
    }
    fn side_lifecycle_world_ready(game: &mut crate::game::Game<'_, Self>) {
        {
            game.client_lifecycle_world_ready();
        }
    }
    fn side_list_native_mods(game: &mut crate::game::Game<'_, Self>) {
        {
            game.list_native_mods();
        }
    }
    fn side_living_absorption(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
    ) -> Option<f32> {
        { game.client_living_absorption(living) }
    }
    fn side_living_attribute_value(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
        id: &str,
    ) -> Option<f64> {
        { game.client_living_attribute_value(living, id) }
    }
    fn side_living_equipment_stack<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        living: LivingRef,
        slot: EquipmentSlot,
    ) -> Option<EquipmentRead<'__side_g>> {
        { game.client_living_equipment_stack(living, slot) }
    }
    fn side_living_read_target(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
    ) -> Option<Target> {
        { game.client_living_read_target(living) }
    }
    fn side_living_target_alive(game: &crate::game::Game<'_, Self>, id: u32) -> bool {
        { game.client_living_target_alive(id) }
    }
    fn side_living_use_ticks(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
    ) -> Option<Option<u32>> {
        { game.client_living_use_ticks(living) }
    }
    fn side_living_used_item_hand(
        game: &crate::game::Game<'_, Self>,
        living: LivingRef,
    ) -> Option<Hand> {
        { game.client_living_used_item_hand(living) }
    }
    fn side_menu_carried(
        game: &crate::game::Game<'_, Self>,
        menu: MenuRef,
    ) -> Option<Option<ItemStack>> {
        { game.client_menu_carried(menu) }
    }
    fn side_menu_kind(game: &crate::game::Game<'_, Self>, menu: MenuRef) -> Option<Option<String>> {
        { game.client_menu_kind(menu) }
    }
    fn side_menu_slot(
        game: &crate::game::Game<'_, Self>,
        menu: MenuRef,
        slot: usize,
    ) -> Option<Option<ItemStack>> {
        { game.client_menu_slot(menu, slot) }
    }
    fn side_menu_slot_count(game: &crate::game::Game<'_, Self>, menu: MenuRef) -> Option<usize> {
        { game.client_menu_slot_count(menu) }
    }
    fn side_pick_section_move(game: &mut crate::game::Game<'_, Self>, id: u32, to: [f64; 3]) {
        {
            game.client_pick_section_move(id, to);
        }
    }
    fn side_pick_section_remove(game: &mut crate::game::Game<'_, Self>, id: u32) {
        {
            game.client_pick_section_remove(id);
        }
    }
    fn side_pick_section_sync_moved_id(game: &mut crate::game::Game<'_, Self>, id: u32) {
        {
            game.client_pick_section_sync_moved_id(id);
        }
    }
    fn side_player(game: &crate::game::Game<'_, Self>, uuid: u128) -> Option<PlayerRef> {
        { game.client_player(uuid) }
    }
    fn side_player_client_brand<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<&'__side_g str> {
        { game.client_player_client_brand(player) }
    }
    fn side_player_collision_flags(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Result<Option<CollisionFlags>, LevelReadError> {
        { game.client_player_collision_flags(player) }
    }
    fn side_player_container_menu(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<MenuRef> {
        { game.client_player_container_menu(player) }
    }
    fn side_player_destroy_progress(
        game: &mut crate::game::Game<'_, Self>,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<f32> {
        { game.client_player_destroy_progress(player, level, pos, state) }
    }
    fn side_player_dimension_key(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<DimensionKey> {
        { game.client_player_dimension_key(player) }
    }
    fn side_player_has_correct_tool_for_drops(
        game: &mut crate::game::Game<'_, Self>,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<bool> {
        { game.client_player_has_correct_tool_for_drops(player, level, pos, state) }
    }
    fn side_player_read_both<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<PlayerRead<'__side_g>> {
        { game.client_player_read_both(player) }
    }
    fn side_retire_disabled_instance_groups(
        game: &crate::game::Game<'_, Self>,
        effective: &BTreeSet<String>,
    ) {
        {
            game.client_retire_disabled_instance_groups(effective);
        }
    }
    fn side_set_audio_loop_context(
        game: &mut crate::game::Game<'_, Self>,
        audio: Option<LoopContext>,
    ) {
        {
            game.client_set_audio_loop_context(audio);
        }
    }
    fn side_signal_block_entity(
        game: &crate::game::Game<'_, Self>,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<BlockEntityRef> {
        { game.client_signal_block_entity(level, pos) }
    }
    fn side_signal_block_entity_data(
        game: &crate::game::Game<'_, Self>,
        entity: BlockEntityRef,
    ) -> Option<Compound> {
        { game.client_signal_block_entity_data(entity) }
    }
    fn side_signal_capability_site(
        game: &crate::game::Game<'_, Self>,
        at: BlockSide,
    ) -> Option<BlockCapabilitySite> {
        { game.client_signal_capability_site(at) }
    }
    fn side_signal_container_size(
        game: &crate::game::Game<'_, Self>,
        at: BlockSide,
    ) -> Option<usize> {
        { game.client_signal_container_size(at) }
    }
    fn side_signal_container_stack(
        game: &crate::game::Game<'_, Self>,
        at: BlockSide,
        slot: usize,
    ) -> Option<mod_api::ItemStack> {
        { game.client_signal_container_stack(at, slot) }
    }
    fn side_tick_uses_server_sections(game: &crate::game::Game<'_, Self>) -> bool {
        { game.client_tick_uses_server_sections() }
    }
    fn side_world_table_items<'__side_g>(
        game: &'__side_g crate::game::Game<'_, Self>,
    ) -> Option<&'__side_g Arc<Items>> {
        { game.client_world_table_items() }
    }
    fn side_level_ref(
        game: &crate::game::Game<'_, Self>,
        epoch: WorldEpoch,
        key: DimensionKey,
    ) -> LevelRef {
        { LevelRef::new(epoch, key) }
    }
    fn side_online_player_sender(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<PlayerSender> {
        { None }
    }
}
