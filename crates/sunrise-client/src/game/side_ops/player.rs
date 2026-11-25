use crate::game::{Game, Hit, commands_common::Target, remote::movement::MovePacket};
use mod_api::{EntityRef, LevelRef, PlayerAbilities, PlayerRef};
use sunrise_client::{command::args::GameMode, items::ItemStack};
use sunrise_core::geometry::{Aabb, Vec3};
use sunrise_world::stepped_interpolation::PosRot;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    // Hands, inventory & equipment (6).
    #[inline(always)]
    pub(in crate::game) fn side_wear_glider(&mut self, points: u32) {
        <__Side as SideOps>::side_wear_glider(self, points)
    }
    #[inline(always)]
    pub(in crate::game) fn side_on_inventory_item_added(&mut self, stack: &ItemStack) {
        <__Side as SideOps>::side_on_inventory_item_added(self, stack)
    }
    #[inline(always)]
    pub(in crate::game) fn side_reset_attack_strength(&mut self) {
        <__Side as SideOps>::side_reset_attack_strength(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_main_hand_attack_speed(&self) -> f64 {
        <__Side as SideOps>::side_main_hand_attack_speed(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_known_player_movement(&self) -> Vec3 {
        <__Side as SideOps>::side_known_player_movement(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_apply_equipment(&mut self) {
        <__Side as SideOps>::side_apply_equipment(self)
    }

    // Game mode & abilities (4).
    #[inline(always)]
    pub(in crate::game) fn side_may_build(&self) -> bool {
        <__Side as SideOps>::side_may_build(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_instabuild(&self) -> bool {
        <__Side as SideOps>::side_instabuild(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_player_info_game_mode(&self) -> Option<GameMode> {
        <__Side as SideOps>::side_player_info_game_mode(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_player_abilities(
        &self,
        player: PlayerRef,
    ) -> Option<PlayerAbilities> {
        <__Side as SideOps>::side_player_abilities(self, player)
    }

    // Player identity & residency (6).
    #[inline(always)]
    pub(in crate::game) fn side_own_entity_id(&self) -> i32 {
        <__Side as SideOps>::side_own_entity_id(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_own_profile_uuid(&self) -> Option<u128> {
        <__Side as SideOps>::side_own_profile_uuid(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_own_profile_name(&self) -> String {
        <__Side as SideOps>::side_own_profile_name(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_local_player(&self) -> Option<PlayerRef> {
        <__Side as SideOps>::side_local_player(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_remove_local_player_body(&mut self) {
        <__Side as SideOps>::side_remove_local_player_body(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_bind_local_player_residency(&mut self) {
        <__Side as SideOps>::side_bind_local_player_residency(self)
    }

    // Aim, raycast & targeting (16; includes 1 #[cfg(test)]).
    #[inline(always)]
    pub(in crate::game) fn side_aim(&self) -> Option<([f64; 3], [f64; 3])> {
        <__Side as SideOps>::side_aim(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_pick(&self) -> Option<Hit> {
        <__Side as SideOps>::side_pick(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_pick_unfiltered(&self) -> Option<Hit> {
        <__Side as SideOps>::side_pick_unfiltered(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_mob_hit(&self) -> Option<(usize, f64, [f64; 3])> {
        <__Side as SideOps>::side_mob_hit(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_block_distance(&self) -> Option<f64> {
        <__Side as SideOps>::side_block_distance(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_frame_hit(&self) -> Option<(usize, f64)> {
        <__Side as SideOps>::side_frame_hit(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_boat_hit(&self) -> Option<(usize, f64)> {
        <__Side as SideOps>::side_boat_hit(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_cart_hit(&self) -> Option<(usize, f64)> {
        <__Side as SideOps>::side_cart_hit(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_armor_stand_hit(&self) -> Option<(usize, f64, [f64; 3])> {
        <__Side as SideOps>::side_armor_stand_hit(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_painting_hit(&self) -> Option<(usize, f64)> {
        <__Side as SideOps>::side_painting_hit(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_dragon_hit(&self) -> Option<(u32, f64)> {
        <__Side as SideOps>::side_dragon_hit(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_uses_stored_pick(&self) -> bool {
        <__Side as SideOps>::side_uses_stored_pick(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_pick_shares_vehicle(&self, id: u32) -> bool {
        <__Side as SideOps>::side_pick_shares_vehicle(self, id)
    }
    #[inline(always)]
    pub(in crate::game) fn side_refresh_client_pick(&mut self, partial: f32) {
        <__Side as SideOps>::side_refresh_client_pick(self, partial)
    }
    #[allow(dead_code)]
    #[inline(always)]
    pub(in crate::game) fn side_release_tick_targets(&mut self) {
        <__Side as SideOps>::side_release_tick_targets(self)
    }
    #[cfg(test)]
    #[inline(always)]
    pub(in crate::game) fn side_set_test_forced_hit(&mut self, hit: Option<Hit>) {
        <__Side as SideOps>::side_set_test_forced_hit(self, hit)
    }

    // Client item-use hooks from common remote handlers (2).
    #[inline(always)]
    pub(in crate::game) fn side_reset_client_item_use(&mut self) {
        <__Side as SideOps>::side_reset_client_item_use(self)
    }
    #[inline(always)]
    pub(in crate::game) fn side_tick_client_item_use(&mut self) {
        <__Side as SideOps>::side_tick_client_item_use(self)
    }

    // Dual-role entity & level lookups (6).
    #[inline(always)]
    pub(in crate::game) fn side_client_display_position(
        &self,
        entity: EntityRef,
    ) -> Option<[f64; 3]> {
        <__Side as SideOps>::side_client_display_position(self, entity)
    }
    #[inline(always)]
    pub(in crate::game) fn side_entity_position(&self, entity: EntityRef) -> Option<[f64; 3]> {
        <__Side as SideOps>::side_entity_position(self, entity)
    }
    #[inline(always)]
    pub(in crate::game) fn side_client_copy_box(
        &self,
        id: u32,
        position: Vec3,
        vanilla: Aabb,
    ) -> Aabb {
        <__Side as SideOps>::side_client_copy_box(self, id, position, vanilla)
    }
    #[inline(always)]
    pub(in crate::game) fn side_native_entity_read_target(
        &self,
        entity: EntityRef,
    ) -> Option<Target> {
        <__Side as SideOps>::side_native_entity_read_target(self, entity)
    }
    #[inline(always)]
    pub(in crate::game) fn side_level(&self, dimension: &str) -> Option<LevelRef> {
        <__Side as SideOps>::side_level(self, dimension)
    }
    #[inline(always)]
    pub(in crate::game) fn side_native_current_level(&self) -> Option<LevelRef> {
        <__Side as SideOps>::side_native_current_level(self)
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
        <__Side as SideOps>::side_move_remote_interaction(
            self, id, position, has_path, rotation, packet, snap_old,
        )
    }
    #[inline(always)]
    pub(in crate::game) fn side_move_remote_hook(
        &mut self,
        id: u32,
        position: [f64; 3],
        on_ground: bool,
    ) -> bool {
        <__Side as SideOps>::side_move_remote_hook(self, id, position, on_ground)
    }
    #[inline(always)]
    pub(in crate::game) fn side_has_remote_text_display(&self, id: i32) -> bool {
        <__Side as SideOps>::side_has_remote_text_display(self, id)
    }
}

// Generated by Shape G codemod from side_* role dispatchers.
pub(in crate::game) trait SideOps: Sized {
    fn side_wear_glider(game: &mut crate::game::Game<'_, Self>, points: u32);
    fn side_on_inventory_item_added(game: &mut crate::game::Game<'_, Self>, stack: &ItemStack);
    fn side_reset_attack_strength(game: &mut crate::game::Game<'_, Self>);
    fn side_main_hand_attack_speed(game: &crate::game::Game<'_, Self>) -> f64;
    fn side_known_player_movement(game: &crate::game::Game<'_, Self>) -> Vec3;
    fn side_apply_equipment(game: &mut crate::game::Game<'_, Self>);
    fn side_may_build(game: &crate::game::Game<'_, Self>) -> bool;
    fn side_instabuild(game: &crate::game::Game<'_, Self>) -> bool;
    fn side_player_info_game_mode(game: &crate::game::Game<'_, Self>) -> Option<GameMode>;
    fn side_player_abilities(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<PlayerAbilities>;
    fn side_own_entity_id(game: &crate::game::Game<'_, Self>) -> i32;
    fn side_own_profile_uuid(game: &crate::game::Game<'_, Self>) -> Option<u128>;
    fn side_own_profile_name(game: &crate::game::Game<'_, Self>) -> String;
    fn side_local_player(game: &crate::game::Game<'_, Self>) -> Option<PlayerRef>;
    fn side_remove_local_player_body(game: &mut crate::game::Game<'_, Self>);
    fn side_bind_local_player_residency(game: &mut crate::game::Game<'_, Self>);
    fn side_aim(game: &crate::game::Game<'_, Self>) -> Option<([f64; 3], [f64; 3])>;
    fn side_pick(game: &crate::game::Game<'_, Self>) -> Option<Hit>;
    fn side_pick_unfiltered(game: &crate::game::Game<'_, Self>) -> Option<Hit>;
    fn side_mob_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64, [f64; 3])>;
    fn side_block_distance(game: &crate::game::Game<'_, Self>) -> Option<f64>;
    fn side_frame_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64)>;
    fn side_boat_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64)>;
    fn side_cart_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64)>;
    fn side_armor_stand_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64, [f64; 3])>;
    fn side_painting_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64)>;
    fn side_dragon_hit(game: &crate::game::Game<'_, Self>) -> Option<(u32, f64)>;
    fn side_uses_stored_pick(game: &crate::game::Game<'_, Self>) -> bool;
    fn side_pick_shares_vehicle(game: &crate::game::Game<'_, Self>, id: u32) -> bool;
    fn side_refresh_client_pick(game: &mut crate::game::Game<'_, Self>, partial: f32);
    fn side_release_tick_targets(game: &mut crate::game::Game<'_, Self>);
    #[cfg(test)]
    fn side_set_test_forced_hit(game: &mut crate::game::Game<'_, Self>, hit: Option<Hit>);
    fn side_reset_client_item_use(game: &mut crate::game::Game<'_, Self>);
    fn side_tick_client_item_use(game: &mut crate::game::Game<'_, Self>);
    fn side_client_display_position(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<[f64; 3]>;
    fn side_entity_position(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<[f64; 3]>;
    fn side_client_copy_box(
        game: &crate::game::Game<'_, Self>,
        id: u32,
        position: Vec3,
        vanilla: Aabb,
    ) -> Aabb;
    fn side_native_entity_read_target(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<Target>;
    fn side_level(game: &crate::game::Game<'_, Self>, dimension: &str) -> Option<LevelRef>;
    fn side_native_current_level(game: &crate::game::Game<'_, Self>) -> Option<LevelRef>;
    fn side_move_remote_interaction(
        game: &mut crate::game::Game<'_, Self>,
        id: i32,
        position: [f64; 3],
        has_path: bool,
        rotation: Option<(f32, f32)>,
        packet: MovePacket,
        snap_old: Option<PosRot>,
    ) -> bool;
    fn side_move_remote_hook(
        game: &mut crate::game::Game<'_, Self>,
        id: u32,
        position: [f64; 3],
        on_ground: bool,
    ) -> bool;
    fn side_has_remote_text_display(game: &crate::game::Game<'_, Self>, id: i32) -> bool;
}
#[allow(unused_variables)]
impl SideOps for crate::game::server_side::ServerState {
    fn side_wear_glider(game: &mut crate::game::Game<'_, Self>, points: u32) {
        game.server_wear_glider(points)
    }
    fn side_on_inventory_item_added(game: &mut crate::game::Game<'_, Self>, stack: &ItemStack) {
        game.server_on_inventory_item_added(stack)
    }
    fn side_reset_attack_strength(game: &mut crate::game::Game<'_, Self>) {
        game.server_reset_attack_strength()
    }
    fn side_main_hand_attack_speed(game: &crate::game::Game<'_, Self>) -> f64 {
        game.server_main_hand_attack_speed()
    }
    fn side_known_player_movement(game: &crate::game::Game<'_, Self>) -> Vec3 {
        game.server_known_player_movement()
    }
    fn side_apply_equipment(game: &mut crate::game::Game<'_, Self>) {
        game.server_apply_equipment()
    }
    fn side_may_build(game: &crate::game::Game<'_, Self>) -> bool {
        game.server_may_build()
    }
    fn side_instabuild(game: &crate::game::Game<'_, Self>) -> bool {
        game.server_instabuild()
    }
    fn side_player_info_game_mode(game: &crate::game::Game<'_, Self>) -> Option<GameMode> {
        game.server_player_info_game_mode()
    }
    fn side_player_abilities(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<PlayerAbilities> {
        game.server_player_abilities(player)
    }
    fn side_own_entity_id(game: &crate::game::Game<'_, Self>) -> i32 {
        game.server_own_entity_id()
    }
    fn side_own_profile_uuid(game: &crate::game::Game<'_, Self>) -> Option<u128> {
        game.server_own_profile_uuid()
    }
    fn side_own_profile_name(game: &crate::game::Game<'_, Self>) -> String {
        game.server_own_profile_name()
    }
    fn side_local_player(game: &crate::game::Game<'_, Self>) -> Option<PlayerRef> {
        game.server_local_player()
    }
    fn side_remove_local_player_body(game: &mut crate::game::Game<'_, Self>) {
        game.server_remove_local_player_body()
    }
    fn side_bind_local_player_residency(game: &mut crate::game::Game<'_, Self>) {
        game.server_bind_local_player_residency()
    }
    fn side_aim(game: &crate::game::Game<'_, Self>) -> Option<([f64; 3], [f64; 3])> {
        game.server_aim()
    }
    fn side_pick(game: &crate::game::Game<'_, Self>) -> Option<Hit> {
        game.server_pick()
    }
    fn side_pick_unfiltered(game: &crate::game::Game<'_, Self>) -> Option<Hit> {
        game.server_pick_unfiltered()
    }
    fn side_mob_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64, [f64; 3])> {
        game.server_mob_hit()
    }
    fn side_block_distance(game: &crate::game::Game<'_, Self>) -> Option<f64> {
        game.server_block_distance()
    }
    fn side_frame_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64)> {
        game.server_frame_hit()
    }
    fn side_boat_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64)> {
        game.server_boat_hit()
    }
    fn side_cart_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64)> {
        game.server_cart_hit()
    }
    fn side_armor_stand_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64, [f64; 3])> {
        game.server_armor_stand_hit()
    }
    fn side_painting_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64)> {
        game.server_painting_hit()
    }
    fn side_dragon_hit(game: &crate::game::Game<'_, Self>) -> Option<(u32, f64)> {
        game.server_dragon_hit()
    }
    fn side_uses_stored_pick(game: &crate::game::Game<'_, Self>) -> bool {
        game.server_uses_stored_pick()
    }
    fn side_pick_shares_vehicle(game: &crate::game::Game<'_, Self>, id: u32) -> bool {
        game.server_pick_shares_vehicle(id)
    }
    fn side_refresh_client_pick(game: &mut crate::game::Game<'_, Self>, partial: f32) {
        game.server_refresh_client_pick(partial)
    }
    fn side_release_tick_targets(game: &mut crate::game::Game<'_, Self>) {
        game.server_release_tick_targets()
    }
    #[cfg(test)]
    fn side_set_test_forced_hit(game: &mut crate::game::Game<'_, Self>, hit: Option<Hit>) {
        game.server_set_test_forced_hit(hit)
    }
    fn side_reset_client_item_use(game: &mut crate::game::Game<'_, Self>) {
        game.server_reset_client_item_use()
    }
    fn side_tick_client_item_use(game: &mut crate::game::Game<'_, Self>) {
        game.server_tick_client_item_use()
    }
    fn side_client_display_position(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<[f64; 3]> {
        game.server_client_display_position(entity)
    }
    fn side_entity_position(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<[f64; 3]> {
        game.server_entity_position(entity)
    }
    fn side_client_copy_box(
        game: &crate::game::Game<'_, Self>,
        id: u32,
        position: Vec3,
        vanilla: Aabb,
    ) -> Aabb {
        game.server_client_copy_box(id, position, vanilla)
    }
    fn side_native_entity_read_target(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<Target> {
        game.server_native_entity_read_target(entity)
    }
    fn side_level(game: &crate::game::Game<'_, Self>, dimension: &str) -> Option<LevelRef> {
        game.server_level(dimension)
    }
    fn side_native_current_level(game: &crate::game::Game<'_, Self>) -> Option<LevelRef> {
        game.server_native_current_level()
    }
    fn side_move_remote_interaction(
        game: &mut crate::game::Game<'_, Self>,
        id: i32,
        position: [f64; 3],
        has_path: bool,
        rotation: Option<(f32, f32)>,
        packet: MovePacket,
        snap_old: Option<PosRot>,
    ) -> bool {
        game.server_move_remote_interaction(id, position, has_path, rotation, packet, snap_old)
    }
    fn side_move_remote_hook(
        game: &mut crate::game::Game<'_, Self>,
        id: u32,
        position: [f64; 3],
        on_ground: bool,
    ) -> bool {
        game.server_move_remote_hook(id, position, on_ground)
    }
    fn side_has_remote_text_display(game: &crate::game::Game<'_, Self>, id: i32) -> bool {
        game.server_has_remote_text_display(id)
    }
}
#[cfg(feature = "client")]
#[allow(unused_variables)]
impl SideOps for crate::game::client_side::ClientState {
    fn side_wear_glider(game: &mut crate::game::Game<'_, Self>, points: u32) {
        game.client_wear_glider(points)
    }
    fn side_on_inventory_item_added(game: &mut crate::game::Game<'_, Self>, stack: &ItemStack) {
        game.client_on_inventory_item_added(stack)
    }
    fn side_reset_attack_strength(game: &mut crate::game::Game<'_, Self>) {
        game.client_reset_attack_strength()
    }
    fn side_main_hand_attack_speed(game: &crate::game::Game<'_, Self>) -> f64 {
        game.client_main_hand_attack_speed()
    }
    fn side_known_player_movement(game: &crate::game::Game<'_, Self>) -> Vec3 {
        game.client_known_player_movement()
    }
    fn side_apply_equipment(game: &mut crate::game::Game<'_, Self>) {
        game.client_apply_equipment()
    }
    fn side_may_build(game: &crate::game::Game<'_, Self>) -> bool {
        game.client_may_build()
    }
    fn side_instabuild(game: &crate::game::Game<'_, Self>) -> bool {
        game.client_instabuild()
    }
    fn side_player_info_game_mode(game: &crate::game::Game<'_, Self>) -> Option<GameMode> {
        game.client_player_info_game_mode()
    }
    fn side_player_abilities(
        game: &crate::game::Game<'_, Self>,
        player: PlayerRef,
    ) -> Option<PlayerAbilities> {
        game.client_player_abilities(player)
    }
    fn side_own_entity_id(game: &crate::game::Game<'_, Self>) -> i32 {
        game.client_own_entity_id()
    }
    fn side_own_profile_uuid(game: &crate::game::Game<'_, Self>) -> Option<u128> {
        game.client_own_profile_uuid()
    }
    fn side_own_profile_name(game: &crate::game::Game<'_, Self>) -> String {
        game.client_own_profile_name()
    }
    fn side_local_player(game: &crate::game::Game<'_, Self>) -> Option<PlayerRef> {
        game.client_local_player()
    }
    fn side_remove_local_player_body(game: &mut crate::game::Game<'_, Self>) {
        game.client_remove_local_player_body()
    }
    fn side_bind_local_player_residency(game: &mut crate::game::Game<'_, Self>) {
        game.client_bind_local_player_residency()
    }
    fn side_aim(game: &crate::game::Game<'_, Self>) -> Option<([f64; 3], [f64; 3])> {
        game.client_aim()
    }
    fn side_pick(game: &crate::game::Game<'_, Self>) -> Option<Hit> {
        game.client_pick()
    }
    fn side_pick_unfiltered(game: &crate::game::Game<'_, Self>) -> Option<Hit> {
        game.client_pick_unfiltered()
    }
    fn side_mob_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64, [f64; 3])> {
        game.client_mob_hit()
    }
    fn side_block_distance(game: &crate::game::Game<'_, Self>) -> Option<f64> {
        game.client_block_distance()
    }
    fn side_frame_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64)> {
        game.client_frame_hit()
    }
    fn side_boat_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64)> {
        game.client_boat_hit()
    }
    fn side_cart_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64)> {
        game.client_cart_hit()
    }
    fn side_armor_stand_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64, [f64; 3])> {
        game.client_armor_stand_hit()
    }
    fn side_painting_hit(game: &crate::game::Game<'_, Self>) -> Option<(usize, f64)> {
        game.client_painting_hit()
    }
    fn side_dragon_hit(game: &crate::game::Game<'_, Self>) -> Option<(u32, f64)> {
        game.client_dragon_hit()
    }
    fn side_uses_stored_pick(game: &crate::game::Game<'_, Self>) -> bool {
        game.client_uses_stored_pick()
    }
    fn side_pick_shares_vehicle(game: &crate::game::Game<'_, Self>, id: u32) -> bool {
        game.client_pick_shares_vehicle(id)
    }
    fn side_refresh_client_pick(game: &mut crate::game::Game<'_, Self>, partial: f32) {
        game.client_refresh_client_pick(partial)
    }
    fn side_release_tick_targets(game: &mut crate::game::Game<'_, Self>) {
        game.client_release_tick_targets()
    }
    #[cfg(test)]
    fn side_set_test_forced_hit(game: &mut crate::game::Game<'_, Self>, hit: Option<Hit>) {
        game.client_set_test_forced_hit(hit)
    }
    fn side_reset_client_item_use(game: &mut crate::game::Game<'_, Self>) {
        game.client_reset_client_item_use()
    }
    fn side_tick_client_item_use(game: &mut crate::game::Game<'_, Self>) {
        game.client_tick_client_item_use()
    }
    fn side_client_display_position(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<[f64; 3]> {
        game.client_client_display_position(entity)
    }
    fn side_entity_position(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<[f64; 3]> {
        game.client_entity_position(entity)
    }
    fn side_client_copy_box(
        game: &crate::game::Game<'_, Self>,
        id: u32,
        position: Vec3,
        vanilla: Aabb,
    ) -> Aabb {
        game.client_client_copy_box(id, position, vanilla)
    }
    fn side_native_entity_read_target(
        game: &crate::game::Game<'_, Self>,
        entity: EntityRef,
    ) -> Option<Target> {
        game.client_native_entity_read_target(entity)
    }
    fn side_level(game: &crate::game::Game<'_, Self>, dimension: &str) -> Option<LevelRef> {
        game.client_level(dimension)
    }
    fn side_native_current_level(game: &crate::game::Game<'_, Self>) -> Option<LevelRef> {
        game.client_native_current_level()
    }
    fn side_move_remote_interaction(
        game: &mut crate::game::Game<'_, Self>,
        id: i32,
        position: [f64; 3],
        has_path: bool,
        rotation: Option<(f32, f32)>,
        packet: MovePacket,
        snap_old: Option<PosRot>,
    ) -> bool {
        game.client_move_remote_interaction(id, position, has_path, rotation, packet, snap_old)
    }
    fn side_move_remote_hook(
        game: &mut crate::game::Game<'_, Self>,
        id: u32,
        position: [f64; 3],
        on_ground: bool,
    ) -> bool {
        game.client_move_remote_hook(id, position, on_ground)
    }
    fn side_has_remote_text_display(game: &crate::game::Game<'_, Self>, id: i32) -> bool {
        game.client_has_remote_text_display(id)
    }
}
