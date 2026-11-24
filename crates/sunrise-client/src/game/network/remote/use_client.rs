#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::r#use::*;

impl Game<'_> {
    pub(in crate::game) fn finish_remote_use_block(
        &mut self,
        hand: Hand,
        before: Option<u32>,
        swings: u32,
        done: bool,
    ) {
        if done
            && self.swings_started != swings
            && let Some(after) = self.with_hand(hand, |g| g.held().map(|s| s.count))
            && (Some(after) != before || self.creative)
        {
            self.client.heights.item_used(hand);
        }
    }
    /// A container click: the server answers with the authoritative slots.
    pub(in crate::game) fn remote_click(&mut self, right: bool) -> bool {
        let Some(size) = self
            .renderer
            .0
            .as_ref()
            .map(|r| [r.config.width, r.config.height])
        else {
            return false;
        };
        let batch = HudBatch::new(size, self.settings.gui_scale_inputs());
        let Some(internal) = self.slot_at(&batch, self.client.cursor.0, self.client.cursor.1)
        else {
            return false;
        };
        let input = if self.has_shift_down() {
            Input::QuickMove
        } else {
            Input::Pickup
        };
        if self.client.screen == Screen::Creative {
            // CreativeModeInventoryScreen.slotClicked: the Survival Inventory tab clicks the
            // player's own InventoryMenu, and its CreativeInventoryListener sends each slot that
            // changed (SetCreativeModeSlot). No ContainerClick: a server holding the carried
            // stack would give it back after the Destroy Item slot cleared it (#3307).
            self.click_open_inventory(Some(internal), i32::from(right), input);
            return true;
        }
        self.remote_menu_ops(&[(Some(internal), i32::from(right), input)]);
        true
    }
    /// `Consumable.onConsumeTick` for another player using a consumable: the
    /// crumbs at their mouth every 4 ticks once 21.875% of its use has passed
    /// (`shouldEmitParticlesAndSounds`; the sounds come from the server).
    pub(in crate::game) fn remote_player_eating(
        &mut self,
        item: &str,
        ticks: u32,
        at: [f64; 3],
        eye: f64,
        pitch: f32,
        yaw: f32,
    ) {
        let Some(consumable) = self
            .res
            .items()
            .get(item)
            .and_then(|i| i.consumable.as_ref())
            .filter(|c| c.particles)
        else {
            return;
        };
        let duration = consumable.ticks().max(0) as u32;
        let remaining = duration - ticks.min(duration);
        if !consumable.should_emit(remaining as i32) {
            return;
        }
        let options = self.item_particle(&ItemStack::new(item, 1));
        let mut spawned = Vec::new();
        sunrise_client::particles::item_use_particles(
            &mut self.world.random,
            pitch,
            yaw,
            at,
            at[1] + eye,
            5,
            |p, v| spawned.push((p, v)),
        );
        for (p, v) in spawned {
            self.particles.0.add(&options, p, v);
        }
    }
    /// Continue Minecraft.startUseItem after this hand's target passed:
    /// its item, then the off hand's target and item if this is the main hand.
    /// When resumed after a server answer, use the original block hit.
    pub(in crate::game) fn remote_use_hands(
        &mut self,
        hand: Hand,
        attempt: mod_api::InteractionAttempt,
        block_hit: Option<Hit>,
    ) {
        // Nothing in the crosshair: an empty hand's click is heard, by mods only.
        let miss = block_hit.is_none()
            && mod_api::__private::listened(mod_api::HookId::RightClickEmpty)
            && !self.entity_targeted()
            && self.pick().is_none();
        if miss && self.with_hand(hand, |g| g.held().is_none()) {
            self.native_right_click_empty(hand);
        }
        // A successful Item.use always drops the hand (LocalPlayer.itemUsed).
        if self.remote_use_item(hand) {
            self.client.heights.item_used(hand);
            return;
        }
        if hand == Hand::Off {
            return;
        }
        // The off hand's turn: the event again, then the entity or the block.
        if let Some(swing) = self
            .native_interaction(InteractionKey::Use, Hand::Off, attempt)
            .canceled_swing()
        {
            if swing {
                self.swing_hand(Hand::Off, SwingAnimation::DEFAULT);
            }
            return;
        }
        let off_target = if block_hit.is_none() && self.remote_use_targets_entity() {
            self.remote_use_entity(Hand::Off)
        } else {
            self.remote_use_block(Hand::Off, attempt, block_hit)
        };
        if off_target {
            return;
        }
        if miss && self.offhand.is_none() {
            self.native_right_click_empty(Hand::Off);
        }
        if self.remote_use_item(Hand::Off) {
            self.client.heights.item_used(Hand::Off);
        }
    }
}
