#[allow(unused_imports)]
use super::*;

/// The creative-slot wire codec bounds a stack's count to 1..=99; an item whose
/// own max stack size a native mod raised past 99 becomes the ceiling instead.
pub(in crate::game) fn creative_count_bound(item_max_stack: u32) -> u32 {
    item_max_stack.max(99)
}

/// Whether a creative stack's count is past the bound its item's own max stack gives it.
pub(in crate::game) fn creative_count_exceeds(count: u32, own_max: u32) -> bool {
    count > creative_count_bound(own_max)
}

impl Game<'_> {
    /// DedicatedServer.isUnderSpawnProtection: no operators, an operator or a
    /// radius of 0 or less is never under it; else a block within the radius
    /// of the world spawn (square, in the Overworld) is.
    pub(in crate::game) fn under_spawn_protection(&self, gi: usize, pos: [i32; 3]) -> bool {
        let Some(host) = self.lan.as_ref() else {
            return false;
        };
        let g = &host.hub.guests[gi];
        self.spawn_protection_for(host, g.uuid)
            .is_some_and(|radius| self.spawn_protects(radius, &g.dimension, pos))
    }

    pub(in crate::game) fn spawn_protection_for(&self, host: &LanHost, uuid: u128) -> Option<i32> {
        let radius = host.hub.spawn_protection;
        (self.is_dedicated_server()
            && radius > 0
            && !host.hub.ops.is_empty()
            && !host.hub.ops.contains_key(&uuid))
        .then_some(radius)
    }

    pub(in crate::game) fn spawn_protects(
        &self,
        radius: i32,
        dimension: &str,
        pos: [i32; 3],
    ) -> bool {
        let Some(spawn) = self.level.world_spawn else {
            return false;
        };
        let dx = pos[0].abs_diff(spawn[0]);
        let dz = pos[2].abs_diff(spawn[2]);
        dimension == "minecraft:overworld" && dx.max(dz) <= radius as u32
    }

    /// ServerPlayer.sendSpawnProtectionMessage.
    pub(in crate::game) fn spawn_protection_message(&self, conn: u32, pos: [i32; 3]) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let message = sunrise_client::text::Text::translate(
            "build.spawn_protection",
            &[sunrise_client::text::Text::literal(format!(
                "{}, {}, {}",
                pos[0], pos[1], pos[2]
            ))],
        )
        .with_color(sunrise_client::text::RED);
        host.hub.send_to(
            conn,
            &C::SystemChat(play::SystemChat {
                content: sunrise_client::lan::data::component_nbt(&message.component()),
                overlay: true,
            }),
        );
    }

    /// `ServerGamePacketListenerImpl.wasBlockPlacementAttempt`: a block item,
    /// or a bucket with something in it, that is not cooling down.
    pub(in crate::game) fn was_block_placement_attempt(&mut self, hand: Hand) -> bool {
        let Some(stack) = self.with_hand(hand, |g| g.held().cloned()) else {
            return false;
        };
        let Some(info) = self.items().get(&stack.item).cloned() else {
            return false;
        };
        (info.block.is_some() || sunrise_client::items::bucket_contents(&stack.item).is_some())
            && !self
                .cooldowns
                .is_on_cooldown(&sunrise_client::cooldowns::group(
                    &stack,
                    info.use_cooldown.as_ref(),
                ))
    }

    /// `ServerPlayer.sendBuildLimitMessage`: the red overlay with the limit.
    pub(in crate::game) fn build_limit_message(&self, conn: u32, too_high: bool, limit: i32) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let message = sunrise_client::text::Text::translate(
            if too_high {
                "build.tooHigh"
            } else {
                "build.tooLow"
            },
            &[sunrise_client::text::Text::literal(limit.to_string())],
        )
        .with_color(sunrise_client::text::RED);
        host.hub.send_to(
            conn,
            &C::SystemChat(play::SystemChat {
                content: sunrise_client::lan::data::component_nbt(&message.component()),
                overlay: true,
            }),
        );
    }

    /// A guest's client started or stopped flying (the hub applied it): the
    /// mods hear it with the guest's state swapped in, and when one cancels,
    /// the guest keeps what they had and is sent their abilities again, as
    /// vanilla answers what it refuses.
    pub(in crate::game) fn lan_flight_toggled(&mut self, conn: u32, flying: bool) {
        let Some((uuid, name)) = self
            .lan
            .as_ref()
            .and_then(|host| host.hub.guest(conn))
            .map(|g| (g.uuid, g.name.clone()))
        else {
            return;
        };
        let mut cancel = false;
        // With the guest's state swapped in, so that the helpers reach them.
        self.as_guest(conn, |g| {
            g.native_player_toggle_flight(uuid, &name, flying, &mut cancel);
        });
        if !cancel {
            return;
        }
        if let Some(host) = self.lan.as_mut() {
            let _ = host.hub.update_guest(conn, |g| {
                g.abilities.flying = !flying;
                g.send(&g.abilities_packet());
            });
        }
    }

    /// A packet from a guest that the hub does not handle itself.
    pub(in crate::game) fn lan_guest_packet(&mut self, conn: u32, packet: S) {
        // The window's ordinary reply cannot fix a predicted swap in a player slot
        // it doesn't show. Prepare the repair even if menu validation closes the window.
        const OFFHAND_BUTTON: i8 = crate::game::shield::OFFHAND_SLOT as i8;
        let swap = if let S::ContainerClick(click) = &packet
            && click.input == sunrise_client::lan::menu::input::SWAP
            && matches!(click.button, 0..=8 | OFFHAND_BUTTON)
        {
            use super::super::lan_menus::ClickReply;
            self.lan
                .as_ref()
                .and_then(|host| host.hub.guest(conn))
                .and_then(|guest| guest.menu.as_ref())
                .filter(|menu| {
                    menu.window == click.container
                        && usize::try_from(click.slot).is_ok_and(|slot| slot < menu.slot_count())
                })
                .and_then(|menu| {
                    if menu.mod_type.is_some() && menu.player_slots() == 0 {
                        Some(ClickReply::Inventory)
                    } else if click.button == OFFHAND_BUTTON
                        && (menu.mod_type.is_some() || menu.slots == Slots::Native)
                    {
                        Some(ClickReply::Offhand)
                    } else {
                        None
                    }
                })
        } else {
            None
        };
        self.lan_guest_packet_inner(conn, packet);
        if let Some(reply) = swap {
            self.reply_menu_click(conn, reply);
        }
    }

    /// `ServerLevel.getEntity(id)`'s `Entity.saveWithoutId`: the player or
    /// other entity of the level in place with that network id.
    pub(in crate::game) fn level_entity_tag(&mut self, id: i32) -> Option<sunrise_core::nbt::Tag> {
        use crate::game::commands::Target;
        let host = self.lan.as_ref()?;
        let player = host
            .hub
            .guests
            .iter()
            .find(|g| g.entity_id == id && host.hub.here(g))
            .map(|g| g.conn);
        let mut tag = match player {
            Some(player) => self
                .as_guest(player, |g| g.command_target_nbt(Target::Player))
                .flatten()?,
            None => {
                let target = self.level_entity(id)?;
                self.command_target_nbt(target)?
            }
        };
        if let sunrise_core::nbt::Tag::Compound(c) = &mut tag {
            c.remove(&"id".into());
        }
        Some(tag)
    }

    /// `ServerGamePacketListenerImpl.tryPickItem` for the guest: a matching stack
    /// is selected, else a creative player gets the item; the held slot is sent
    /// back and the changed slots follow with the tick's inventory changes.
    pub(in crate::game) fn guest_pick_item(
        &mut self,
        conn: u32,
        from_entity: bool,
        pos: [i32; 3],
        entity_id: i32,
        stack: impl FnOnce(&mut Self) -> Option<ItemStack>,
    ) {
        let Some((uuid, name)) = self
            .lan
            .as_ref()
            .and_then(|host| host.hub.guest(conn))
            .map(|g| (g.uuid, g.name.clone()))
        else {
            return;
        };
        let picked = self.as_guest(conn, |g| {
            let stack = stack(g)?;
            let infinite = g.creative;
            let mut cancel = false;
            g.native_player_pick_item(uuid, &name, from_entity, pos, entity_id, &mut cancel);
            if cancel {
                return None;
            }
            g.pick_into_inventory(&stack, infinite);
            Some(())
        });
        if picked.flatten().is_some()
            && let Some(host) = self.lan.as_ref()
            && let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn)
        {
            host.hub.send_to(
                conn,
                &C::SetHeldSlot(play::SetHeldSlot {
                    slot: g.selected as i32,
                }),
            );
        }
    }

    /// What ServerboundSetCreativeModeSlotPacket asks of a stack. Its codec
    /// (`ItemStack.validatedStreamCodec`) encodes the stack with `ItemStack.CODEC`, whose count is
    /// 1 to 99: more fails the packet and disconnects the player, as vanilla's decoder error does.
    /// A native mod can raise an item's own stack size past 99, which raises the ceiling instead.
    /// handleSetCreativeModeSlot's `validData` then ignores a stack beyond its own stack size.
    pub(in crate::game) fn creative_stack_valid(&mut self, gi: usize, s: &ItemStack) -> bool {
        let size = self.stack_max_size(s);
        let max = creative_count_bound(size);
        let Some(host) = self.lan.as_mut() else {
            return false;
        };
        if creative_count_exceeds(s.count, size) {
            let g = &host.hub.guests[gi];
            let conn = g.conn;
            let reason = format!("Invalid packet: count {} is not in 1..={max}", s.count);
            eprintln!("{} sent an invalid creative stack: {reason}", g.name);
            host.hub
                .kick(conn, sunrise_protocol::text::literal(&reason));
            return false;
        }
        s.count <= size
    }

    pub(in crate::game) fn item_from_network(
        &self,
        stack: &Option<sunrise_protocol::item::Stack>,
    ) -> Option<ItemStack> {
        let s = stack.as_ref()?;
        let mut out = self.lan.as_ref()?.hub.stack_from_network(s);
        out.count = out.count.max(1);
        Some(out)
    }

    /// `ServerGamePacketListenerImpl.handleTeleportToEntityPacket`: search the live levels in
    /// server order, then teleport to the first matching entity's position and rotation.
    pub(in crate::game) fn guest_spectate_teleport(&mut self, gi: usize, uuid: u128) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let guest = &host.hub.guests[gi];
        let (conn, from) = (guest.conn, Dim::from_id(&guest.dimension));
        let uuid = sunrise_client::lan::playerdata::uuid_ints(uuid);
        let Some((dim, target)) = Dim::all().into_iter().find_map(|dim| {
            self.level_target_by_uuid(dim, uuid)
                .map(|target| (dim, target))
        }) else {
            return;
        };
        // Only enter another level to read a found entity's pose, not to probe every level.
        let back = if dim == self.dim {
            None
        } else if let Some(back) = self.enter_command_level(dim) {
            Some(back)
        } else {
            return;
        };
        let at = self.target_position(target);
        let [yaw, pitch] = self.target_rotation(target);
        if let Some(back) = back
            && self.enter_level(back).is_err()
        {
            return;
        }
        let Some(at) = at else {
            return;
        };
        // ServerPlayer.teleportTo resets the spectator's camera before traveling.
        self.guest_set_camera(gi, None);
        if from != dim {
            if !self.server_player_travel_allowed(
                conn,
                dim,
                crate::game::server_travel::TravelCause::Teleport,
            ) {
                return;
            }
            self.apply_direct_level_teleport(crate::game::server_travel::LevelTeleport {
                conn,
                dim,
                at,
                rotation: Some([yaw, pitch]),
                vehicle: None,
            });
            return;
        }
        self.as_guest(conn, |game| {
            game.place_player_teleport_in_level(
                Vec3::new(at[0], at[1], at[2]),
                Some([yaw, pitch]),
                [false; 3],
            )
        });
    }

    /// Ordinary click routing. A scoped override calls this inside its action on Continue;
    /// an unsubscribed click uses it directly. False means the route refused the click.
    pub(in crate::game) fn lan_default_container_click(
        &mut self,
        conn: u32,
        p: &play::ContainerClick,
    ) -> bool {
        let Some(gi) = self.guest_index(conn) else {
            return false;
        };
        let guest = &self.lan.as_ref().unwrap().hub.guests[gi];
        if let Some(open) = guest.menu.as_ref().filter(|m| m.window == p.container) {
            if open.mod_type.is_some()
                && usize::try_from(p.slot).is_ok_and(|s| s >= open.slot_count())
            {
                return false;
            }
            if open.slots == Slots::Native && open.owned_slots().is_none() && !guest.spectator() {
                // Display menus do not move items unless their slots were explicitly backed.
                if let Some(applied) = self.place_menu_click(
                    guest.uuid,
                    p.container,
                    i32::from(p.slot),
                    i32::from(p.button),
                    p.input,
                ) {
                    return applied;
                }
                self.reply_predicted_menu_click(conn, p, Vec::new());
                return true;
            }
        }
        if let Some(applied) = self
            .lan_owned_menu_click(conn, p)
            .or_else(|| self.lan_mod_quick_move(conn, p, None))
        {
            return applied;
        }
        if self.lan_merchant_click(conn, p)
            || self.lan_inventory_click(conn, p)
            || self.lan_crafting_click(conn, p)
        {
            return true;
        }
        if let Some(applied) = self.lan_machine_click(conn, p) {
            return applied;
        }
        if self.lan_station_click(conn, p)
            || self.lan_lectern_click(conn, p)
            || self.lan_mount_click(conn, p)
        {
            return true;
        }
        let Some(host) = self.lan.as_ref() else {
            return false;
        };
        let slots = host.hub.guests[gi]
            .menu
            .as_ref()
            .filter(|m| m.window == p.container)
            .map(|m| m.slots);
        let before = slots.and_then(|s| s.read(&self.world, &host.hub.guests[gi]));
        let Some(drops) = self.lan_hub_click(conn, p) else {
            return false;
        };
        let Some(host) = self.lan.as_ref() else {
            return false;
        };
        if let Some(Slots::Block(key) | Slots::Double(key, _)) = slots
            && slots.and_then(|s| s.read(&self.world, &host.hub.guests[gi])) != before
        {
            self.after_commit(super::super::transactions::After::Changed(key));
        }
        if let Some(Slots::Block(key)) = slots {
            self.after_commit(super::super::transactions::After::StoreCartItems(key));
        }
        if !drops.is_empty() {
            self.after_commit(super::super::transactions::After::GuestMenu(
                conn,
                Box::new(super::super::lan_crafting::GuestMenuEffects::dropped(drops)),
            ));
        }
        true
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::{creative_count_bound, creative_count_exceeds};

    #[test]
    fn creative_count_bound_kicks_vanilla_100_but_not_modded_own_size() {
        // A vanilla item (own max stack 64 or 1) stays on the wire's 1..=99 bound.
        for own_max in [1, 64] {
            let bound = creative_count_bound(own_max);
            assert_eq!(bound, 99);
            assert!(99 <= bound, "99 must not kick a vanilla item");
            assert!(100 > bound, "100 must kick a vanilla item");
        }
        // An item a native mod raised past 99 is allowed up to its own max stack.
        let bound = creative_count_bound(500);
        assert_eq!(bound, 500);
        assert!(500 <= bound, "500 must not kick a modded item");
        assert!(501 > bound, "501 must kick a modded item");
    }

    #[test]
    fn creative_count_exceeds_the_bound_of_the_items_own_max() {
        assert!(creative_count_exceeds(100, 64));
        assert!(creative_count_exceeds(100, 1));
        assert!(!creative_count_exceeds(99, 64));
        assert!(!creative_count_exceeds(500, 500));
        assert!(creative_count_exceeds(501, 500));
    }
}
