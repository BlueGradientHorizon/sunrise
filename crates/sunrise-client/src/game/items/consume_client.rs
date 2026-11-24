#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Dim, Game};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::api_values::TryIntoApi;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::collision_source::Mover;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::consume::Emitter;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::consume::ItemConsumable;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::consume::MobConsumption;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::consume::MobTeleporter;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::consume::Teleporter;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::consume::USE_ITEM_COMPLETE_EVENT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::consume::consumed_snapshot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::consume::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::convert::Infallible;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    items::{ConsumeEffect, ItemInfo, ItemStack},
    mobs::Who,
    particles::{ParticleOptions, ParticleType},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::Vec3, position::BlockPos, random::LegacyRandom, random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use {
    sunrise_core::use_animation::UseAnimation,
    sunrise_world::{
        consumable::{Consumable, EmissionWorld},
        teleport_consume::{TeleportConsume, TeleportConsumeWorld},
    },
};

impl TeleportConsumeWorld for MobTeleporter<'_, '_> {
    type Error = ();
    type Level = Dim;
    type Random = u32;
    type Source = LegacyRandom;
    fn position_field(&self) -> Vec3 {
        // get_random immediately refuses an absent actor before any draw or apply.
        self.at().unwrap_or_default()
    }
    fn get_random(&mut self) -> Result<u32, ()> {
        self.at()?;
        Ok(self.id)
    }
    fn random_source(&mut self, id: u32) -> &mut LegacyRandom {
        &mut self
            .game
            .mobs
            .iter_mut()
            .find(|m| m.id == id)
            .expect("get_random validated the actor without an intervening callback")
            .random
    }
    fn min_y(&mut self, _: Dim) -> Result<i32, ()> {
        Ok(self.game.world.shape.min_y)
    }
    fn logical_height(&mut self, _: Dim) -> Result<i32, ()> {
        Ok(self.game.world.shape.logical_height)
    }
    fn is_passenger(&mut self) -> Result<bool, ()> {
        self.game
            .mobs
            .iter()
            .find(|m| m.id == self.id)
            .map(|m| m.extra.passenger)
            .ok_or(())
    }
    fn stop_riding(&mut self) -> Result<(), ()> {
        self.game.dismount_mob(self.id);
        Ok(())
    }
    fn position(&mut self) -> Result<Vec3, ()> {
        self.at()
    }
    fn teleport_allowed(&mut self, target: &mut Vec3) -> Result<bool, ()> {
        let from = self.at()?;
        let mut to = [target.x, target.y, target.z];
        let mut cancel = false;
        if let Some(consumed) = self.consumed {
            self.game.native_item_consumption_teleport(
                self.consumer,
                consumed,
                [from.x, from.y, from.z],
                &mut to,
                &mut cancel,
            );
        }
        if self.game.dim != self.dim
            || !matches!(self.game.native_living_target(self.consumer),
                Some(super::commands::Target::Mob(id)) if id == self.id)
            || !self
                .game
                .mobs
                .iter()
                .any(|m| m.id == self.id && self.usage.still_valid(m, from))
        {
            return Err(());
        }
        if to.iter().all(|v| v.is_finite()) {
            let shape = self.game.world.shape;
            let high = shape.min_y + shape.logical_height - 1;
            *target = Vec3::new(
                to[0],
                to[1].clamp(f64::from(shape.min_y), f64::from(high)),
                to[2],
            );
        }
        Ok(!cancel)
    }
    fn random_teleport(&mut self, at: Vec3) -> Result<bool, ()> {
        Ok(self.game.living_random_teleport(
            self.consumer,
            [at.x, at.y, at.z],
            Some("minecraft:consumable_does_not_teleport_to"),
        ))
    }
    fn teleport_event(&mut self, _: Dim, old: Vec3) -> Result<(), ()> {
        let ctx = self.game.who_context(Some(Who::Mob(self.id)), None);
        self.game
            .post_game_event("minecraft:teleport", [old.x, old.y, old.z], ctx);
        Ok(())
    }
    fn is_fox(&self) -> bool {
        matches!(self.usage, MobConsumption::Fox(_))
    }
    fn teleport_sound(&mut self, _: Dim, at: Vec3, fox: bool) -> Result<(), ()> {
        self.game.audio.0.play(
            if fox {
                "minecraft:entity.fox.teleport"
            } else {
                "minecraft:item.chorus_fruit.teleport"
            },
            Some([at.x, at.y, at.z]),
            1.0,
            1.0,
        );
        Ok(())
    }
    fn block_position(&mut self) -> Result<BlockPos, ()> {
        Ok(BlockPos::containing(self.at()?))
    }
    fn directional_particles(&mut self, _: Dim, origin: BlockPos, offset: i32) -> Result<(), ()> {
        self.game
            .level_event_at(2017, [origin.x, origin.y, origin.z], offset);
        Ok(())
    }
    fn reset_fall_distance(&mut self) -> Result<(), ()> {
        let m = self
            .game
            .mobs
            .iter_mut()
            .find(|m| m.id == self.id)
            .ok_or(())?;
        m.fall_distance = 0.0;
        Ok(())
    }
    fn reset_impulse(&mut self) -> Result<(), ()> {
        // The existing mob model has no separate CurrentImpulseContext state.
        Ok(())
    }
}

impl Game<'_> {
    pub(in crate::game) fn emit_living_teleport(
        &mut self,
        from: [f64; 3],
        to: [f64; 3],
        width: f64,
        height: f64,
    ) {
        let mut spawned = Vec::with_capacity(128);
        sunrise_client::particles::living_teleport_particles(
            self.particles.0.random(),
            from,
            to,
            width,
            height,
            |p, v| spawned.push((p, v)),
        );
        let options = ParticleOptions::simple(ParticleType::Portal);
        for (p, v) in spawned {
            self.particles.0.add(&options, p, v);
        }
    }
    /// `LivingEntity.handleEntityEvent(46)` for the entity the packet names:
    /// this player, or a mob the client is drawing.
    pub(in crate::game) fn entity_teleport_particles(&mut self, id: i32) {
        if self.remote.as_ref().is_some_and(|r| r.entity_id == id) {
            self.portal_particles();
            return;
        }
        let spawned = {
            let Some(m) = self.mobs.iter_mut().find(|m| m.id == id as u32) else {
                return;
            };
            let from = [m.previous.x, m.previous.y, m.previous.z];
            let to = [m.position.x, m.position.y, m.position.z];
            let (width, height) = (f64::from(m.width()), f64::from(m.height()));
            let mut spawned = Vec::with_capacity(128);
            sunrise_client::particles::living_teleport_particles(
                &mut m.random,
                from,
                to,
                width,
                height,
                |p, v| spawned.push((p, v)),
            );
            spawned
        };
        let options = ParticleOptions::simple(ParticleType::Portal);
        for (p, v) in spawned {
            self.particles.0.add(&options, p, v);
        }
    }
    /// `LivingEntity.completeUsingItem` for a consumable: `Consumable.onConsume`,
    /// then `ItemStack.applyAfterUseComponentSideEffects`' use remainder.
    pub(in crate::game) fn finish_consuming(
        &mut self,
        info: &ItemInfo,
        consumable: &ItemConsumable,
        stack: &ItemStack,
    ) {
        let slot = self.held_slot();
        self.emit_consume(consumable, stack, 16);
        // ServerPlayer: the used statistic and the consume_item trigger.
        self.award_stat("used", &stack.item, 1);
        self.trigger_item("minecraft:consume_item", stack);
        // FoodProperties.onConsume, the player's one ConsumableListener.
        if let Some((nutrition, saturation)) = sunrise_client::item_sync::food(stack, info.food) {
            let at = self.feet();
            let pitch = self.world.random.triangle_float(1.0, 0.4);
            self.audio.0.play(&consumable.sound, at, 1.0, pitch);
            if let Some(p) = self.player.as_mut() {
                // FoodData.eat(nutrition, saturation)
                let food = &mut p.health.food;
                food.food = (food.food + nutrition as i32).clamp(0, 20);
                food.saturation = (food.saturation + saturation).clamp(0.0, food.food as f32);
            }
            // Mth.randomBetween(random, 0.9, 1.0)
            let pitch = self.world.random.next_float() * (1.0f32 - 0.9f32) + 0.9;
            self.audio
                .0
                .play("minecraft:entity.player.burp", self.feet(), 0.5, pitch);
        }
        // SuspiciousStewEffects.onConsume, a ConsumableListener: each entry is
        // added as a `MobEffectInstance(effect, duration)` (amplifier 0).
        if let Some(data) = stack.extra.as_deref() {
            for (id, duration) in &data.stew_effects {
                if let Some(def) = sunrise_client::brewing::effect(id) {
                    self.add_player_effect(sunrise_client::effects::EffectInstance::new(
                        def.id, *duration, 0,
                    ));
                }
            }
        }
        // Consumable.onConsume applies these entries on the logical server only;
        // the client's listener, particles and sounds above still run there.
        self.apply_consumable_effects(stack, &consumable.effects, None);
        let event = if consumable.animation == UseAnimation::Drink {
            "minecraft:drink"
        } else {
            "minecraft:eat"
        };
        self.post_entity_event(event, Who::Player);
        // ItemStack.consume, then UseRemainder.convertIntoRemainder: nothing
        // with infinite materials, the hand when emptied, else the inventory
        // (ServerPlayer.handleExtraItemsCreatedOnUse) or a drop. A listener may
        // replace the stack or change the selected slot; neither is this use.
        if self.held_slot() != slot || self.stack_in(slot) != Some(stack) {
            return;
        }
        self.consume_held();
        self.apply_use_remainder(stack);
    }
}
