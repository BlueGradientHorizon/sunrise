#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// Runs one interaction and restores the caller's swing policy afterwards, on every return path.
    pub(in crate::game) fn with_interaction_swing_scope<R>(
        &mut self,
        action: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous = self.remote.as_ref().map(|remote| remote.interaction_swing);
        let result = action(self);
        if let (Some(remote), Some(previous)) = (self.remote.as_mut(), previous) {
            remote.interaction_swing = previous;
        }
        result
    }

    pub(in crate::game) fn remote_use(&mut self, attempt: mod_api::InteractionAttempt) {
        self.with_interaction_swing_scope(|game| game.remote_use_inner(attempt));
    }
    pub(in crate::game) fn remote_start_attack(&mut self) -> bool {
        // An agent's `attack ID` checks its entity against the pick its click is taken with,
        // and its click on another target is dropped before it acts.
        if self.agent_attack_click_refused() {
            return false;
        }
        self.with_interaction_swing_scope(Game::remote_start_attack_inner)
    }
    pub(in crate::game) fn remote_continue_attack(&mut self, down: bool) {
        self.with_interaction_swing_scope(|game| game.remote_continue_attack_inner(down));
    }
    /// `ServerPlayer.dismountTo` lands a rider at the ghast's own (x, maxY, z)
    /// (`HappyGhast.getDismountLocationForPassenger`). A vanilla client
    /// simulates the ghast it rides, so its copy is where the server has it;
    /// this window's copy trails the server's by the stepped interpolation and
    /// would rise into the rider's feet, which lets the rider fall through.
    /// Put the copy under the landing spot.
    pub(in crate::game) fn place_ghast_under(&mut self, id: u32, spot: Vec3) {
        let Some(m) = self
            .mobs
            .iter_mut()
            .find(|m| m.id == id && m.species() == sunrise_client::mobs::Species::HappyGhast)
        else {
            return;
        };
        let at = Vec3::new(spot.x, spot.y - f64::from(m.height()), spot.z);
        // Only a teleport that is that dismount: right above the ghast.
        if (at.x - m.position.x).powi(2) + (at.z - m.position.z).powi(2) > 25.0
            || (at.y - m.position.y).abs() > 5.0
        {
            return;
        }
        m.position = at;
        m.previous = at;
        if let Some(handler) = self
            .remote
            .as_mut()
            .and_then(|r| r.stepped.get_mut(&(id as i32)))
        {
            handler.cancel();
        }
    }
    /// `ClientPacketListener.handleLookAt`: `Entity.lookAt` from this
    /// player's feet or eyes, towards the packet's entity (its feet or eyes)
    /// when this client tracks it, else towards the packet's point.
    pub(in crate::game) fn remote_look_at(&mut self, p: &play::PlayerLookAt) {
        // `EntityAnchorArgument.Anchor`: 0 feet, 1 eyes.
        const EYES: i32 = 1;
        let Some(player) = self.player.as_ref() else {
            return;
        };
        let mut from = [player.position.x, player.position.y, player.position.z];
        if p.from == EYES {
            from[1] += f64::from(player.dimensions.eye_height);
        }
        let to = p
            .entity
            .as_ref()
            .and_then(|e| {
                let m = self.mobs.iter().find(|m| m.id == e.id as u32)?;
                let y = if e.anchor == EYES {
                    m.eye_y()
                } else {
                    m.position.y
                };
                Some([m.position.x, y, m.position.z])
            })
            .unwrap_or(p.target);
        (self.yaw, self.pitch) = look_at_rotation(from, to);
    }
    /// The shared dry-level boundary for a normal block use or the empty
    /// method requested by a canceled `BLOCK` phase's `TryWithEmptyHand`.
    /// The `useItemOn` answers a client level gives without changing a block, for either hand:
    /// - VaultBlock: something in the hand on an ACTIVE vault is SUCCESS_SERVER (no swing here);
    /// - RedStoneOreBlock: SUCCESS, unless a block item goes in the cell beside it
    ///   (`BlockPlaceContext.canPlace`), PASS;
    /// - PumpkinBlock: shears are SUCCESS (only the server carves it);
    /// - WeatheringCopperGolemStatueBlock: an axe on an unaffected statue is SUCCESS (the server
    ///   lets the golem out).
    pub(in crate::game) fn client_block_answer(
        &self,
        hit: &crate::game::Hit,
        held: Option<&ItemStack>,
    ) -> Option<InteractionResult> {
        if self.vault_takes_click(hit.pos, held) {
            return Some(InteractionResult::Consume);
        }
        let name = self.block_name(hit.pos);
        match &*name {
            "minecraft:redstone_ore" | "minecraft:deepslate_redstone_ore" => {
                let beside = [0, 1, 2].map(|i| hit.pos[i] + hit.face[i]);
                let takes = self
                    .res
                    .facts()
                    .get(self.world.block(beside[0], beside[1], beside[2]))
                    .replaceable;
                let places =
                    held.is_some_and(|s| self.res.items().block_items.contains_key(&*s.item));
                Some(if places && takes {
                    InteractionResult::Pass
                } else {
                    InteractionResult::Success
                })
            }
            "minecraft:pumpkin" => held
                .is_some_and(|s| &*s.item == "minecraft:shears")
                .then_some(InteractionResult::Success),
            "minecraft:copper_golem_statue" => held
                .is_some_and(|s| self.item_in_tag("minecraft:axes", &s.item))
                .then_some(InteractionResult::Success),
            _ => None,
        }
    }
    /// MultiPlayerGameMode.useItemOn inside startPrediction, for a block item:
    /// the blocks `BlockItem.place` set at `pos` are placed on this client's
    /// level at once, so the next click of a fast bridge aims at them rather
    /// than past them (the server would refuse it). Its place sound is the
    /// placer's own (`playSound(player, ...)`: the server sends it to the
    /// others only).
    pub(in crate::game) fn predict_place(
        &mut self,
        placed: &DryPlace,
        stack: &ItemStack,
        seq: i32,
    ) {
        let pos = placed.pos;
        let at = [pos.x, pos.y, pos.z];
        // The dry simulation puts the stack state in the primary write. The
        // live copy takes item components before replaying the other cells.
        let Some((primary, rest)) = placed.changes.split_first() else {
            return;
        };
        self.predict_blocks(std::slice::from_ref(primary), seq);
        if placed.matches_candidate {
            self.apply_predicted_item(at, stack);
        }
        self.predict_blocks(rest, seq);
        let state = primary.1;
        if !self.res.facts().is_air(state) {
            let sound = self.own_sound_type(at, state, mod_api::BlockSoundCause::Place);
            self.play_client_local_sound(
                LevelSound::new(
                    &sound.place,
                    at.map(|n| f64::from(n) + 0.5),
                    (sound.volume + 1.0) / 2.0,
                    sound.pitch * 0.8,
                )
                .in_source(mod_api::SoundSource::Blocks),
            );
        }
    }
    /// Whether `held`'s `Item.use` needs a target this client sees none of
    /// (not SUCCESS): BucketItem.use (a fluid source or powder snow to pick
    /// up through `ClipContext.Fluid.SOURCE_ONLY`, a block to empty a full
    /// one against, `mayUseItemAt`), BottleItem.use (water to fill from) and
    /// BoatItem.use (a block or fluid surface with room for the boat).
    pub(in crate::game) fn use_finds_no_target(&self, held: &ItemStack) -> bool {
        let id = &*held.item;
        let facts = |pos: [i32; 3]| {
            self.res
                .facts()
                .get(self.world.block(pos[0], pos[1], pos[2]))
        };
        let source = |want: Option<Fluid>| {
            let fluid_hit = self.pick_fluid();
            // pick_fluid steps through waterlogged blocks, but their water is
            // a source the SOURCE_ONLY ray stops at (and a BucketPickup).
            if let Some(hit) = self.pick()
                && facts(hit.pos).waterlogged
            {
                let eye = self.aim().map_or(hit.point, |(eye, _)| eye);
                let d = |p: [f64; 3]| (0..3).map(|a| (p[a] - eye[a]).powi(2)).sum::<f64>();
                if fluid_hit
                    .as_ref()
                    .is_none_or(|fluid| d(fluid.point) >= d(hit.point))
                {
                    return want.is_none_or(|want| want == Fluid::Water);
                }
            }
            fluid_hit.is_some_and(|hit| {
                let f = facts(hit.pos);
                let fluid = f.fluid != Fluid::None && f.fluid_level == 8 && !f.fluid_falling;
                match want {
                    Some(want) => fluid && f.fluid == want,
                    None => fluid || self.is_powder_snow(hit.pos),
                }
            })
        };
        // SolidBucketItem only has useOn: its use is Item.use's PASS.
        if id == "minecraft:powder_snow_bucket" {
            return true;
        }
        if id == "minecraft:bucket" {
            return !self.may_build() || !source(None);
        }
        if sunrise_client::items::bucket_contents(id).is_some() {
            return !self.may_build() || self.pick().is_none();
        }
        if id == "minecraft:glass_bottle" {
            // A dragon's breath cloud never counts here: its owner is not
            // synced, so this client's BottleItem.use finds none and PASSes;
            // the packet still goes and the server fills the bottle.
            return !source(Some(Fluid::Water));
        }
        if crate::game::boats::any_boat_wood(id).is_some() {
            return self.boat_to_place(held).is_none();
        }
        false
    }
    /// Whether `stack` stays in use until the button lets go (bows,
    /// crossbows, tridents, spyglasses, shields, eyes of ender, whose
    /// `getUseDuration` 0 never counts down to the end) or the server ends
    /// it (a mod's item): its use only counts ticks here.
    pub(in crate::game) fn held_until_released(&self, stack: &ItemStack) -> bool {
        if self.mod_use(stack) {
            return true;
        }
        matches!(
            &*stack.item,
            "minecraft:bow"
                | "minecraft:crossbow"
                | "minecraft:trident"
                | "minecraft:spyglass"
                | "minecraft:ender_eye"
        ) || sunrise_client::spear::Kinetic::of(&stack.item).is_some()
            || self.blocks_attacks(stack).is_some()
            || self.patched_held_use(stack)
    }
    pub(in crate::game) fn remote_handle_input(&mut self) {
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        let stop = std::mem::take(&mut r.stop_mining);
        let ready = r.loaded_sent && self.sleeping.is_none() && !self.mod_mappings_screen_open();
        self.refresh_client_pick(1.0);
        if stop || !ready {
            self.stop_remote_mining();
        }
        if ready {
            sunrise_client::interaction_input::tick(&mut RemoteInputWorld(self));
        }
        self.finish_agent_attack_input(ready);
        if ready && self.clicks.miss_time > 0 {
            self.clicks.miss_time -= 1;
        }
    }
    /// `MultiPlayerGameMode.releaseUsingItem`: sends `RELEASE_USE_ITEM` and
    /// runs the client half of `Item.releaseUsing` (a Riptide launch moves the
    /// player itself). Vanilla reaches it both when the use key is let go and,
    /// in the same tick, when a keybind opens a screen.
    pub(in crate::game) fn release_remote_use(&mut self) {
        self.send_action(6);
        // a mod's Stop may skip the item's release, and a mod's own release follows.
        let hand = self.used_hand();
        if let Some((slot, ticks)) = self.using.take()
            && let Some(hand) = hand
            && let Some(stack) = self.stack_in(slot).cloned()
        {
            self.with_hand(hand, |g| {
                if !g.use_item_stopped(&stack, ticks) {
                    return;
                }
                if &*stack.item == "minecraft:trident" {
                    g.release_trident(g.ticks_held(&stack, ticks));
                }
                g.mod_release_using(&stack, ticks);
            });
        }
    }
}

impl Game<'_> {
    /// `Entity.isColliding(pos, state)` for the local player: the state's own
    /// collision shape, at the player's pose, against the player's box.
    pub(in crate::game) fn block_meets_player(&self, pos: [i32; 3], state: BlockStateId) -> bool {
        let Some(player) = self.player.as_ref() else {
            return false;
        };
        let entity = player.bounds();
        let context = sunrise_world::collision_context::CollisionContext::for_entity(
            sunrise_world::collision_context::CollisionEntity {
                identity: sunrise_world::entity_obstruction::EntityIdentity(0),
                kind: sunrise_world::collision_context::EntityContextKind::Player,
                y: player.position.y,
                descending: player.shift_key_down,
                main_hand: None,
            },
            false,
        );
        let mut scene = sunrise_client::collisions::SceneCollisions {
            world: &self.world,
            binding: self.collision,
            subject: None,
        };
        let at = sunrise_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        match self
            .collision
            .collision_shape_with_context(&mut scene, state, at, &context)
        {
            Ok(shape) => shape_meets(&shape, pos, entity),
            Err(_) => false,
        }
    }

    /// Whether a main-hand right click with `held` would end on the block hit, for the gameplay
    /// guide: what `remote_block_reacts` answers on its dry level, by the templates' rules alone,
    /// so looking asks no mod's placement or survival hook.
    pub(in crate::game) fn block_takes_click(
        &mut self,
        hit: &super::super::Hit,
        held: Option<&ItemStack>,
    ) -> bool {
        self.remote_block_reacts(hit, held, false)
            .0
            .consumes_action()
    }

    /// `InteractionResult.CONSUME` for this click: the block keeps it and does nothing, so the
    /// item in hand never gets it either (an empty flower pot, a pressed button). For the
    /// guide's look, as `block_takes_click`.
    pub(in crate::game) fn block_swallows_click(
        &mut self,
        hit: &super::super::Hit,
        held: Option<&ItemStack>,
    ) -> bool {
        matches!(
            self.remote_block_reacts(hit, held, false).0,
            InteractionResult::Consume
        )
    }

    /// The idle controller guide cannot know what these Parts will decide on a click.
    pub(in crate::game) fn can_hint_block_placement(stack: &ItemStack) -> bool {
        use mod_api::{__private::item_listens, HookId};
        !item_listens(HookId::PlacementContext, &stack.item)
            && !item_listens(HookId::MustSurvive, &stack.item)
    }

    /// Run BlockItem.place on a disposable level for a click: where it placed
    /// and the blocks it set. The real level supplies collision shapes and
    /// building-blocking entities; random draws, scheduled ticks and events
    /// stay in the preview. `ItemStack.useOn` runs it in adventure mode only
    /// when the used stack's `can_place_on` names the clicked block.
    pub(in crate::game) fn remote_can_place(
        &mut self,
        hit: &super::super::Hit,
        stack: &ItemStack,
    ) -> Option<DryPlace> {
        self.remote_place_preview(hit, stack, true)
    }

    /// Whether a click would place `stack` (the controller guide's look): as
    /// `remote_can_place`, by the templates' rules alone, so looking asks no
    /// mod's placement or survival hook.
    pub(in crate::game) fn remote_would_place(
        &mut self,
        hit: &super::super::Hit,
        stack: &ItemStack,
    ) -> bool {
        self.remote_place_preview(hit, stack, false).is_some()
    }
}
