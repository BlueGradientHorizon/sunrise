#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::packets_player::*;

impl Game<'_> {
    /// `C::ClearTitles`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game::client_side) fn apply_clear_titles_packet(&mut self, p: play::Bool1) {
        self.client.titles.clear();
        if p.value {
            self.client.titles.reset_times();
        }
    }
    /// `C::ContainerSetSlot`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game::client_side) fn apply_container_set_slot_packet(
        &mut self,
        p: play::ContainerSetSlot,
    ) {
        self.remote_menu_state(p.container, p.state);
        let stack = self.to_stack(&p.item);
        self.tutorial_get_item(stack.as_ref());
        // ClientPacketListener.handleContainerSetSlot: a hotbar slot that
        // gains items squeezes for 5 ticks.
        if p.container == 0
            && (36..=45).contains(&p.slot)
            && let Some(new) = &stack
        {
            let (pop, old) = match p.slot {
                45 => (9, self.offhand.as_ref()),
                s => (s as usize - 36, self.inventory[s as usize - 36].as_ref()),
            };
            if old.is_none_or(|old| old.count < new.count) {
                self.client.hotbar_pop[pop] = 5;
            }
        }
        let before = self.creative_stacks();
        self.set_menu_slot(p.container, i32::from(p.slot), stack);
        self.remote_synced_since(&before);
    }
    /// `C::Login`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game::client_side) fn apply_login_packet(&mut self, p: play::Login) {
        // A client's parts, from the tags it plays with.
        if let Err(e) = self.resolve_parts() {
            log::warn!("Sunrise: {e}");
        }
        // Configuration's tags, over the block ids the parts just bound.
        self.install_remote_tags();
        self.refresh_render_visibility();
        self.client.test_highlights.clear();
        // handleLogin: `debugSubscriber.clear()`.
        self.client.debug_charts.clear_subscriptions();
        self.client.debug_density.borrow_mut().clear();
        self.forget_synced_recipes();
        self.level.hardcore = p.hardcore;
        // handleLogin: startWaitingForNewLevel with Reason.OTHER, which
        // replaces ServerReconfigScreen.
        if let Some(r) = self.remote.as_mut() {
            r.reconfiguring = None;
            r.clear_attachments();
            r.viewed.clear();
            // A new ClientPacketListener owns a fresh clock manager.
            r.clocks.clear();
            r.clock_game_time = 0;
            self.world.game_time = 0;
        }
        // handleLogin makes a new MultiPlayerGameMode (carried index 0) and a
        // new LocalPlayer (slot 0, no effects, riding nothing: the server
        // pairs a vehicle again with ClientboundSetPassengersPacket).
        self.selected = 0;
        self.potions.effects.clear();
        // handleLogin creates a fresh LocalPlayer only when none exists yet.
        if self.remote.as_ref().is_none_or(|r| !r.logged_in) {
            self.client.post_effects.clear();
        }
        self.riding = None;
        if let Some(player) = self.player.as_mut() {
            player.entity_id = p.player_id;
        }
        self.native_net
            .holds
            .borrow_mut()
            .clear_cauldron_continuations();
        if let Some(r) = self.remote.as_mut() {
            r.carried.set(Some(0));
            r.pending_cauldrons.get_mut().clear();
            // handleLogin makes a new ClientLevel: its predictions count from 0.
            r.sequence = 0;
            r.own_invisible = false;
            r.own_on_fire = false;
            r.glowing.remove(&p.player_id);
            r.online_mode = p.online_mode;
            r.levels.clone_from(&p.levels);
            r.enforces_secure_chat = p.enforces_secure_chat;
            r.entity_id = p.player_id;
            r.local_entity_silent = false;
            // handleLogin: the new LocalPlayer is the camera.
            r.camera = None;
            r.detached_camera = None;
            r.logged_in = true;
            r.view_distance = p.chunk_radius;
            r.simulation_distance = p.simulation_distance;
            r.show_death_screen = p.show_death_screen;
            r.reduced_debug_info = p.reduced_debug_info;
            r.do_limited_crafting = p.limited_crafting;
        }
        self.clear_spectated_entity_post_effect();
        self.bind_local_player_residency();
        self.refresh_debug_renderers();
        // LocalPlayer.setReducedDebugInfo always rebuilds, also on a second login.
        self.debug_entries.0.invalidate();
        self.streaming.radius = crate::game::entity_culling::streaming_radius_for(
            self.settings.render_distance,
            Some(p.chunk_radius),
        );

        // handleLogin: startWaitingForNewLevel(.., Reason.OTHER).
        self.enter_remote_dimension(
            &p.spawn,
            RemoteDimensionEntry::Login,
            sunrise_client::level_loading::Reason::Other,
        );
        // No ClientInformation here: the configuration phase sent it,
        // and ClientPacketListener.broadcastClientInformation sends it
        // again only when an option changes it.
        self.remote_chat_login();
        if let Some(remote) = &self.remote {
            self.native_client_player_logging_in(p.player_id, remote.profile);
        }
    }
    /// `C::PlayerCombatKill`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game::client_side) fn apply_player_combat_kill_packet(
        &mut self,
        p: play::PlayerCombatKill,
    ) -> Result<()> {
        // The component resolves in the client's current language, as
        // the chat line does, not in the language the session joined
        // with (`Remote.lang` is the vanilla server-language table).
        let message = remote_text(&p.message);
        let me = self.remote.as_ref().is_some_and(|r| r.entity_id == p.id);
        if let Some(r) = self.remote.as_mut() {
            r.death_message = Some(message);
        }
        if me {
            self.reset_client_item_use();
        }
        // ClientPacketListener.handlePlayerCombatKill: the death
        // screen, whatever health was last sent, or `respawn()` at
        // once when it is hidden.
        if me && self.client.screen != Screen::Dead && self.player.is_some() {
            if self.remote.as_ref().is_none_or(|r| r.show_death_screen) {
                self.close_dialogs();
                self.set_screen(Screen::Dead);
                self.ui.death_time = 0;
                self.ui.death_confirm = None;
                self.capture(false);
            } else {
                self.respawn_player()?;
            }
        }
        Ok(())
    }
    /// `C::Respawn`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game::client_side) fn apply_respawn_packet(&mut self, p: play::Respawn) {
        self.reset_client_item_use();
        // The new LocalPlayer copies reduced debug info through its setter.
        self.debug_entries.0.invalidate();
        let old_player = self.remote.as_ref().map(|r| (r.entity_id, r.profile));
        if let Some((_, uuid)) = old_player {
            self.forget_entity_initialization(Holder::Player(uuid));
        }
        if let Some(r) = self.remote.as_ref() {
            self.audio.0.remove_sound_entity(r.entity_id as u32);
        }
        // A new player, without effects (the server sends them
        // again) and with no item in use.
        self.potions.effects.clear();
        self.client.post_effects.clear();
        // Every synced attachment but the player's is sent again; the viewed
        // levels go or stay with the level entered (`enter_respawn_level`).
        if let Some(r) = self.remote.as_mut() {
            r.attachments
                .retain(|holder, _| matches!(holder, Holder::Player(_)));
        }
        self.using = None;
        // After a death (no KEEP_ENTITY_DATA) the new LocalPlayer
        // rides nothing: the server pairs a vehicle again with its
        // passengers. A dimension change keeps the vehicle it travels in.
        // handleRespawn: KEEP_ENTITY_DATA gives the new player the old
        // one's motion, `resetPos` none.
        let kept_velocity = if p.keep & 2 != 0 {
            self.player.as_ref().map(|pl| pl.velocity)
        } else {
            None
        };
        if let Some(r) = self.remote.as_mut() {
            r.respawn_velocity = kept_velocity;
        }
        if p.keep & 2 == 0 {
            self.reset_remote_player_riding();
            // KEEP_ENTITY_DATA copies the old player's synched data
            // (its shared flags) to the new one; without it they reset.
            if let Some(r) = self.remote.as_mut() {
                r.own_invisible = false;
                r.own_on_fire = false;
                r.glowing.remove(&r.entity_id);
                r.local_entity_silent = false;
            }
        }
        // handleRespawn compares the levels' keys: a mod's level is
        // its own dimension even where `Dim` has no name for it.
        let dimension_changed = self
            .remote
            .as_ref()
            .is_some_and(|r| r.level != p.spawn.dimension);
        let died = self.client.screen == Screen::Dead
            || self
                .player
                .as_ref()
                .is_some_and(|m| m.health.health() <= 0.0);
        let reason = sunrise_client::level_loading::reason_for_respawn(
            died,
            &p.spawn.dimension,
            self.dim.id(),
        );
        if dimension_changed {
            sunrise_client::spans::startup::arm("dimension.change");
        }
        // handleRespawn makes a new ClientLevel for another dimension
        // key (a proxy's server switch too): its predictions count from 0.
        self.native_net
            .holds
            .borrow_mut()
            .clear_cauldron_continuations();
        if let Some(r) = self.remote.as_mut() {
            r.pending_cauldrons.get_mut().clear();
            if r.level != p.spawn.dimension {
                r.sequence = 0;
            }
        }
        self.portals.respawn_attributes.respawn(p.keep & 1 != 0);
        let entry = if died {
            RemoteDimensionEntry::DeathRespawn
        } else {
            RemoteDimensionEntry::Respawn
        };
        self.enter_respawn_level(&p, entry, reason);
        // handleRespawn adds the new LocalPlayer after what the kept
        // level holds; a new level holds nothing yet.
        let before: std::collections::HashSet<u32> = self
            .mobs
            .iter()
            .map(|m| m.id)
            .chain(self.boats.iter().map(|b| b.id))
            .collect();
        if let Some(r) = self.remote.as_mut() {
            r.ticked_before_player = before;
        }
        if dimension_changed {
            // LevelExtractor.setLevel: a new level drops the highlights.
            self.client.test_highlights.clear();
            let (music, _) = self.situational_music();
            self.client
                .music
                .stop_playing(music.as_ref(), &mut self.audio.0);
        }
        self.set_screen(Screen::Playing);
        if let Some(r) = self.remote.as_mut() {
            r.death_message = None;
            r.health = (20.0, 20, 5.0);
            // handleRespawn: the new LocalPlayer is the camera.
            r.camera = None;
            r.detached_camera = None;
        }
        self.clear_spectated_entity_post_effect();
        self.capture(true);
        if let Some((old_id, uuid)) = old_player {
            self.native_client_player_clone(
                old_id,
                self.own_entity_id(),
                uuid,
                died,
                dimension_changed,
            );
        }
    }
    /// `C::SetHealth`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game::client_side) fn apply_set_health_packet(&mut self, p: play::SetHealth) {
        // ClientPacketListener.handleSetHealth: LocalPlayer.hurtTo.
        let flash = self.remote.as_mut().is_some_and(|r| {
            r.health = (p.health, p.food, p.saturation);
            std::mem::replace(&mut r.flash_on_set_health, true)
        });
        if flash && let Some(pl) = self.player.as_mut() {
            pl.health.flash_health_change(p.health);
        }
        self.apply_remote_health();
        if p.health <= 0.0 {
            self.reset_client_item_use();
        }
        // The death screen opens (or the respawn is asked for) with the
        // death packet when the rule hides it.
        let shown = self.remote.as_ref().is_none_or(|r| r.show_death_screen);
        if p.health <= 0.0 && shown && self.client.screen != Screen::Dead && self.player.is_some() {
            self.close_dialogs();
            self.set_screen(Screen::Dead);
            self.ui.death_time = 0;
            self.ui.death_confirm = None;
            self.capture(false);
        }
    }
    /// `C::SystemChat`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game::client_side) fn apply_system_chat_packet(
        &mut self,
        p: play::SystemChat,
    ) -> Result<()> {
        // The component as sent, translated in the client's own
        // language; mods may change or cancel it. ChatListener.handleOverlay
        // for the action bar.
        let mut text = remote_text(&p.content);
        if !self.chat_received(true, p.overlay, 0, None, None, &mut text) {
            return Ok(());
        }
        if p.overlay {
            self.say_system_queued(&text);
            self.client.titles.set_overlay(text, false);
        } else if self.add_chat_line(text.clone(), sunrise_client::chat::ChatSource::Server) {
            // handleSystemMessage: said once the chat takes it.
            self.say_system_chat_queued(&text);
        }
        Ok(())
    }
    /// `C::TabList`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game::client_side) fn apply_tab_list_packet(&mut self, p: play::TabList) {
        // handleTabListCustomisation: empty text clears it.
        let text = |t: &Tag| {
            let json = sunrise_client::scoreboard::tag_to_json(t);
            let text = sunrise_client::scoreboard::comp_text(&json);
            text.spans
                .iter()
                .any(|span| !span.text.is_empty())
                .then_some(text)
        };
        self.tab_list.0.header = text(&p.header);
        self.tab_list.0.footer = text(&p.footer);
    }
}
