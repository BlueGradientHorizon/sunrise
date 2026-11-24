#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Hit, InteractTarget, Keys, entity::Entity};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::CART_KEY_Y;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::CartIndex;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::CartKind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::CartModels;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::DEFAULT_EXPLOSION_POWER;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::DEFAULT_EXPLOSION_SPEED_FACTOR;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::HEIGHT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::Minecart;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::PreparedModelFace;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::Rail;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::SEAT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::Shape;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::TNT_PRIME_EVENT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::WIDTH;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::can_ride_cart;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::cart_at;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::cart_general_seat;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::cart_is_destroyed;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::cart_push_from;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::cart_render_bounds;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::cart_seat;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::container_keep;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::default_display_offset;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::display_offset;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::emit_model_faces;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::forget_unreadable_display;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::furnace_fuel_after;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::furnace_speed;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::hit::CartHit;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::java_degrees;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::offered_pickup;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::pickup_instead_of_push;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::push_along;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::scale_tint_alpha;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::shown_unless_air;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::takes_push_from_cart;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::tnt_ignites_by_collision;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{collections::HashMap, sync::Arc};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::entity_rules::EntityRules;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::join_seq::{JoinList, ListStamp};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::mobs::{Mob, seen_grid::SeenGrid};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    blocks::Fluid,
    containers::BlockEntity,
    items::{ItemStack, Items},
    mesher::Vertex,
    mobs::{Part, Species, Who, anim::Poses, riders},
    particles::{ParticleOptions, ParticleType},
    random_source,
    scoreboard::pushable_by,
    sound_engine::{BLOCKS, CartSound, PlayResult, SoundInstance, Sounds},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::{Aabb, Vec3, trig},
    position::BlockPos,
    random::{LegacyRandom, RandomSource},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::minecart_cells::CartCells;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::minecart_seat::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::registry::BlockStateId;

#[path = "minecarts/save_lending_server.rs"]
pub(in crate::game) mod save_lending;

impl Game<'_> {
    /// `MinecartItem.useOn`: a cart on the clicked rail.
    pub(in crate::game) fn place_minecart(&mut self, hit: &Hit, stack: &ItemStack) -> bool {
        let Some(kind) = CartKind::from_id(&stack.item) else {
            return false;
        };
        let Some(rail) = self.rail(None, hit.pos[0], hit.pos[1], hit.pos[2]) else {
            return false;
        };
        let offset = if rail.shape.is_slope() { 0.5 } else { 0.0 };
        let at = Vec3::new(
            f64::from(hit.pos[0]) + 0.5,
            f64::from(hit.pos[1]) + 0.0625 + offset,
            f64::from(hit.pos[2]) + 0.5,
        );
        self.server.next_entity += 1;
        let cart = Minecart::new(self.server.next_entity, kind, at);
        self.world
            .modified
            .insert((hit.pos[0] >> 4, hit.pos[2] >> 4));
        self.carts.push(cart);
        let id = self.server.next_entity;
        self.apply_item_stack_config(super::commands::Target::Cart(id), kind.id(), stack, true);
        if self.serving.is_some() || self.options.dedicated.is_some() {
            let cart = self.carts.last().unwrap();
            self.native_vehicle_create(
                cart.kind.id(),
                [cart.position.x, cart.position.y, cart.position.z],
            );
        }
        // MinecartItem.useOn: ENTITY_PLACE at the rail, by the player, with the block under it.
        let below = self.world.block(hit.pos[0], hit.pos[1] - 1, hit.pos[2]);
        self.post_block_game_event(
            "minecraft:entity_place",
            hit.pos,
            self.sculk_actor(),
            Some(below),
        );
        if !self.creative {
            self.consume_held();
        }
        // ItemStack.useOn counts the use.
        self.award_stat("used", &stack.item, 1);
        self.swing_arm();
        true
    }
}

#[cfg(test)]
#[path = "minecarts/lifetime_tests_server.rs"]
pub(in crate::game) mod lifetime_tests;

#[cfg(test)]
#[path = "minecarts/minecart_tnt_tests_server.rs"]
pub(in crate::game) mod minecart_tnt_tests;
