#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::world_update::order_uploads;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::world_update::section_distance_sq;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::world_update::*;

impl Game<'_> {
    pub(in crate::game) fn clear_lod(&mut self) {
        // The samplers waiting on the server stop first: dropping the far
        // terrain joins them.
        if let Some(remote) = &self.remote {
            remote.session.cancel_far_terrain();
        }
        self.lod.0 = None;
        self.client.lod_source_attempted = false;
        self.client.lod_observe.clear();
        self.lod_blue_map.0 = None;
        self.client.lod_covered_at = None;
        if let Some(renderer) = &mut self.renderer.0 {
            renderer.clear_lods();
        }
    }
    pub(in crate::game) fn prepare_lod(&mut self) {
        let enabled = self.far_terrain_choice().0 > 0
            && self.renderer.0.is_some()
            && self.player.is_some()
            && self.title.0.is_none();
        if !enabled {
            if self.lod.0.is_some() || self.client.lod_source_attempted {
                self.clear_lod();
            }
            return;
        }
        // Another level: its own kept chunks. (Another world at the same
        // address, a proxy's next server, comes through a new login; a seed
        // changed at a respawn of the same level is a plugin hiding it.)
        if self
            .client
            .lod_level
            .as_ref()
            .is_some_and(|level| self.remote.as_ref().is_some_and(|r| r.level != *level))
        {
            self.clear_lod();
        }
        if self.client.lod_source_attempted {
            return;
        }
        self.client.lod_source_attempted = true;
        self.client.lod_level = None;
        let dim = self.dim;
        let source = match (&self.integrated, self.server_far_terrain(), &self.remote) {
            (Some(server), ..) => server
                .ask(|reply| super::owner_link::OwnerRequest::LodSource(dim, reply))
                .flatten(),
            (None, Some(offer), Some(remote)) => {
                let states = self.res.facts().states.len() as u32;
                let biomes = self.res.biomes().index.clone();
                let source: Arc<dyn sunrise_client::streaming::LodSource> =
                    remote.session.far_terrain(offer, biomes, states);
                Some(source)
            }
            // A friend's world has a new code at every share: nothing to keep.
            (None, None, Some(remote))
                if remote
                    .session
                    .address
                    .starts_with(sunrise_client::net::FRIEND) =>
            {
                None
            }
            // No offer from a server: draw what it sent, on this visit and before.
            (None, None, Some(remote)) => std::env::current_dir().ok().map(|home| {
                use sunrise_client::server_terrain as terrain;
                let address = &remote.session.address;
                let root = home.join("server-terrain");
                let dir = terrain::level_dir(&root, address, remote.hashed_seed, &remote.level);
                self.client.lod_level = Some(remote.level.clone());
                // Past them, the server's BlueMap, if the player named it.
                let maps = root.join(terrain::server_folder(address)).join("BlueMap");
                let settings = &self.settings.sunrise.blue_map;
                let blue_map = settings.server(address).map(|server| {
                    use sunrise_client::bluemap;
                    // The map's address only at debug: logs land in crash reports.
                    log::info!("Sunrise: far terrain also draws {}'s BlueMap", remote.level);
                    log::debug!("Sunrise: BlueMap at {}", server.url);
                    let map = Arc::new(bluemap::BlueMap::new(
                        &server.url,
                        &remote.level,
                        server.maps.clone(),
                        maps,
                        settings.radius_blocks(),
                        bluemap::Palette::from_game(self.pack.assets.registry, self.res.biomes()),
                    ));
                    map.start();
                    map
                });
                self.lod_blue_map.0 = blue_map.clone();
                terrain::ServerTerrain::new(dir, blue_map) as _
            }),
            _ => Some(self.streaming.lod_source()),
        };
        if let Some(source) = source {
            self.lod.0 = Some(sunrise_client::lod::Lod::new(source, self.worker_context()));
            self.client
                .lod_observe
                .extend(self.world.chunks.keys().copied());
        }
    }
}
