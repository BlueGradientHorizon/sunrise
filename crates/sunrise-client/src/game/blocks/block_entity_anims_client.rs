use super::Game;
use crate::game::server_side::block_entity_anims::AnimTick;
use crate::game::server_side::block_entity_anims::PLAYER;
use sunrise_client::{
    block_entity_anim::{BellEntity, ConduitTarget, Ticker},
    containers::BlockEntity,
    particles::{ParticleOptions, ParticleType},
    skins,
};
use sunrise_core::geometry::{Aabb, Vec3};

impl Game<'_, crate::game::client_side::ClientState> {
    /// `SkullBlockRenderer.getPlayerSkin`: the skin sprite of the profile of
    /// the player head at `pos`. The skin download is started here; while it
    /// is in flight (or when it cannot be got) the profile's default skin
    /// stands in. A head with no profile answers None: `SKIN_BY_TYPE`.
    pub(in crate::game::client_side) fn skull_skin_at(&self, pos: [i32; 3]) -> Option<String> {
        let Some(BlockEntity::Skull(skull)) =
            self.world.block_entities.get(&(pos[0], pos[1], pos[2]))
        else {
            return None;
        };
        let skin = skull.skin.as_ref()?;
        // PlayerSkin.Patch: a `texture` override replaces the profile's skin.
        if let Some(texture) = &skin.texture
            && let Some(sprite) = skins::patch_skin(
                texture,
                &mut self.side.skin_downloads.0.borrow_mut(),
                |location| self.side.gui.atlas.sprites.contains_key(location),
            )
        {
            return Some(sprite);
        }
        // The texture a packed or resolved `textures` property fetches: the
        // download of a static profile, or the one the services answer for a
        // `Dynamic` (name- or id-only) profile.
        let download = skin.download.clone().or_else(|| {
            let key = skin.dynamic.as_ref()?;
            let skins::Resolve::Done(Some(profile)) =
                self.side.skin_profiles.borrow_mut().state(key)
            else {
                return None;
            };
            let url = skins::unpack_insecure(profile.textures()?).skin?;
            let location = skins::skin_location(&skins::texture_hash(&url)?);
            Some((url, location))
        });
        if let Some((url, location)) = download {
            let mut downloads = self.side.skin_downloads.0.borrow_mut();
            if self.side.gui.atlas.sprites.contains_key(&location) {
                let _ = downloads.state(skins::Kind::Skin, &url);
                downloads.drawn(&location);
                return Some(location);
            }
            // Its slot was evicted: offer the image again.
            if downloads.state(skins::Kind::Skin, &url) == skins::Download::Ready {
                downloads.again(&location);
            }
        }
        Some(skin.fallback.to_owned())
    }
    /// `BellBlockEntity.updateEntities` on the client: entities within 48 blocks.
    pub(in crate::game) fn client_bell_gather(&self, pos: [i32; 3]) -> Vec<u64> {
        let area = Aabb::new(
            Vec3::new(
                f64::from(pos[0]) - 48.0,
                f64::from(pos[1]) - 48.0,
                f64::from(pos[2]) - 48.0,
            ),
            Vec3::new(
                f64::from(pos[0]) + 49.0,
                f64::from(pos[1]) + 49.0,
                f64::from(pos[2]) + 49.0,
            ),
        );
        let mut out = Vec::new();
        if self
            .client_player_view()
            .is_some_and(|p| p.bounds.intersects(area))
        {
            out.push(PLAYER);
        }
        for m in &self.mobs {
            if !m.removed && m.bounds().intersects(area) {
                out.push(u64::from(m.id));
            }
        }
        out
    }
    /// `BellBlockEntity.clientTick`: resonance particles on the client copy.
    pub(in crate::game) fn client_tick_bell(
        &mut self,
        pos: [i32; 3],
        tags: &sunrise_client::items::EntityTypeTags,
    ) {
        let player = self.client_player_view().map(|p| BellEntity {
            position: [p.position.x, p.position.y, p.position.z],
            alive: p.alive,
            raider: false,
        });
        let mobs = &self.mobs;
        let entity = |h: u64| {
            if h == PLAYER {
                return player;
            }
            let m = mobs.iter().find(|m| u64::from(m.id) == h)?;
            Some(BellEntity {
                position: [m.position.x, m.position.y, m.position.z],
                alive: m.alive() && !m.removed,
                raider: tags.has_bit(
                    m.kind.flags.entity_tag_slot,
                    sunrise_client::items::EntityTypeTags::RAIDERS,
                ),
            })
        };
        let tick = self
            .side
            .be_anims
            .bells
            .entry(pos)
            .or_default()
            .tick(pos, entity);
        if let Some((particles, _)) = tick.ended {
            for (p, color) in particles {
                // ColorParticleOption.create(ENTITY_EFFECT, color): ARGB.
                let c = color as u32;
                let channel = |shift: u32| ((c >> shift) & 255) as f32 / 255.0;
                let options = ParticleOptions {
                    color: [channel(16), channel(8), channel(0), channel(24)],
                    ..ParticleOptions::simple(ParticleType::EntityEffect)
                };
                self.side.particles.0.add(&options, p, [0.0; 3]);
            }
        }
    }
    /// Client conduit target resolution from `BlockEntity::Conduit(uuid)` + remote UUID table.
    fn client_conduit_target(&self, pos: [i32; 3]) -> Option<ConduitTarget> {
        let Some(BlockEntity::Conduit(Some(uuid))) =
            self.world.block_entities.get(&(pos[0], pos[1], pos[2]))
        else {
            return None;
        };
        let u = sunrise_client::lan::playerdata::ints_uuid(*uuid);
        let id = self
            .remote
            .as_ref()
            .and_then(|r| r.level_uuids.get(&u).copied())
            .map(|id| id as u32)
            .or_else(|| {
                self.persist
                    .uuids
                    .iter()
                    .find(|&(_, v)| v == uuid)
                    .map(|(&id, _)| id)
            })?;
        let m = self.mobs.iter().find(|m| m.id == id && !m.removed)?;
        Some(ConduitTarget {
            position: [m.position.x, m.position.y, m.position.z],
            eye_y: m.eye_y(),
            width: m.width(),
            height: m.height(),
        })
    }
    /// The client ticker of the animated block entity at `pos` (`ClientLevel.tickBlockEntities`),
    /// in the ticker list's order with every other client ticker.
    pub(in crate::game) fn tick_block_entity_anim(
        &mut self,
        pos: [i32; 3],
        ticker: Ticker,
        anim: &mut AnimTick,
    ) {
        self.claim_anim(pos);
        {
            match ticker {
                Ticker::Lid => self.tick_chest_lid(pos),
                Ticker::Book => {
                    // The player where earlier tickers (a lid's push) left it.
                    let player = match &self.player {
                        Some(p) if !self.spectator() => {
                            Some([p.position.x, p.position.y, p.position.z])
                        }
                        _ => None,
                    };
                    let anims = &mut self.side.be_anims;
                    anims.books.entry(pos).or_default().tick(
                        pos,
                        player.as_slice(),
                        &mut anims.book_random,
                    );
                }
                Ticker::Shulker { facing } => {
                    // `ShulkerBoxBlockEntity.tick` runs on both sides: the
                    // client moves its own player, including with a menu open.
                    // The server ticks its boxes in `tick_block_entities`.
                    if !self.is_logical_server() {
                        let tick = self
                            .side
                            .be_anims
                            .shulkers
                            .entry(pos)
                            .or_default()
                            .tick(pos, facing);
                        // The lid pushes at its turn, so later tickers read the pushed player.
                        if let Some(push) = tick.push {
                            if let Some(shulker) = self.side.be_anims.shulkers.get(&pos) {
                                self.world.shulkers.insert(pos, shulker.clone());
                            }
                            self.push_shulker_local(vec![(pos, facing, push)]);
                        }
                    }
                }
                Ticker::Skull { powered } => {
                    self.side
                        .be_anims
                        .skulls
                        .entry(pos)
                        .or_default()
                        .tick(powered);
                }
                Ticker::Bell => {
                    // Server bells tick independently; clients animate their copy.
                    if !self.is_logical_server() {
                        let tags = self.items().entity_type_tags.clone();
                        self.client_tick_bell(pos, &tags);
                    }
                }
                Ticker::Conduit => {
                    let frames = anim.frames.get_or_insert_with(|| {
                        self.res
                            .items()
                            .block_tags
                            .get("minecraft:conduit_effect_block")
                            .cloned()
                            .unwrap_or_default()
                    });
                    let target = self.client_conduit_target(pos);
                    let world = &self.world;
                    let facts = self.res.facts();
                    let registry = self.pack.assets.registry;
                    let is_water = |p: [i32; 3]| {
                        facts.get(world.block(p[0], p[1], p[2])).fluid
                            == sunrise_client::blocks::Fluid::Water
                    };
                    let is_frame = |p: [i32; 3]| {
                        registry
                            .state(world.block(p[0], p[1], p[2]))
                            .is_some_and(|s| frames.contains(&s.name))
                    };
                    let time = world.game_time;
                    let anims = &mut self.side.be_anims;
                    let particles = anims.conduits.entry(pos).or_default().client_tick(
                        pos,
                        time,
                        is_water,
                        is_frame,
                        target,
                        self.side.particles.0.level_random(),
                    );
                    let nautilus = ParticleOptions::simple(ParticleType::Nautilus);
                    for (p, v) in particles {
                        self.side.particles.0.add(&nautilus, p, v);
                    }
                }
                Ticker::Pot => {}
                Ticker::Gateway => {
                    // The server's portalTick (`tick_end_gateway`) already ran:
                    // its block event reaches this copy before it ticks.
                    let initial = self.gateway_loaded(pos);
                    self.side
                        .be_anims
                        .gateways
                        .entry(pos)
                        .or_insert(initial)
                        .tick();
                }
            }
        }
    }
}

#[cfg(test)]
impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn test_conduit_target(&self, pos: [i32; 3]) -> Option<ConduitTarget> {
        self.client_conduit_target(pos)
    }
}

#[cfg(test)]
pub(in crate::game) mod bell_ownership_tests {
    use sunrise_client::block_entity_anim::{Bell, BlockEntityAnims};

    #[test]
    fn server_bells_survive_visibility_changes_but_remote_bells_do_not() {
        let pos = [8, 200, 8];
        let mut anims = BlockEntityAnims::default();
        let mut bell = Bell::default();
        bell.trigger_event(1, 2, 100, || vec![7]);
        anims.bells.insert(pos, bell);
        anims.pots.insert(pos, Default::default());
        anims.retain_present_preserving_bells(&[]);
        assert_eq!(anims.bells[&pos].nearby.as_deref(), Some([7].as_slice()));
        assert!(anims.bells[&pos].shaking);
        assert!(anims.pots.is_empty());
        anims.retain_present(&[]);
        assert!(anims.bells.is_empty());
    }
}

#[cfg(test)]
pub(in crate::game) mod conduit_target_tests {
    use crate::game::server_side::block_entity_anims::conduit_target_tests::{
        CONDUIT, attack_conduit_for_test, game, summon,
    };
    use crate::game::{Audio, Options, Progress, Settings, execution_role, load_resources};
    use std::sync::Arc;
    use sunrise_client::{chunk::Chunk, containers::BlockEntity};
    use sunrise_core::nbt::Tag;

    fn game_with_client(
        resources: &mut Option<Arc<crate::game::Resources>>,
        role: execution_role::ExecutionRole,
    ) -> crate::game::ClientGame<'_> {
        let root = std::path::PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES")
                .expect("set SUNRISE_TEST_RESOURCES to prepared resources"),
        );
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            role,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = role;
        let res = resources.insert(loaded.resources);
        let mut game = crate::game::ClientGame::new(
            res,
            loaded.client_res.unwrap(),
            loaded.pack,
            loaded.client_pack.unwrap(),
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.clone(),
            )),
            None,
        )
        .unwrap();
        game.build_registries();

        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        game
    }

    /// A window of one: a client game with no conduit state of its own, and a
    /// conduit block whose block entity arrives from the server.
    fn client(resources: &mut Option<Arc<crate::game::Resources>>) -> crate::game::ClientGame<'_> {
        let mut game = game_with_client(resources, execution_role::ExecutionRole::Client);
        let (pipe, _server) = sunrise_protocol::connection::Pipe::pair();
        game.connect_local(pipe, "Window").unwrap();
        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        let state = game
            .pack
            .assets
            .registry
            .default_state("minecraft:conduit")
            .expect("the conduit's block state");
        game.world
            .set(CONDUIT[0], CONDUIT[1], CONDUIT[2], state)
            .unwrap();
        // The block entity a chunk's block entities build (`from_name`).
        game.world.block_entities.insert(
            (CONDUIT[0], CONDUIT[1], CONDUIT[2]),
            BlockEntity::Conduit(None),
        );
        game
    }

    /// `updateAndAttackTarget` puts the target in the block entity's `Target`
    /// and sends it (`sendBlockUpdated`); the window loads that tag, finds the
    /// mob and `animationTick` draws one nautilus particle at its eye. A tag
    /// without a `Target` takes the mob away again.
    #[test]
    #[ignore = "daily"]
    fn a_conduits_target_reaches_the_client_as_a_particle() {
        let mut server_resources = None;
        let mut server = game(&mut server_resources);
        let drowned = summon(&mut server, "drowned");
        server.world.block_log = Some(Vec::new());
        attack_conduit_for_test(&mut server, CONDUIT, true);
        let uuid = server.persist.uuids[&drowned];
        let key = (CONDUIT[0], CONDUIT[1], CONDUIT[2]);
        // What `sendBlockUpdated` puts on the wire.
        let tag = crate::lan::chunks::entity_tag(
            &server.world.block_entities[&key],
            false,
            &Default::default(),
        )
        .expect("a conduit has an update tag");
        assert_eq!(tag.get("Target"), Some(&Tag::IntArray(uuid.to_vec())));
        assert!(
            server.world.block_log.as_ref().unwrap().contains(&key),
            "the new target is sent"
        );
        // The window: no `conduit_servers` of its own, the same mob, and the
        // server's tag through `handleBlockEntityData`.
        let mut client_resources = None;
        let mut client = client(&mut client_resources);
        let watched = 77_u32;
        client.spawn_entity(
            watched as i32,
            "minecraft:drowned",
            [4.5, 100.0, 8.5],
            [0.0; 3],
        );
        client.persist.uuids.insert(watched, uuid);
        let kind = client
            .remote
            .as_ref()
            .and_then(|r| r.builtin.get("minecraft:block_entity_type"))
            .and_then(|ids| ids.id("minecraft:conduit"))
            .expect("the conduit's block entity type");
        client.remote_block_entity_data(
            CONDUIT,
            kind,
            mod_api::BlockEntityDataSource::UpdatePacket,
            &tag,
        );
        let target = client
            .test_conduit_target(CONDUIT)
            .expect("the window sees the hunted mob");
        let (position, eye_y) = {
            let m = client.mobs.iter().find(|m| m.id == watched).unwrap();
            (m.position, m.eye_y())
        };
        assert_eq!(target.eye_y, eye_y);
        let mut anims = sunrise_client::block_entity_anim::Conduit::default();
        let particles = anims.client_tick(
            CONDUIT,
            1,
            |_| true,
            |_| false,
            Some(target),
            &mut client.world.random,
        );
        // One particle, at the eye: a frame's own particles need a frame block.
        assert_eq!(particles.len(), 1);
        assert_eq!(particles[0].0, [position.x, eye_y, position.z]);
        // The conduit stopped hunting: its empty tag drops the target.
        client.remote_block_entity_data(
            CONDUIT,
            kind,
            mod_api::BlockEntityDataSource::UpdatePacket,
            &Tag::Compound(Default::default()),
        );
        assert!(
            client.test_conduit_target(CONDUIT).is_none(),
            "the target is dropped"
        );
    }
}
