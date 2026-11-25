#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::packets_world::*;

impl Game<'_> {
    /// `C::BlockEvent`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game::client_side) fn apply_block_event_packet(
        &mut self,
        p: play::BlockEvent,
    ) -> Result<()> {
        // Level.blockEvent on the client level: the block's triggerEvent
        // runs on the client's copy of whatever is at pos (a piston's
        // moveBlocks builds the moving block entities it animates, a note
        // block plays); what it doesn't handle reaches the client copy of
        // the block entity (chest lids, shulker boxes).
        let pos = sunrise_core::position::BlockPos::new(p.pos[0], p.pos[1], p.pos[2]);
        let triggered = self.with_level(|b, level, ticks| {
            b.client_block_event(level, ticks, pos, i32::from(p.b0), i32::from(p.b1))
        });
        self.client.moving_pistons_stale = true;
        self.process_level_events()?;
        if !triggered {
            let state = self.world.block(p.pos[0], p.pos[1], p.pos[2]);
            let block = &self.res.facts().get(state).name;
            let named = self.remote.as_ref().and_then(|r| {
                r.builtin
                    .get("minecraft:block")?
                    .name(p.block)
                    .map(str::to_owned)
            });
            if named.as_deref() == Some(&**block) {
                self.block_entity_event(p.pos, i32::from(p.b0), i32::from(p.b1));
            }
        }
        Ok(())
    }
    /// `C::GameEvent`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game::client_side) fn apply_game_event_packet(&mut self, p: play::GameEvent) {
        match p.event {
            // NO_RESPAWN_BLOCK_AVAILABLE: ClientPacketListener.handleGameEvent
            // sends the player this system message.
            0 => self.client_system_message(sunrise_client::text::Text::translate(
                "block.minecraft.spawn.not_valid",
                &[],
            )),
            // LEVEL_CHUNKS_LOAD_START: the server has begun sending the level.
            13 => {
                if let Some(r) = self.remote.as_mut() {
                    r.level_load.loading_packets_received();
                }
            }
            // GUARDIAN_ELDER_EFFECT: the apparition, and the curse sound
            // unless the elder guardian is silent.
            10 => {
                self.particles.0.elder_guardian();
                if p.rounded_param() == 1
                    && let Some(at) = self.player.as_ref().map(|m| m.position)
                {
                    self.play_client_local_sound(
                        LevelSound::new(
                            "minecraft:entity.elder_guardian.curse",
                            [at.x, at.y, at.z],
                            1.0,
                            1.0,
                        )
                        .in_source(mod_api::SoundSource::Hostile),
                    );
                }
            }
            // WIN_GAME: the poem and credits, then PERFORM_RESPAWN.
            4 if self.client.screen != Screen::Win => {
                self.open_win_screen(true, super::super::win::Finish::RemoteRespawn);
            }
            // DEMO_EVENT: a parentless popup or a client-system hint.
            5 => self.apply_demo_event(p.param),
            // START_RAINING, STOP_RAINING, RAIN_LEVEL_CHANGE and THUNDER_LEVEL_CHANGE.
            event @ (1 | 2 | 7 | 8) => self.world.weather.game_event(event, p.param),
            3 => {
                self.set_local_mode(crate::game::game_mode::mode_from_id(p.rounded_param()));
                if self.spectator() {
                    self.reset_client_item_use();
                }
            }
            // IMMEDIATE_RESPAWN: LocalPlayer.setShowDeathScreen.
            11 => {
                if let Some(r) = self.remote.as_mut() {
                    r.show_death_screen = p.param == 0.0;
                }
            }
            // PLAY_ARROW_HIT_SOUND: ClientPacketListener.handleGameEvent
            // plays ARROW_HIT_PLAYER at player eye height.
            6 => {
                if let Some(at) = self.eye_position() {
                    self.play_client_local_sound(
                        LevelSound::new("minecraft:entity.arrow.hit_player", at, 0.18, 0.45)
                            .in_source(mod_api::SoundSource::Players),
                    );
                }
            }
            // PUFFER_FISH_STING: ClientPacketListener.handleGameEvent
            // plays PUFFER_FISH_STING at player position.
            9 => {
                if let Some(at) = self.player.as_ref().map(|m| m.position) {
                    self.play_client_local_sound(
                        LevelSound::new(
                            "minecraft:entity.puffer_fish.sting",
                            [at.x, at.y, at.z],
                            1.0,
                            1.0,
                        )
                        .in_source(mod_api::SoundSource::Neutral),
                    );
                }
            }
            // LIMITED_CRAFTING: LocalPlayer.setDoLimitedCrafting.
            12 => {
                if let Some(r) = self.remote.as_mut() {
                    r.do_limited_crafting = p.param == 1.0;
                }
            }
            _ => {}
        }
    }
}
