//! Side-dispatched operations for item use, consumption, fishing bobbers, buckets, and tridents.
use super::{Game, Hit, consume::ItemConsumable};
use sunrise_client::items::ItemStack;
use sunrise_core::geometry::Vec3;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn side_emit_consume(
        &mut self,
        consumable: &ItemConsumable,
        stack: &ItemStack,
        count: i32,
    ) {
        if self.is_logical_server() {
            self.server_emit_consume(consumable, stack, count);
        } else {
            self.client_emit_consume(consumable, stack, count);
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_move_bobber_response(
        &mut self,
        id: u32,
        position: Vec3,
        ground: &mut sunrise_world::ground_support::GroundState,
        wanted: Vec3,
        moved: Vec3,
        velocity: &mut Vec3,
    ) {
        if self.is_logical_server() {
            self.server_move_bobber_response(id, position, ground, wanted, moved, velocity);
        } else {
            self.client_move_bobber_response(id, position, ground, wanted, moved, velocity);
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_consume_held_slot(&mut self, amount: u32) {
        if self.is_logical_server() {
            self.server_consume_held_slot(amount);
        } else {
            self.client_consume_held_slot(amount);
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_on_start_using_item(&mut self, stack: &ItemStack) {
        if self.is_logical_server() {
            self.server_on_start_using_item(stack);
        } else {
            self.client_on_start_using_item(stack);
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_apply_use_cooldown(&mut self, stack: &ItemStack) {
        if self.is_logical_server() {
            self.server_apply_use_cooldown(stack);
        } else {
            self.client_apply_use_cooldown(stack);
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_item_pick(&self) -> Option<Hit> {
        if self.is_logical_server() {
            self.server_item_pick()
        } else {
            self.client_item_pick()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_bucket_liquid_user_entity_id(&self) -> Option<u32> {
        if self.is_logical_server() {
            self.server_bucket_liquid_user_entity_id()
        } else {
            self.client_bucket_liquid_user_entity_id()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_in_rain(&mut self, at: Vec3, top: f64) -> bool {
        if self.is_logical_server() {
            self.server_in_rain(at, top)
        } else {
            self.client_in_rain(at, top)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_projectile_player_uuid(&mut self) -> Option<u128> {
        if self.is_logical_server() {
            self.server_projectile_player_uuid()
        } else {
            self.client_projectile_player_uuid()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_acting_player_ref(&self) -> Option<mod_api::PlayerRef> {
        if self.is_logical_server() {
            self.server_acting_player_ref()
        } else {
            self.client_acting_player_ref()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_current_level_ref(&self) -> Option<mod_api::LevelRef> {
        if self.is_logical_server() {
            self.server_current_level_ref()
        } else {
            self.client_current_level_ref()
        }
    }
}
