//! Receiving-provider credit follows ordered events and decoder placeholders.
use super::*;

impl Remote {
    pub(super) fn receive_credited_backlog(&mut self, events: &mut VecDeque<Received>) {
        // Chunk packets start decoding now; the frame that reaches one in
        // order inserts its column.
        for receipt in self.session.poll() {
            let (event, credit) = receipt.into_parts();
            events.push_back(match event {
                Event::Packet(p) if matches!(*p, C::LevelChunkWithLight(_)) => {
                    match self.decoder.submit(self.chunk_format.clone(), p) {
                        None => Received::Chunk(self.chunk_format.clone(), credit),
                        Some(p) => Received::Event(Event::Packet(p), credit),
                    }
                }
                event => Received::Event(event, credit),
            });
        }
    }
}

impl Game<'_> {
    /// Handle what the server sent, in order. `before_tick`: a tick is about to
    /// run, so what an earlier poll left for later is taken whole (no frame
    /// budget, each column waited for) and nothing newer is received.
    pub(super) fn poll_remote_events(&mut self, before_tick: bool) -> Result<()> {
        let Some(r) = self.remote.as_mut() else {
            return Ok(());
        };
        let mut events = std::mem::take(&mut r.backlog);
        if !before_tick {
            r.receive_credited_backlog(&mut events);
        }
        let overdue = r
            .backlog_since
            .is_some_and(|since| since.elapsed() >= BACKLOG_LIMIT);
        let mut chunks = Duration::ZERO;
        // A run of ForgetLevelChunk (thousands when the server's view moves far: a
        // teleport at 32 chunks) is forgotten at once, before whatever follows it.
        let mut forgotten = sunrise_client::fast_hash::FxHashSet::default();
        let mut forgotten_credits = Vec::new();
        while let Some(item) = events.pop_front() {
            if let Received::Event(Event::Packet(packet), _) = &item
                && let C::ForgetLevelChunk(p) = &**packet
            {
                self.observe_client_packet(packet);
                forgotten.insert((p.pos[0], p.pos[1]));
                forgotten_credits.push(item.into_event_credit());
                continue;
            }
            sunrise_client::spans::time("pr_forget", || {
                self.forget_remote_columns(&mut forgotten, true)
            });
            forgotten_credits.clear();
            let chunk = matches!(item, Received::Chunk(_, _));
            if chunk && !overdue && !before_tick && chunks >= CHUNK_PACKETS_PER_FRAME {
                events.push_front(item);
                break;
            }
            let started = chunk.then(Instant::now);
            let (event, _credit) = match item {
                Received::Event(event, credit) => (event, credit),
                Received::Chunk(format, credit) => {
                    // A column still decoding waits for the next frame, with
                    // what came after it, overdue or not.
                    let Some(next) = self.remote.as_ref().map(|r| {
                        if before_tick {
                            r.decoder.wait_next(TICK_DECODE_WAIT)
                        } else {
                            r.decoder.next()
                        }
                    }) else {
                        break;
                    };
                    let (packet, column) = match next {
                        Next::Ready(decoded) => decoded,
                        Next::Decoding => {
                            events.push_front(Received::Chunk(format, credit));
                            break;
                        }
                        Next::Lost => {
                            log::warn!("Sunrise: the chunk decoder stopped; a column is missing");
                            continue;
                        }
                    };
                    self.observe_client_packet(&packet);
                    if let C::LevelChunkWithLight(p) = *packet {
                        sunrise_client::spans::time("pr_chunk", || {
                            self.remote_chunk(&p, Some((&format, column)))
                        });
                    }
                    chunks += started.map_or(Duration::ZERO, |t| t.elapsed());
                    continue;
                }
            };
            match event {
                Event::Status(s) => {
                    if let Some(r) = self.remote.as_mut() {
                        r.status = s;
                    }
                }
                Event::Joined {
                    registries,
                    name,
                    uuid,
                } => {
                    if let Err(error) = self.set_registries(registries) {
                        let reason = Text::translate(
                            "disconnect.genericReason",
                            &[Text::literal(error.to_string())],
                        );
                        return self.leave_server(Some(reason));
                    }
                    self.connection_joined();
                    self.close_dialogs();
                    if let Some(r) = self.remote.as_mut() {
                        r.joined = true;
                        r.profile = uuid;
                        r.profile_name = name.clone();
                        r.status = sunrise_client::lang::get_or(
                            "multiplayer.downloadingTerrain",
                            "Loading terrain...",
                        );
                    }
                    self.begin_book_negotiation();
                }
                Event::Features(features) => {
                    if let Some(remote) = &mut self.remote {
                        remote.features = features.into();
                    }
                }
                Event::Brand(brand) => {
                    if let Some(remote) = &mut self.remote {
                        remote.brand = brand;
                    }
                }
                Event::QuietCommands => {
                    if let Some(remote) = &mut self.remote {
                        remote.quiet_commands = true;
                    }
                }
                Event::CustomBooks => {
                    if let Some(remote) = &mut self.remote {
                        remote.custom_books = true;
                    }
                }
                Event::FarTerrain(offer) => {
                    if let Some(remote) = &mut self.remote {
                        remote.far_terrain = Some(offer);
                    }
                }
                Event::Reconfiguring => {
                    self.reset_book_view();
                    // handleConfigurationStart: the held-back chat shows, then
                    // clearClientLevel opens ServerReconfigScreen over no level.
                    self.flush_chat_queue();
                    self.send_chat_acknowledgement();
                    self.close_dialogs();
                    // The server configuring us now (a proxy's next one)
                    // offers its own far terrain, or none, and announces its
                    // own features again.
                    if let Some(remote) = &mut self.remote {
                        remote.viewed.clear();
                        remote.far_terrain = None;
                        remote.quiet_commands = false;
                        remote.custom_books = false;
                    }
                    self.clear_lod();
                    if let Some(remote) = &self.remote {
                        remote.session.reset_far_terrain();
                    }
                    self.remove_local_player_body();
                    self.set_screen(Screen::Playing);
                    self.capture(false);
                    // clearClientLevel drops the client level now, so its
                    // unload comes before the configuration screen, not
                    // after the next login's load.
                    self.forget_all_remote_columns();
                    if let Some(old) = self.remote.as_mut().and_then(|r| r.heard_level.take()) {
                        self.native_client_level_unload(&old);
                    }
                    self.native_lifecycle.begin_close();
                    self.native_lifecycle_world_closed();
                    let ticks = self.ticks;
                    if let Some(r) = self.remote.as_mut() {
                        self.native_net.holds.borrow_mut().clear();
                        r.reconfiguring = Some(ticks);
                        r.chat_tree_received = false;
                    }
                }
                Event::Reconfigured(registries) => {
                    if let Err(error) = self.set_registries(registries) {
                        let reason = Text::translate(
                            "disconnect.genericReason",
                            &[Text::literal(error.to_string())],
                        );
                        return self.leave_server(Some(reason));
                    }
                    // handleConfigurationStart → clearClientLevel, and the next
                    // ClientPacketListener starts with no players or scoreboard,
                    // no waypoints (its own ClientWaypointManager: a proxy's next
                    // server never untracks the last one's), and its new
                    // ClientSuggestionProvider with no custom chat completions.
                    // clearClientLevel clears the narrator.
                    self.clear_narrator();
                    self.hud_disconnected();
                    self.score.board = Default::default();
                    self.clear_waypoints();
                    if let Some(r) = self.remote.as_mut() {
                        r.clear_attachments();
                        r.players.clear();
                        r.tab_players.clear();
                        r.listed.clear();
                        r.signatures = Default::default();
                        r.custom_completions.clear();
                    }
                    self.ui.registries.custom_completions.clear();
                }
                // handleTransfer: "Cannot transfer to server from
                // singleplayer" (no ServerData), so the world just closes.
                Event::Transfer { host, port } => {
                    if let Some(r) = self.remote.as_mut()
                        && r.session.address != "local"
                    {
                        r.transfer = Some(format!("{host}:{port}"));
                    }
                }
                Event::PackPush(p) => self.handle_pack_push(p),
                Event::PackPop(id) => self.handle_pack_pop(id),
                Event::ServerLinks(links) => {
                    if let Some(r) = self.remote.as_mut() {
                        r.server_links = crate::game::server_links::trusted(links);
                    }
                }
                Event::ShowDialog { dialog, registries } => {
                    self.show_dialog_in_phase(&dialog, Some(registries));
                }
                Event::ClearDialog => self.clear_dialog(),
                Event::CodeOfConduct(text) => self.handle_code_of_conduct(text),
                Event::DecodeError(e) => {
                    if let Some(r) = self.remote.as_mut() {
                        r.decode_errors += 1;
                        if r.decode_errors <= 20 {
                            log::warn!("Sunrise: skipped undecodable packet {e}");
                        }
                    }
                }
                Event::Disconnected {
                    reason,
                    key,
                    from_server,
                } => {
                    self.reset_book_view();
                    log::info!("Sunrise: disconnected: {reason}");
                    // ClientboundTransferPacket left a target: connect there
                    // instead of showing the disconnect, the stored cookies
                    // carried over (ClientCommonPacketListenerImpl.
                    // handleTransfer → ConnectScreen.startConnecting).
                    let transfer = self.remote.as_mut().and_then(|r| {
                        r.transfer.take().map(|address| {
                            (address, r.session.name.clone(), r.session.cookies.clone())
                        })
                    });
                    if let Some((address, name, cookies)) = transfer {
                        self.connection_left(sunrise_hooks::connection::Left::Transferred);
                        self.leave_server(None)?;
                        self.connect_server_with_cookies(&address, &name, Some(cookies))?;
                        return Ok(());
                    }
                    let by = if from_server {
                        sunrise_hooks::connection::By::Server
                    } else {
                        sunrise_hooks::connection::By::Connection
                    };
                    self.connection_ended(&reason, key.as_deref(), by);
                    self.leave_server(Some(Text::literal(reason)))?;
                    return Ok(());
                }
                Event::RawPayload(incoming) => {
                    if let Some(remote) = self.remote.as_ref() {
                        self.run_payload(remote.session.payload_endpoint(), *incoming);
                    }
                }
                Event::Packet(packet) => match *packet {
                    C::CustomPayload(p) if p.channel == sunrise_client::perf_net::CHANNEL => {
                        self.remote_perf_reply(&p.data)
                    }
                    packet => self.handle_packet(packet)?,
                },
            }
            chunks += started.map_or(Duration::ZERO, |t| t.elapsed());
            if self.remote.is_none() {
                break;
            }
        }
        if self.remote.is_some() {
            sunrise_client::spans::time("pr_forget", || {
                self.forget_remote_columns(&mut forgotten, true)
            });
        }
        if let Some(r) = self.remote.as_mut() {
            r.backlog_since = if events.is_empty() {
                None
            } else {
                Some(r.backlog_since.unwrap_or_else(Instant::now))
            };
            r.backlog = events;
        }
        self.send_suggestion_request();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::native_entity_refs::tests::with_client_game;
    use sunrise_client::{multiplayer::Screen as MultiplayerScreen, title::Mode};
    use sunrise_protocol::connection::Pipe;

    fn push_events(game: &mut Game<'_>, events: impl IntoIterator<Item = Event>) {
        let (tx, rx) = sunrise_client::net::ingress::channel(1024 * 1024, |_| 0).unwrap();
        let backlog = &mut game.remote.as_mut().unwrap().backlog;
        for event in events {
            tx.send(event).unwrap();
            let (event, credit) = rx.try_recv_owned().unwrap().into_parts();
            backlog.push_back(Received::Event(event, credit));
        }
    }

    fn login_packet() -> play::Clientbound {
        C::Login(play::Login {
            player_id: 7,
            hardcore: false,
            levels: vec!["minecraft:overworld".into()],
            max_players: 1,
            chunk_radius: 2,
            simulation_distance: 2,
            reduced_debug_info: false,
            show_death_screen: true,
            limited_crafting: false,
            spawn: play::SpawnInfo {
                dimension_type: 0,
                dimension: "minecraft:overworld".into(),
                seed: 0,
                game_type: 1,
                previous_game_type: None,
                debug: false,
                flat: true,
                last_death: None,
                portal_cooldown: 0,
                sea_level: 63,
            },
            online_mode: false,
            enforces_secure_chat: false,
        })
    }

    fn registries_with_biome(base: &Dynamic, biome_id: &str) -> Arc<Dynamic> {
        let mut registries = base.clone();
        registries.registries.insert(
            "minecraft:dimension_type".into(),
            vec![("minecraft:overworld".into(), None)],
        );
        registries.registries.insert(
            BIOME_REGISTRY.into(),
            vec![
                ("minecraft:plains".into(), None),
                (
                    biome_id.into(),
                    Some(Tag::Compound(sunrise_core::nbt::Compound::new())),
                ),
            ],
        );
        Arc::new(registries)
    }

    fn assert_disconnected_with_generic_reason(game: &Game<'_>) {
        assert!(
            game.remote.is_none(),
            "client must disconnect on invalid biome registry"
        );
        assert!(
            !game.connection_open,
            "connection must close on invalid biome registry"
        );
        let mode = &game.title.as_ref().expect("the disconnect screen").mode;
        assert!(matches!(
            mode,
            Mode::Multiplayer(MultiplayerScreen::Disconnected { reason, .. })
                if reason.translation_key() == Some("disconnect.genericReason")
        ));
    }

    #[test]
    #[ignore = "daily"]
    fn invalid_biome_registry_disconnects_before_join_or_reconfigured_play() {
        with_client_game(|game| {
            // Joined with an invalid biome identifier (e.g. `..:bad` with supplied data) must
            // disconnect before marking the join complete or processing queued play packets.
            game.title = None;
            let (pipe, _server) = Pipe::pair();
            game.connect_local(pipe, "InvalidBiomeJoin").unwrap();
            game.connection_starts("example.invalid:25565", "InvalidBiomeJoin", false);
            let invalid =
                registries_with_biome(&game.remote.as_ref().unwrap().registries, "..:bad");
            push_events(
                game,
                [
                    Event::Joined {
                        registries: invalid,
                        uuid: 1,
                        name: "Alex".into(),
                    },
                    Event::Packet(Box::new(login_packet())),
                ],
            );
            game.poll_remote_events(true).unwrap();
            assert_disconnected_with_generic_reason(game);

            // Reconfigured with an invalid biome identifier after a valid play session must
            // disconnect rather than allowing play to resume with the previous registries.
            game.world_biomes = None;
            game.world_biomes_base = None;
            game.title = None;
            let (pipe, _server) = Pipe::pair();
            game.connect_local(pipe, "InvalidBiomeReconfigure").unwrap();
            game.connection_starts("example.invalid:25565", "InvalidBiomeReconfigure", false);
            let valid =
                registries_with_biome(&game.remote.as_ref().unwrap().registries, "test:marsh");
            let invalid =
                registries_with_biome(&game.remote.as_ref().unwrap().registries, "..:bad");
            push_events(
                game,
                [
                    Event::Joined {
                        registries: valid,
                        uuid: 2,
                        name: "Steve".into(),
                    },
                    Event::Packet(Box::new(login_packet())),
                ],
            );
            game.poll_remote_events(true).unwrap();
            assert!(
                game.remote
                    .as_ref()
                    .is_some_and(|r| r.joined && r.logged_in && r.reconfiguring.is_none())
            );
            assert!(world_biomes!(game).contains("test:marsh"));

            push_events(
                game,
                [
                    Event::Reconfiguring,
                    Event::Reconfigured(invalid),
                    Event::Packet(Box::new(login_packet())),
                ],
            );
            game.poll_remote_events(true).unwrap();
            assert_disconnected_with_generic_reason(game);
        });
    }
}
