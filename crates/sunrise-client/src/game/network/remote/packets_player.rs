#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// `C::SetExperience`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_set_experience_packet(&mut self, p: play::SetExperience) {
        self.experience = Experience {
            level: p.level,
            progress: p.progress,
            total: p.total,
        }
    }

    /// `C::PlayerAbilities`: mutates `Remote::abilities` in arrival order, preserving `may_build`.
    pub(in crate::game) fn apply_player_abilities_packet(&mut self, p: play::PlayerAbilities) {
        self.creative = p.flags & 8 != 0;
        if let Some(r) = self.remote.as_mut() {
            r.abilities.invulnerable = p.flags & 1 != 0;
            r.abilities.flying = p.flags & 2 != 0;
            r.abilities.may_fly = p.flags & 4 != 0;
            r.abilities.instabuild = p.flags & 8 != 0;
            r.abilities.flying_speed = p.flying_speed;
            r.abilities.walking_speed = p.walking_speed;
        }
        self.apply_remote_abilities();
    }

    /// `ClientPacketListener.handleContainerContent` initializes only the addressed menu.
    pub(in crate::game) fn apply_container_set_content_packet(
        &mut self,
        p: play::ContainerSetContent,
    ) {
        let open = self
            .remote
            .as_ref()
            .and_then(|r| r.menu.as_ref())
            .map_or(0, |m| m.0);
        if p.container == 0 || p.container == open {
            self.remote_menu_state(p.container, p.state);
            for (slot, item) in p.items.iter().enumerate() {
                let stack = self.to_stack(item);
                self.set_menu_slot(p.container, slot as i32, stack);
            }
            let carried = self.to_stack(&p.carried);
            if p.container == open {
                self.carried = carried;
            } else if let Some(r) = self.remote.as_mut() {
                r.inventory_carried = carried;
            }
            self.remote_synced();
        }
    }

    /// `C::SetCursorItem`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_set_cursor_item_packet(&mut self, p: play::SetCursorItem) {
        let stack = self.to_stack(&p.item);
        self.tutorial_get_item(stack.as_ref());
        self.carried = stack;
    }

    /// `C::SetPlayerInventory`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_set_player_inventory_packet(
        &mut self,
        p: play::SetPlayerInventory,
    ) {
        let stack = self.to_stack(&p.item);
        self.tutorial_get_item(stack.as_ref());
        let before = self.creative_stacks();
        let held = match p.slot {
            0..=35 => self.inventory[p.slot as usize].as_ref(),
            40 => self.offhand.as_ref(),
            _ => None,
        };
        let changed = !super::super::hand::matches_for_swap(held, stack.as_ref());
        if let Ok(slot) = usize::try_from(p.slot) {
            self.remote_hand_slot_set(slot, changed);
        }
        match p.slot {
            0..=35 => self.inventory[p.slot as usize] = stack,
            36..=39 => self.armor[3 - (p.slot - 36) as usize] = stack,
            // Inventory.EQUIPMENT_SLOT_MAPPING: SLOT_OFFHAND, SLOT_BODY_ARMOR, SLOT_SADDLE.
            40 => self.offhand = stack,
            41 => {
                self.equipment.set(sunrise_items::equipment::BODY, stack);
            }
            42 => {
                self.equipment.set(sunrise_items::equipment::SADDLE, stack);
            }
            _ => {}
        }
        self.remote_synced_since(&before);
    }

    /// `C::SetHeldSlot`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_set_held_slot_packet(&mut self, p: play::SetHeldSlot) {
        // ClientPacketListener.handleSetHeldSlot: a hotbar slot only, and the
        // carried index stays, so the next tick answers with SetCarriedItem.
        if (0..9).contains(&p.slot) {
            self.selected = p.slot as usize;
        }
    }

    /// `C::PlayerInfoUpdate`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_player_info_update_packet(&mut self, p: play::PlayerInfoUpdate) {
        if let Some(r) = self.remote.as_mut() {
            // handlePlayerInfoUpdate: new entries first, then every
            // action on every known entry.
            // INITIALIZE_CHAT: the session (or none) each entry carries; the
            // player's validator is made again when their chat is next read.
            if p.actions & 2 != 0 {
                for e in &p.entries {
                    r.chat_sessions.insert(e.uuid, e.chat_session.clone());
                    r.chat_validators.remove(&e.uuid);
                }
            }
            if p.actions & 1 != 0 {
                for e in &p.entries {
                    if let Some(name) = &e.name {
                        r.tab_players.entry(e.uuid).or_insert_with(|| {
                            sunrise_client::tab_list::TabPlayer::new(e.uuid, name.clone())
                        });
                        r.tab_game_modes
                            .entry(e.uuid)
                            .or_insert_with(|| Arc::new(std::sync::atomic::AtomicI32::new(0)));
                        if let Some(t) = e.properties.iter().find(|p| p.name == "textures") {
                            // Keep the raw property until the services keys settle.
                            r.skin_textures
                                .entry(e.uuid)
                                .or_insert_with(|| Arc::new(PlayerTextures::new(t.clone())));
                        }
                    }
                }
            }
        }
        for e in p.entries {
            let mode_change = e.game_mode.and_then(|mode| {
                let r = self.remote.as_ref()?;
                let info = r.tab_players.get(&e.uuid)?;
                (info.game_mode != mode).then_some((info.game_mode, mode))
            });
            if let Some((current, mode)) = mode_change {
                // LocalPlayer.onGameModeChanged(SPECTATOR) clears vertical motion before PlayerInfo.setGameMode.
                if mode == 3
                    && self.remote.as_ref().is_some_and(|r| {
                        r.profile == e.uuid
                            && (self.player.is_some() || r.has_logical_player || r.logged_in)
                    })
                {
                    if let Some(player) = self.player.as_mut() {
                        player.velocity.y = 0.0;
                    }
                    if let Some(vel) = self
                        .remote
                        .as_mut()
                        .and_then(|r| r.respawn_velocity.as_mut())
                    {
                        vel.y = 0.0;
                    }
                }
                if mod_api::__private::listened(mod_api::HookId::ClientPlayerChangeGameType) {
                    let entity_id = self.remote.as_ref().and_then(|r| {
                        if e.uuid == r.profile {
                            (self.player.is_some() || r.has_logical_player || r.logged_in)
                                .then_some(r.entity_id)
                        } else {
                            r.player_entity(e.uuid).map(|id| id as i32)
                        }
                    });
                    self.native_client_player_change_game_type(entity_id, e.uuid, current, mode);
                }
            }
            if let Some(r) = self.remote.as_mut() {
                if let Some(t) = r.tab_players.get_mut(&e.uuid) {
                    if let Some(mode) = e.game_mode {
                        t.game_mode = mode;
                        r.tab_game_modes
                            .entry(e.uuid)
                            .or_insert_with(|| Arc::new(std::sync::atomic::AtomicI32::new(mode)))
                            .store(mode, std::sync::atomic::Ordering::Relaxed);
                    }
                    if let Some(latency) = e.latency {
                        t.latency = latency;
                    }
                    if let Some(name) = &e.display_name {
                        t.display_name = name.as_ref().map(sunrise_client::scoreboard::tag_to_json);
                    }
                    if let Some(order) = e.list_order {
                        t.list_order = order;
                    }
                    if let Some(hat) = e.show_hat {
                        t.show_hat = hat;
                    }
                    match e.listed {
                        Some(true) => {
                            r.listed.insert(e.uuid);
                        }
                        Some(false) => {
                            r.listed.remove(&e.uuid);
                        }
                        None => {}
                    }
                }
                if let Some(name) = e.name {
                    r.players.insert(e.uuid, name);
                }
                // `Player.isPickable` sees `AbstractClientPlayer.gameMode` as soon as
                // `ClientPacketListener.applyPlayerInfoUpdate` publishes its new mode.
                if mode_change.is_some()
                    && let Some(id) = r.player_entity(e.uuid)
                    && let Some(index) = self.mob_index.find(&self.mobs, id)
                {
                    let spectator = r.remote_player_info_game_mode(id as i32) == Some(SPECTATOR);
                    self.mobs
                        .update(index, |mob| mob.extra.spectator = spectator);
                }
            }
        }
    }

    /// `C::PlayerInfoRemove`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_player_info_remove_packet(&mut self, p: play::PlayerInfoRemove) {
        if let Some(r) = self.remote.as_mut() {
            for u in p.uuids {
                r.players.remove(&u);
                r.tab_players.remove(&u);
                r.tab_game_modes.remove(&u);
                r.chat_sessions.remove(&u);
                r.chat_validators.remove(&u);
                r.skin_textures.remove(&u);
                r.listed.remove(&u);
                r.attachments.remove(&Holder::Player(u));
            }
        }
    }

    /// `C::Commands`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_commands_packet(&mut self, p: play::Commands) {
        let tree = sunrise_client::command::network::from_packet(
            &p,
            sunrise_client::command::vanilla::dispatcher(),
        );
        // The chat's tree follows the server's now; a mod's commands
        // go into it again (`chat_tree_for`).
        let chat_tree = self.chat_tree_for(Some(&tree));
        if let Some(r) = self.remote.as_mut() {
            r.chat_tree = chat_tree;
            r.chat_tree_received = true;
        }
    }

    /// `C::DisguisedChat`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_disguised_chat_packet(&mut self, p: play::DisguisedChat) {
        self.enqueue_chat(QueuedChat {
            signature: None,
            sender: 0,
            kind: QueuedChatKind::Disguised {
                chat_type: p.chat_type,
                message: p.message,
            },
        });
    }

    /// `C::SetDisplayObjective`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_set_display_objective_packet(
        &mut self,
        p: play::SetDisplayObjective,
    ) {
        // DisplaySlot ids: list, sidebar, below_name, then the team sidebars.
        let slot = match p.slot {
            0 => "list".to_owned(),
            2 => "below_name".to_owned(),
            n @ 3..=18 => format!(
                "sidebar.team.{}",
                sunrise_client::text::COLORS[(n - 3) as usize].1
            ),
            _ => "sidebar".to_owned(),
        };
        let board = &mut self.score.board;
        let objective = board.objective(&p.objective).map(|o| o.name.clone());
        board.set_display_objective(&slot, objective.as_deref());
    }

    /// `C::SetScore`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_set_score_packet(&mut self, p: play::SetScore) {
        let board = &mut self.score.board;
        if board.objective(&p.objective).is_some() {
            let s = board.score_mut(&p.owner, &p.objective);
            s.value = p.score;
            s.display = p
                .display
                .as_ref()
                .map(sunrise_client::scoreboard::tag_to_json);
            s.number_format = p.number_format.as_ref().map(number_format);
        }
    }

    /// `C::PlayerChat`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_player_chat_packet(&mut self, p: play::PlayerChat) -> Result<()> {
        // SignedMessageBody.Packed.unpack: the last seen it names must
        // be in the cache; then the message joins it
        // (MessageSignatureCache.push).
        let Some(r) = self.remote.as_mut() else {
            return Ok(());
        };
        // handlePlayerChat: the global index counts every chat packet; one out of
        // order disconnects.
        let expected = r.next_chat_index;
        r.next_chat_index = r.next_chat_index.wrapping_add(1);
        if p.global_index != expected {
            log::error!(
                "Missing or out-of-order chat message from server, expected index {expected} but got {}",
                p.global_index
            );
            let reason = Text::translate("multiplayer.disconnect.bad_chat_index", &[]);
            return self.leave_server(Some(reason));
        }
        let last_seen: Option<Vec<Signature>> =
            p.last_seen.iter().map(|s| r.signatures.unpack(s)).collect();
        let Some(last_seen) = last_seen else {
            log::error!(
                "Message from player with ID {:032x} referenced unrecognized signature id",
                p.sender
            );
            let reason = Text::translate("multiplayer.disconnect.invalid_packet", &[]);
            return self.leave_server(Some(reason));
        };
        let signature = p
            .signature
            .as_deref()
            .and_then(sunrise_protocol::secure_chat::signature);
        r.signatures.push(&last_seen, signature.as_ref());
        // The sender's PlayerInfo: none is an error line; else their
        // `SignedMessageValidator` decides (`updateAndValidate`), and a message it
        // takes without a signature of its own shows as unsigned.
        if !r.tab_players.contains_key(&p.sender) {
            log::error!(
                "Received player chat packet for unknown player with ID: {:032x}",
                p.sender
            );
            self.handle_chat_message_error(p.sender, p.chat_type, signature);
            return Ok(());
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        let incoming = secure_chat::Incoming {
            sender: p.sender,
            index: p.index,
            signature: signature.as_ref(),
            body: secure_chat::Body {
                content: &p.content,
                timestamp: p.timestamp,
                salt: p.salt,
                last_seen: &last_seen,
            },
            unsigned_content: p.unsigned_content.as_ref(),
            filter: &p.filter,
        };
        let Some(validated) = r.validate_chat(&incoming, now) else {
            self.handle_chat_message_error(p.sender, p.chat_type, signature);
            return Ok(());
        };
        let signature = signature.filter(|_| validated == secure_chat::Validated::Signed);
        if p.filter.0 == 1 {
            // `FilterMask.FULLY_FILTERED` can never display or narrate; keep
            // FIFO ordering and its deletion/ack key without text payloads.
            self.enqueue_chat(QueuedChat {
                signature,
                sender: p.sender,
                kind: QueuedChatKind::Filtered,
            });
            return Ok(());
        }
        // ChatListener.handlePlayerChatMessage: Only Show Secure Chat shows the
        // signed content, never the server's replacement, so what the line shows is
        // picked here, at receipt, with the receipt time. The line is decorated, its
        // trust evaluated and shown when the queue accepts the message
        // (`showMessageToPlayer`): a component translates when it is read
        // (`TranslatableContents.decompose`), in the language of that moment.
        let only_show_secure = self.settings.file.bool("onlyShowSecureChat");
        let displayed = if only_show_secure {
            serde_json::Value::from(p.content.as_str())
        } else {
            p.unsigned_content.as_ref().map_or_else(
                || serde_json::Value::from(p.content.as_str()),
                sunrise_client::scoreboard::tag_to_json,
            )
        };
        let signed = signature.is_some();
        let listened = mod_api::__private::listened(mod_api::HookId::ClientChatReceived);
        let info = listened.then(|| Box::new(chat_received::player_chat_info(&p, signed)));
        let received_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        self.enqueue_chat(QueuedChat {
            signature,
            sender: p.sender,
            kind: QueuedChatKind::Player {
                chat_type: p.chat_type,
                signed,
                timestamp: p.timestamp,
                received_ms,
                only_show_secure,
                displayed,
                signed_content: p.content,
                unsigned_content: p.unsigned_content,
                filter: p.filter,
                info,
            },
        });
        Ok(())
    }

    /// `C::AwardStats`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_award_stats_packet(&mut self, p: play::AwardStats) -> Result<()> {
        // handleAwardStats: StatsCounter.setValue for each.
        let Some(r) = self.remote.as_ref() else {
            return Ok(());
        };
        let mut known = Vec::new();
        for (ty, value, count) in p.stats {
            let Some(ty) = r
                .builtin
                .get("minecraft:stat_type")
                .and_then(|t| t.name(ty))
            else {
                continue;
            };
            let values = match ty {
                "minecraft:mined" => "minecraft:block",
                "minecraft:killed" | "minecraft:killed_by" => "minecraft:entity_type",
                "minecraft:custom" => "minecraft:custom_stat",
                _ => "minecraft:item",
            };
            if let Some(key) = r.builtin.get(values).and_then(|v| v.name(value)) {
                known.push((ty.to_owned(), key.to_owned(), count));
            }
        }
        for (ty, key, count) in known {
            self.stats.values.entry(ty).or_default().insert(key, count);
        }
        Ok(())
    }

    /// `C::ContainerSetData`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_container_set_data_packet(
        &mut self,
        p: play::ContainerSetData,
    ) -> Result<()> {
        if self.open_menu_name(p.container) == "minecraft:lectern" {
            self.remote_lectern_page(i32::from(p.value));
            return Ok(());
        }
        if self.remote_station_open(p.container) {
            self.remote_station_data(usize::try_from(p.id).unwrap_or(usize::MAX), p.value);
            return Ok(());
        }
        if usize::try_from(p.id)
            .is_ok_and(|index| self.native_screen_data(p.container, index, i32::from(p.value)))
        {
            return Ok(());
        }
        let v = i32::from(p.value);
        match self
            .container
            .and_then(|key| self.world.block_entities.get_mut(&key))
        {
            Some(BlockEntity::Furnace(f)) => match p.id {
                0 => f.lit_remaining = v,
                1 => f.lit_total = v,
                2 => f.cooking_timer = v,
                3 => f.cooking_total = v,
                _ => {}
            },
            // CrafterBlockEntity.containerData.
            Some(BlockEntity::Crafter(c)) => match usize::try_from(p.id) {
                Ok(k @ 0..9) => c.disabled[k] = v == 1,
                Ok(9) => c.triggered = v,
                _ => {}
            },
            // BrewingStandBlockEntity.dataAccess.
            Some(BlockEntity::Brewing(b)) => match p.id {
                0 => b.brew_time = v,
                1 => b.fuel = v,
                2 => b.total_brew_time = v,
                3 => b.total_fuel = v,
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
