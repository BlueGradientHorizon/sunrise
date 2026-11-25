//! Role dispatchers owned by lane `blocks` (`side_ops/blocks.rs`).
use super::Game;
use sunrise_client::{Result, containers::BlockEntity};
use sunrise_client_render::moving_colliders::Prepared;
use sunrise_core::{geometry::Vec3, nbt::Tag};
use sunrise_world::entity_move::MoverType;

impl Game<'_> {
    pub(in crate::game) fn side_encode_live_block_entity(
        &self,
        key: (i32, i32, i32),
        entity: &BlockEntity,
        block: &str,
    ) -> Tag {
        if self.is_logical_server() {
            self.server_encode_live_block_entity(key, entity, block)
        } else {
            self.client_encode_live_block_entity(key, entity, block)
        }
    }

    pub(in crate::game) fn side_station_changed(&mut self) {
        if self.is_logical_server() {
            self.server_station_changed()
        } else {
            self.client_station_changed()
        }
    }

    pub(in crate::game) fn side_combiner_take_effects(
        &mut self,
        kind: super::stations_common::Kind,
        pos: [i32; 3],
        level_change: i32,
    ) -> bool {
        if self.is_logical_server() {
            self.server_combiner_take_effects(kind, pos, level_change)
        } else {
            self.client_combiner_take_effects(kind, pos, level_change)
        }
    }

    pub(in crate::game) fn side_move_local_player(
        &mut self,
        mover: MoverType,
        movement: Vec3,
        auto_jump: bool,
    ) -> Result<Option<Vec3>> {
        if self.is_logical_server() {
            self.server_move_local_player(mover, movement, auto_jump)
        } else {
            self.client_move_local_player(mover, movement, auto_jump)
        }
    }

    pub(in crate::game) fn side_push_piston_entities(
        &mut self,
        pushes: Vec<super::piston_push::Push>,
    ) {
        if self.is_logical_server() {
            self.server_push_piston_entities(pushes)
        } else {
            self.client_push_piston_entities(pushes)
        }
    }

    pub(in crate::game) fn side_carry_pass(&mut self, start: &Prepared) {
        if self.is_logical_server() {
            self.server_carry_pass(start)
        } else {
            self.client_carry_pass(start)
        }
    }
}
