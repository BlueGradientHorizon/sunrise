use super::*;

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn finish_player_autosaves(&mut self, wait: bool) -> Result<bool> {
        let saved = self.side.player_autosaves.0.completed(wait);
        if !saved.is_empty() {
            let directory = sunrise_client::lan::playerdata::directory(&self.side.world_root);
            for player in saved {
                self.native_player_save_to_file(player.uuid, &player.name, &directory);
            }
        }
        self.side.player_autosaves.0.writes_finished()
    }
    /// `MinecraftServer.onTickRateChanged`: mods hear of it, then a rate that
    /// asks for a shorter wait than the one already running shortens it.
    pub(in crate::game) fn on_tick_rate_changed(&mut self) {
        self.native_server_tick_rate_changed();
        let interval = self.next_autosave_interval();
        if interval < self.side.ticks_until_autosave {
            self.side.ticks_until_autosave = interval;
        }
    }
    pub(in crate::game) fn save_game_metadata(&mut self) -> Result<()> {
        self.save_maps();
        self.save_random_sequences();
        self.save_fight();
        self.save_wandering_trader();
        self.save_portal_arrivals();
        self.save_stats();
        self.save_advancements();
        self.save_scoreboard();
        self.save_bossbars();
        self.save_schedule();
        self.save_warden_tracker();
        self.sync_level();
        // Level::write can synchronously replace the integrated owner file.
        let _ = self.finish_player_autosaves(true);
        let stored_owner = self.store_owner();
        write_level_for_role(
            &mut self.level,
            &self.side.world_root,
            self.options.execution_role,
            self.res.items(),
            self.side.crash_save.keep_owner_file,
        )?;
        // PlayerEvent.SaveToFile for the owner, once level.dat is written.
        if let Some((uuid, name)) = stored_owner {
            let directory = sunrise_client::lan::playerdata::directory(&self.side.world_root);
            self.native_player_save_to_file(uuid, &name, &directory);
        }
        // PlayerList.saveAll: every placed player saves with the server, to
        // their own file (`PlayerDataStorage.save`); the owner is also in the
        // file level.dat names, which is the one it loads from
        // (`playerdata::load_owner`).
        if self.options.execution_role == execution_role::ExecutionRole::DedicatedServer
            && let Some(host) = self.lan.as_mut()
        {
            // The vehicle tree goes into a player's file when they leave
            // (`take_parent_vehicle`); here it stays in the chunk saves.
            host.hub.guests.update_all(|g| {
                if g.placed && !self.side.crash_save.excludes(g.conn) {
                    // Read-only fixture input, correlated with the real save receipt.
                    // Keep this identical on the untouched/fixed selector builds.
                    if std::env::var_os("SUNRISE_DEV_ADVANCEMENT_SELECTOR_AUDIT").is_some() {
                        eprintln!(
                            "advancement selector model raw: {}",
                            serde_json::json!({"conn":g.conn,"name":g.name,
                                "uuid":sunrise_protocol::crypto::uuid_string(g.uuid),
                                "game_time":self.world.game_time,
                                "progress":g.advancements.to_json(&self.adv.tree, sunrise_world::DATA_VERSION)})
                        );
                    }
                    self.side.player_autosaves.0.enqueue(g, &self.side.world_root, self.res.items());
                }
            });
        }
        if self.options.execution_role == execution_role::ExecutionRole::DedicatedServer
            && let Some(host) = self.lan.as_ref()
        {
            self.save_connected_guest_statistics(host);
            for guest in host.hub.guests.iter() {
                if host
                    .owner
                    .as_ref()
                    .is_some_and(|owner| owner.uuid == guest.uuid)
                    || !host.play.contains_key(&guest.conn)
                {
                    continue;
                }
                self.save_guest_advancements(host, guest);
            }
        }
        self.save_world_state();
        Ok(())
    }
}
