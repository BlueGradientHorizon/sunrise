use super::*;

impl Game<'_, crate::game::client_side::ClientState> {
    #[inline(always)]
    pub(in crate::game) fn client_acting_player_uuid(&self) -> Option<u128> {
        self.remote.as_ref().map(|r| r.profile())
    }
    #[inline(always)]
    pub(in crate::game) fn client_resolve_enchant_particles(&self) {
        self.resolve_enchant_particles();
    }
    #[inline(always)]
    pub(in crate::game) fn client_tags_ready(&self) -> bool {
        false
    }
    #[inline(always)]
    pub(in crate::game) fn client_world_tables(&self) -> Option<&WorldTables> {
        None
    }
    pub(in crate::game::client_side) fn defer_world_entry(
        &mut self,
        entry: PendingWorldEntry,
    ) -> Result<bool> {
        self.poll_world_rest()?;
        if self.rest_loader.is_none() {
            return Ok(false);
        }
        self.perf.world_opening();
        if self.side.title.0.is_none() {
            let title = self
                .new_title(std::env::current_dir()?.join("saves"))
                .with_user(|| self.options.player_name());
            self.show_title(title);
        }
        self.side.pending_entry.0 = Some(entry);
        log::info!("Sunrise: world entry waiting for world resources");
        Ok(true)
    }
    /// `CreateWorldScreen.openCreateWorldScreen`: read the world generation
    /// data the create screen describes, on a worker, then open the screen
    /// (`title.preparing`).
    pub(in crate::game::client_side) fn poll_world_creation(&mut self) {
        let Some(title) = self.side.title.0.as_ref() else {
            return;
        };
        if !title.preparing() {
            return;
        }
        if self.side.creation_job.0.is_none() {
            let dir = self.options.resources.join("worldgen");
            let registry = self.pack.assets.registry;
            // The chosen packs' worldgen over the game's own:
            // `applyNewPackConfig` reloads with the new pack config.
            let overrides = title
                .creation_packs()
                .map(|(dir, packs)| {
                    world_loading::creation_worldgen_overrides(
                        dir.as_deref(),
                        &self.options.resources,
                        &packs,
                        self.res.biomes(),
                        &self.pack.additional_data,
                    )
                })
                .unwrap_or_default();
            let data_source = self.pack.data_changes.clone();
            let job = std::thread::Builder::new()
                .stack_size(sunrise_threads::GAME_THREAD_STACK_BYTES)
                .name("world-creation".into())
                .spawn(move || {
                    sunrise_client::vanillagen::shared_data_from(
                        &dir,
                        &overrides,
                        registry,
                        &data_source,
                    )
                    .map(|_| ())
                    .map_err(|e| e.to_string())
                });
            match job {
                Ok(job) => self.side.creation_job.0 = Some(job),
                Err(error) => {
                    log::warn!("Sunrise: world creation worker could not start: {error}");
                    if let Some(title) = self.side.title.0.as_mut() {
                        title.finish_preparing();
                        title.failed(format!(
                            "{}: {error}",
                            sunrise_client::lang::get("createWorld.preparing")
                        ));
                    }
                }
            }
            return;
        }
        if !self
            .side
            .creation_job
            .0
            .as_ref()
            .is_none_or(std::thread::JoinHandle::is_finished)
        {
            return;
        }
        let outcome = self
            .side
            .creation_job
            .0
            .take()
            .and_then(|job| job.join().ok());
        if let Some(title) = self.side.title.0.as_mut() {
            match outcome {
                Some(Ok(())) => title.finish_preparing(),
                // `applyNewPackConfig`'s failure: back out of the read with
                // the message, the Create screen's options intact.
                outcome => {
                    let e = outcome
                        .and_then(|r| r.err())
                        .unwrap_or_else(|| "the read never ran".to_owned());
                    log::warn!("Sunrise: preparing world creation failed: {e}");
                    // Clear the flag first: `failed` sets the error screen
                    // with Create as its back target, which
                    // `finish_preparing` would replace with a fresh world.
                    title.finish_preparing();
                    title.failed(format!(
                        "{}: {e}",
                        sunrise_client::lang::get_or(
                            "createWorld.preparing",
                            "Preparing for world creation..."
                        )
                    ));
                }
            }
        }
    }
    pub(in crate::game) fn resume_world_entry(&mut self) -> Result<()> {
        self.poll_integrated_start()?;
        self.poll_world_rest()?;
        self.poll_world_creation();
        if self.rest_loader.is_some() {
            return Ok(());
        }
        let on_world_screens = self.side.title.0.as_ref().is_some_and(|title| {
            !matches!(
                title.mode,
                sunrise_client::title::Mode::Main | sunrise_client::title::Mode::Multiplayer(_)
            )
        });
        world_loading::preload_world_data(
            on_world_screens,
            &self.options.resources,
            &self.pack.data_changes,
            self.pack.assets.registry,
        );
        if std::mem::take(&mut self.side.pending_pack_reload) {
            self.reload_packs();
        }
        match self.side.pending_entry.0.take() {
            Some(PendingWorldEntry::Local {
                folder,
                name,
                seed,
                creative,
                create,
            }) => {
                self.open_world(&folder, name.as_deref(), seed, creative, create.as_deref())?;
            }
            Some(PendingWorldEntry::Server {
                address,
                name,
                list_name,
                cookies,
                lan,
                realm,
            }) => {
                self.connect_server_with_options(
                    &address,
                    &name,
                    cookies,
                    lan,
                    realm,
                    list_name.as_deref(),
                )?;
            }
            Some(PendingWorldEntry::StudioReplay(path)) => {
                let path_str = path.to_string_lossy().to_string();
                if let Err(e) = self.open_studio_replay(&path_str) {
                    log::warn!("Sunrise Studio: could not open {path_str}: {e}");
                }
            }
            None => {}
        }
        Ok(())
    }
}
