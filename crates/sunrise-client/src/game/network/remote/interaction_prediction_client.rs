#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::interaction_prediction::*;

impl Game<'_> {
    /// `Minecraft.startUseItem`'s entity turn for `hand`: Interact goes to
    /// the server, and whether the click ends there is what this client's
    /// copy of the entity answers (`MultiPlayerGameMode.interact`).
    pub(in crate::game) fn remote_use_entity(&mut self, hand: Hand) -> bool {
        // A menu this use opens (a chest minecart's) is no clicked block's.
        if let Some(r) = self.remote.as_mut() {
            r.use_target = None;
        }
        let spectator = self.spectator();
        let sneaking = self.remote_shift_down();
        let interact_packet = |id, location| {
            S::Interact(play::Interact {
                id,
                hand: hand_id(hand),
                location,
                secondary: sneaking,
            })
        };
        if self.uses_stored_pick() || spectator {
            let Some(mod_api::CrosshairTarget::Entity { entity, point }) = self.crosshair_target()
            else {
                return false;
            };
            let Some(([lo, hi], origin)) = self.use_entity_geometry(entity) else {
                return false;
            };
            // Minecraft.startUseItem checks the target's blockPosition() against the world
            // border before range/interaction and returns from the whole click when outside.
            let block = sunrise_core::position::BlockPos::containing(Vec3::new(
                origin[0], origin[1], origin[2],
            ));
            if !self.border.contains(f64::from(block.x), f64::from(block.z)) {
                return true;
            }
            let Some(eye) = self.eye_position() else {
                return false;
            };
            // Client interact range: closest box point, strict bound and no server buffer.
            // Refuse this use without choosing another target or the block behind it.
            let distance_squared = (0..3)
                .map(|i| (eye[i] - eye[i].clamp(lo[i], hi[i])).powi(2))
                .sum::<f64>();
            if distance_squared >= self.entity_reach.powi(2) {
                return false;
            }
            if spectator {
                // MultiPlayerGameMode.interact sends for either hand before returning PASS.
                let location = std::array::from_fn(|i| point[i] - origin[i]);
                self.send(interact_packet(entity.id() as i32, location));
                return false;
            }
        }
        // A boat, minecart, item frame, armor stand or painting nearer than
        // any mob: the server uses it (AbstractBoat.interact boards it).
        if let Some(hit) = self.remote_object_hit()
            && self.mob_hit().is_none_or(|(_, d, _)| hit.distance < d)
            && self.before_block(hit.distance)
        {
            // Interaction.interact answers SUCCESS when it responds and
            // CONSUME otherwise: both end the click (nothing behind the box
            // is used), but only SUCCESS swings; the packet carries the hand.
            let response = self.client.interactions.get(&hit.id).map(|i| i.response);
            // Objects whose interact passes (paintings, empty or fixed item frames,
            // boats sneaked at or out of control, carts that pass) leave each hand's
            // turn to its item: a crossbow aimed at it shoots.
            let is_painting = self.paintings.iter().any(|p| p.id as i32 == hit.id);
            let held = self.with_hand(hand, |g| g.held().cloned());
            let frame_passes = self
                .item_frames
                .iter()
                .find(|f| f.id as i32 == hit.id)
                .is_some_and(|f| {
                    item_frame_interact_passes(f.fixed, f.item.is_some(), held.is_some())
                });
            let boat_passes = self
                .boats
                .iter()
                .enumerate()
                .find(|(_, b)| b.id as i32 == hit.id)
                .is_some_and(|(i, b)| {
                    boat_interact_passes(
                        b.chest,
                        sneaking,
                        b.out_of_control >= 60.0,
                        self.boat_has_room(i),
                    )
                });
            let (cart_passes, is_rideable_cart) = self
                .carts
                .iter()
                .find(|c| c.id as i32 == hit.id)
                .map(|c| {
                    let ridden = self.riding == Some(c.id)
                        || self.mobs.iter().any(|m| m.extra.mount == Some(c.id));
                    (
                        c.kind
                            .interact_passes(sneaking, ridden, self.can_use_game_master_blocks()),
                        c.kind == crate::game::minecarts::CartKind::Rideable,
                    )
                })
                .unwrap_or((false, false));
            // Cushion.interact on a client: PASS when sneaked at or when
            // something already sits on it, CONSUME (no swing) otherwise.
            let is_cushion = self.cushions.iter().any(|c| c.id as i32 == hit.id);
            let cushion_passes = is_cushion
                && (sneaking
                    || self.riding.is_some_and(|r| r as i32 == hit.id)
                    || self
                        .remote
                        .as_ref()
                        .is_some_and(|r| r.has_passengers(hit.id as u32)));
            // EndCrystal, EnderDragonPart, PrimedTnt, and FallingBlock keep Entity.interact's PASS.
            let crystal_passes = self.crystals.iter().any(|c| c.id as i32 == hit.id)
                || self.level_dragon_part(hit.id as u32).is_some();
            let tnt_or_falling_passes = self.tnts.iter().any(|t| t.id as i32 == hit.id)
                || self.falling.iter().any(|f| f.id as i32 == hit.id);
            let passes = self
                .remote
                .as_ref()
                .is_some_and(|r| r.general.rows.contains_key(&hit.id))
                || is_painting
                || frame_passes
                || boat_passes
                || cart_passes
                || cushion_passes
                || crystal_passes
                || tnt_or_falling_passes;
            // Otherwise the main hand's turn takes it; there is no off hand turn.
            if response.is_none() && hand == Hand::Off && !passes {
                return false;
            }
            self.send(interact_packet(hit.id, hit.location));
            if let Some(done) = self.entity_interaction(hand, hit.id) {
                return done;
            }
            if passes {
                return false;
            }
            // Minecart.interact resets the passenger's easing baseline on
            // a successful boarding click, not on SetPassengers (commands
            // can mount a player without this interaction).
            if let Some(i) = self.carts.iter().position(|c| {
                c.id as i32 == hit.id && c.kind == crate::game::minecarts::CartKind::Rideable
            }) {
                self.carts
                    .update(i, |c| c.player_rotation_offset = c.rotation_offset);
            }
            // MinecartCommandBlock.interact on the client: the editor opens.
            if self.carts.iter().any(|c| {
                c.id as i32 == hit.id && c.kind == crate::game::minecarts::CartKind::CommandBlock
            }) {
                self.open_cart_command_block(hit.id as u32);
            }
            // Minecart.interact boards the cart and answers CONSUME on the client,
            // which ends the click without an arm swing. AbstractBoat.interact
            // answers SUCCESS there, which swings. Cushion.interact answers
            // CONSUME there too.
            if remote_object_swings(response, is_rideable_cart || is_cushion) {
                self.with_hand(hand, Game::swing_arm);
            }
            return true;
        }
        if let Some((i, distance, location)) = self.mob_hit()
            && self.before_block(distance)
        {
            let id = self.mobs[i].id as i32;
            self.send(interact_packet(id, location));
            if let Some(done) = self.entity_interaction(hand, id) {
                return done;
            }
            // Player.interactOn: another player passes, and so does a mob
            // the held item means nothing to: the item is used instead.
            let player = self
                .remote
                .as_ref()
                .is_some_and(|r| r.level_players.contains_key(&id));
            if !player && self.remote_mob_takes(i, hand) {
                self.remote_feed_particles(i, hand);
                // As for the objects above, only SUCCESS swings the hand: a
                // CONSUME ends the click without one.
                if self.remote_mob_swings(i, hand) {
                    self.with_hand(hand, Game::swing_arm);
                }
                return true;
            }
            if !player
                && self.with_hand(hand, |g| {
                    g.item_interact_mob(Some(crate::game::InteractTarget { id, location }))
                })
            {
                return true;
            }
        }
        false
    }
    /// Bounding box `[lo, hi]` and world origin `[x, y, z]` of a crosshair entity for
    /// `Minecraft.startUseItem` (including remote/resident leash knots, which have no API body).
    pub(in crate::game) fn use_entity_geometry(
        &self,
        entity: mod_api::EntityRef,
    ) -> Option<([[f64; 3]; 2], [f64; 3])> {
        let knot_block = self
            .client
            .remote_knots
            .get(&entity.id())
            .copied()
            .or_else(|| {
                self.resident_entities
                    .knots
                    .iter()
                    .find(|k| k.id == entity.id())
                    .map(|k| k.fence)
            });
        if let Some(block) = knot_block {
            let bounds = crate::game::pick_entity::knot_bounds(block);
            let p = sunrise_client::mobs::leash::knot_state(block).position;
            let (lo, hi) = (bounds.min_position(), bounds.max_position());
            return Some(([[lo.x, lo.y, lo.z], [hi.x, hi.y, hi.z]], [p.x, p.y, p.z]));
        }
        Some((
            self.entity_bounding_box(entity)?,
            self.entity_position(entity)?,
        ))
    }
}
