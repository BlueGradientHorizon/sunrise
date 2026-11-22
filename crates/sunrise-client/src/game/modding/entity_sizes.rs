//! `entity_size` (`EntityEvent.Size`): vanilla's `Entity.refreshDimensions` lets a listener
//! change the size it just computed, whenever the pose changes and from the constructor; a
//! player's standing, crouching and swimming candidates are also resolved before its motion, for
//! the pose fit tests.
//! Sunrise computes a mob's size from its state (`Mob::dims`) and keeps a player's in its body
//! (`PlayerMotion::refresh_dimensions`), so the hook is asked here instead: a pass tells it each
//! entity whose pose or vanilla size changed since it was last asked (or that was never asked,
//! the constructor's call), and keeps the answer on the entity (`pose::SizeAnswer`) for as long
//! as that pose and vanilla size hold. The server's mobs are asked before and after their ticks;
//! a guest's body once per server tick, with its candidates, before its motion; the client's own
//! body for its candidates before its tick and for its current pose after it; and a client's copy
//! of a server mob when it is added and after each synched value or attribute packet applied to
//! it (`Entity.onSyncedDataUpdated(DATA_POSE)` and the other values its size follows, and
//! `minecraft:scale`, each of which runs `refreshDimensions` on a client).

use super::Game;
use mod_api::{EntityRef, Pose};
use sunrise_client::pose::{Dimensions, SizeAnswer};

impl Game<'_> {
    /// Ask `entity_size` for an entity whose vanilla size in `pose` is `vanilla`, unless
    /// `previous` was already asked for both. `None`: nothing to ask, keep `previous`.
    fn ask_entity_size(
        &self,
        id: u32,
        pose: Pose,
        vanilla: [f32; 3],
        previous: Option<SizeAnswer>,
    ) -> Option<SizeAnswer> {
        if previous.is_some_and(|answer| answer.asked(pose, vanilla)) {
            return None;
        }
        let entity = EntityRef::new(self.native_lifecycle.epoch()?, id);
        // `getOldSize`: the size it had (the last answer's, or vanilla's on the first ask).
        let old = previous.map_or(vanilla, |answer| answer.effective());
        let mut size = vanilla;
        self.native_entity_size(entity, pose, old, &mut size);
        // Bound mod answers once, before collision/pathfinding consume them. 32 blocks
        // exceeds even the dragon's natural width (16), while capping a path node's
        // bounding-box scan at 33^3 cells and keeping integer extents representable.
        // Invalid answers retain vanilla size, just like non-finite answers.
        let usable = size
            .iter()
            .all(|v| v.is_finite() && *v >= 0.0 && *v <= 32.0);
        Some(SizeAnswer {
            pose,
            vanilla,
            size: (usable && size != vanilla).then_some(size),
        })
    }

    /// The server's mobs whose pose or size changed since they were last asked.
    pub(super) fn native_mob_sizes(&mut self) {
        if !mod_api::__private::listened(mod_api::HookId::EntitySize) {
            return;
        }
        for i in 0..self.mobs.len() {
            let m = &self.mobs[i];
            let (id, pose, vanilla, previous) = (m.id, m.pose(), m.vanilla_dims(), m.size_answer);
            if let Some(answer) = self.ask_entity_size(id, pose, vanilla, previous)
                && let Some(m) = self.mobs.get_mut(i)
                && m.id == id
            {
                m.size_answer = Some(answer);
            }
        }
    }

    /// A client's copy of the server's mob `id`, when its pose or vanilla size changed since it
    /// was last asked (or it was never asked: it was just added).
    pub(super) fn native_remote_mob_size(&mut self, id: u32) {
        if !mod_api::__private::listened(mod_api::HookId::EntitySize) {
            return;
        }
        let Some(i) = self.mob_index.find(&self.mobs, id) else {
            return;
        };
        let m = &self.mobs[i];
        let (pose, vanilla, previous) = (m.pose(), m.vanilla_dims(), m.size_answer);
        if let Some(answer) = self.ask_entity_size(id, pose, vanilla, previous)
            && let Some(m) = self.mobs.get_mut(i)
            && m.id == id
        {
            m.size_answer = Some(answer);
        }
    }

    /// A body's pose or size changed since it was last asked: ask, and size it by the answer.
    fn ask_body_size(
        &self,
        id: u32,
        body: &sunrise_client::motion::PlayerMotion,
    ) -> Option<SizeAnswer> {
        let vanilla = body.pose.dimensions(body.scale).to_array();
        body.pose_fit_sizes
            .iter()
            .flatten()
            .copied()
            .find(|answer| answer.asked(body.pose.into(), vanilla))
            .or_else(|| self.ask_entity_size(id, body.pose.into(), vanilla, body.size_answer))
    }

    /// Snapshot only the three candidate boxes the shared pose fit test asks about.
    fn body_fit_sizes(
        &self,
        id: u32,
        body: &sunrise_client::motion::PlayerMotion,
        previous: Option<SizeAnswer>,
    ) -> [Option<SizeAnswer>; 3] {
        if !mod_api::__private::listened(mod_api::HookId::EntitySize) {
            return [None; 3];
        }
        let Ok(scale) = body.attributes.computed_value_of("minecraft:scale") else {
            return [None; 3];
        };
        use sunrise_client::pose::Pose as MotionPose;
        [
            MotionPose::Standing,
            MotionPose::Crouching,
            MotionPose::Swimming,
        ]
        .map(|pose| {
            let vanilla = pose.dimensions(scale as f32).to_array();
            self.ask_entity_size(id, pose.into(), vanilla, previous)
                .or_else(|| previous.filter(|answer| answer.asked(pose.into(), vanilla)))
        })
    }

    /// Resolve candidate sizes with the read Host before borrowing the body for motion.
    pub(super) fn native_local_player_fit_sizes(&mut self) {
        if !mod_api::__private::listened(mod_api::HookId::EntitySize) {
            return;
        }
        let Some(id) = self.remote.as_ref().map(|remote| remote.entity_id as u32) else {
            return;
        };
        let Some(answers) = self
            .player
            .as_ref()
            .map(|body| self.body_fit_sizes(id, body, body.size_answer))
        else {
            return;
        };
        if let Some(body) = self.player.as_mut() {
            body.pose_fit_sizes = answers;
        }
    }

    fn apply_body_size(body: &mut sunrise_client::motion::PlayerMotion, answer: SizeAnswer) {
        body.size_answer = Some(answer);
        body.dimensions = Dimensions::from_array(answer.effective());
    }

    /// The client's own body, after its tick (`LocalPlayer.tick` set its pose).
    pub(super) fn native_local_player_size(&mut self) {
        if !mod_api::__private::listened(mod_api::HookId::EntitySize) {
            return;
        }
        let Some(id) = self.remote.as_ref().map(|remote| remote.entity_id as u32) else {
            return;
        };
        let Some(answer) = self
            .player
            .as_ref()
            .and_then(|body| self.ask_body_size(id, body))
        else {
            return;
        };
        if let Some(body) = self.player.as_mut() {
            Self::apply_body_size(body, answer);
        }
    }

    /// Each guest's body on the server, once a tick.
    pub(super) fn native_guest_sizes(&mut self) {
        if !mod_api::__private::listened(mod_api::HookId::EntitySize) {
            return;
        }
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let asked: Vec<_> = host
            .hub
            .guests
            .as_slice()
            .iter()
            .filter_map(|guest| {
                let body = guest.motion.as_deref()?;
                let answer = self.ask_body_size(guest.entity_id as u32, body);
                Some((
                    guest.conn,
                    answer,
                    self.body_fit_sizes(guest.entity_id as u32, body, answer.or(body.size_answer)),
                ))
            })
            .collect();
        for (conn, answer, fit_sizes) in asked {
            if let Some(body) = self
                .lan
                .as_mut()
                .and_then(|host| host.hub.guests.motion_mut(conn))
            {
                if let Some(answer) = answer {
                    Self::apply_body_size(body, answer);
                }
                body.pose_fit_sizes = fit_sizes;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::native_entity_refs::tests::with_game;
    use mod_api::{EntitySizeEvent, Hooks, Host, Pose};
    use std::sync::{Arc, Mutex};

    type Calls = Arc<Mutex<Vec<(Pose, [f32; 3], [f32; 3])>>>;

    struct Grow(Calls);

    #[mod_api::__subscriptions]
    impl Hooks for Grow {
        fn entity_size(&self, _: &dyn Host, event: &mut EntitySizeEvent<'_>) {
            self.0
                .lock()
                .unwrap()
                .push((event.pose, event.old_size, *event.size));
            event.size[0] *= 2.0;
            event.size[1] *= 2.0;
        }
    }

    #[test]
    #[ignore = "daily"]
    fn a_listeners_size_sizes_the_mobs_box_until_vanilla_changes_it() {
        let calls = Calls::default();
        let mods = vec![mod_api::ModEntry::new(
            "grow",
            Box::new(Grow(calls.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                let id = game
                    .spawn_mob_spawned(
                        "cow",
                        [1.0, 70.0, 3.0],
                        crate::game::creatures::SpawnSite::Plain,
                    )
                    .unwrap();
                let i = game.mobs.iter().position(|m| m.id == id).unwrap();
                let vanilla = game.mobs[i].vanilla_dims();
                game.native_mob_sizes();
                let grown = [vanilla[0] * 2.0, vanilla[1] * 2.0, vanilla[2]];
                let m = &game.mobs[i];
                assert_eq!((m.width(), m.height()), (grown[0], grown[1]));
                assert_eq!(m.eye_height(), f64::from(vanilla[2]));
                assert!((m.bounds().size().y - f64::from(grown[1])).abs() < 1e-9);
                // Asked once for a pose and vanilla size, from the constructor's.
                game.native_mob_sizes();
                assert_eq!(
                    calls.lock().unwrap().as_slice(),
                    &[(m_pose(game, i), vanilla, vanilla)]
                );
                // Grown down to a baby: asked again, from the size it had.
                sunrise_client::mobs::finalize::set_baby(&mut game.mobs[i], true);
                game.native_mob_sizes();
                let calls = calls.lock().unwrap();
                assert_eq!(calls.len(), 2);
                assert_eq!(calls[1].1, grown);
                assert_eq!(calls[1].2, game.mobs[i].vanilla_dims());
            })
        });
    }

    struct SetSize(Arc<Mutex<[f32; 3]>>);

    #[mod_api::__subscriptions]
    impl Hooks for SetSize {
        fn entity_size(&self, _: &dyn Host, event: &mut EntitySizeEvent<'_>) {
            *event.size = *self.0.lock().unwrap();
        }
    }

    #[test]
    #[ignore = "daily"]
    fn invalid_size_answers_leave_the_mobs_default_box() {
        let size = Arc::new(Mutex::new([1.0; 3]));
        let mods = vec![mod_api::ModEntry::new(
            "set_size",
            Box::new(SetSize(size.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                let id = game
                    .spawn_mob_spawned(
                        "cow",
                        [1.0, 70.0, 3.0],
                        crate::game::creatures::SpawnSite::Plain,
                    )
                    .unwrap();
                let i = game.mobs.iter().position(|m| m.id == id).unwrap();
                let vanilla = game.mobs[i].vanilla_dims();
                let default_dims = game.mobs[i].dims();
                for invalid in [
                    f32::NAN,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    -1.0,
                    32.001,
                    1e30,
                ] {
                    for axis in 0..3 {
                        let mut answer = vanilla;
                        answer[axis] = invalid;
                        *size.lock().unwrap() = answer;
                        game.mobs[i].size_answer = None;
                        game.native_mob_sizes();
                        let mob = &game.mobs[i];
                        assert_eq!(mob.size_answer.unwrap().effective(), vanilla, "{answer:?}");
                        assert_eq!(mob.dims(), default_dims, "{answer:?}");
                    }
                }
                // The reported crash input must never become a pathfinding extent.
                *size.lock().unwrap() = [1e30; 3];
                game.mobs[i].size_answer = None;
                game.native_mob_sizes();
                assert_eq!(game.mobs[i].dims(), default_dims);
                // Both endpoints are valid: zero-size entities are vanilla too.
                for accepted in [0.0, 32.0] {
                    *size.lock().unwrap() = [accepted; 3];
                    game.mobs[i].size_answer = None;
                    game.native_mob_sizes();
                    assert_eq!(
                        game.mobs[i].dims(),
                        (accepted.into(), accepted.into(), accepted.into())
                    );
                }
                *size.lock().unwrap() = [0.0; 3];
                game.mobs[i].size_answer = None;
                for _ in 0..4 {
                    game.native_mob_sizes();
                    let mob = &game.mobs[i];
                    let path_mob = mob.path_mob();
                    assert_eq!((path_mob.width, path_mob.height), (0.0, 0.0));
                    let world = game.mob_world_base(&[]);
                    assert!(world.no_collision(mob.bounds()));
                    use sunrise_client::mobs::path::{Evaluator, Finder};
                    for evaluator in [Evaluator::Walk, Evaluator::Swim { breach: true }] {
                        let finder = Finder::new(&world, &path_mob, evaluator);
                        let _ = finder.walk_path_type(1, 70, 3);
                        let _ = finder.find(&[[3, 70, 3]], 8.0, 0, 32);
                    }
                }
            })
        });
    }

    #[test]
    #[ignore = "daily"]
    fn partial_collision_sizes_complete_navigation() {
        // Resource loading and the shared mod fixture lock are not navigation.
        // Start the deadline only once the fixture is ready, while keeping it
        // outside the worker so a saturated collision loop still fails boundedly.
        let (ready, started) = std::sync::mpsc::channel();
        let (done, result) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let size = Arc::new(Mutex::new([0.0; 3]));
            let mods = vec![mod_api::ModEntry::new(
                "set_size",
                Box::new(SetSize(size.clone())),
            )]
            .into();
            mod_platform::testing::with_mods(mods, || {
                with_game(|game| {
                    ready.send(()).unwrap();
                    use sunrise_client::mobs::path::{Evaluator, Finder, PathType};
                    game.world
                        .chunks
                        .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
                    let registry = &game.pack.assets.registry;
                    let stone = registry.default_state("minecraft:stone").unwrap();
                    let fence = registry.default_state("minecraft:oak_fence").unwrap();
                    for x in 0..4 {
                        for z in 0..4 {
                            game.world.set(x, 69, z, stone).unwrap();
                        }
                    }
                    game.world.set(0, 70, 0, fence).unwrap();
                    // At the block corner, even subnormal widths remain represented
                    // in the box's x/z extents; the fence post is not on the route.
                    let id = game
                        .spawn_mob_spawned(
                            "cow",
                            [0.0, 70.0, 0.0],
                            crate::game::creatures::SpawnSite::Plain,
                        )
                        .unwrap();
                    let i = game.mobs.iter().position(|m| m.id == id).unwrap();
                    let vanilla = game.mobs[i].vanilla_dims();
                    for answer in [[0.0; 3], [1e-30; 3], [1e-45; 3], vanilla] {
                        *size.lock().unwrap() = answer;
                        game.mobs[i].size_answer = None;
                        game.native_mob_sizes();
                        let mut mob = game.mobs[i].path_mob();
                        mob.on_ground = true;
                        let world = game.mob_world_base(&[]);
                        let finder = Finder::new(&world, &mob, Evaluator::Walk);
                        assert_eq!(finder.walk_path_type(0, 70, 0), PathType::Fence);
                        let path = finder.find(&[[2, 70, 0]], 8.0, 0, 32);
                        assert!(path.is_some(), "{answer:?}");
                    }
                    let id = game
                        .spawn_mob_spawned(
                            "silverfish",
                            [0.99, 70.0, 0.921591],
                            crate::game::creatures::SpawnSite::Plain,
                        )
                        .unwrap();
                    let i = game.mobs.iter().position(|m| m.id == id).unwrap();
                    game.mobs[i].extra.model_scale = 0.0625;
                    *size.lock().unwrap() = game.mobs[i].vanilla_dims();
                    game.native_mob_sizes();
                    let mut mob = game.mobs[i].path_mob();
                    mob.on_ground = true;
                    assert!((mob.width - 0.025).abs() < 1e-8);
                    assert!((mob.height - 0.01875).abs() < 1e-8);
                    let world = game.mob_world_base(&[]);
                    let finder = Finder::new(&world, &mob, Evaluator::Walk);
                    assert_eq!(finder.walk_path_type(0, 70, 0), PathType::Fence);
                    assert!(finder.find(&[[-1, 70, 0]], 8.0, 0, 32).is_some());
                });
            });
            done.send(()).unwrap();
        });
        started.recv().expect("navigation fixture must initialize");
        result
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("partial-collision navigation must finish within 30 seconds");
        worker.join().unwrap();
    }

    struct ShortCrouch(Arc<std::sync::atomic::AtomicUsize>);

    #[mod_api::__subscriptions]
    impl Hooks for ShortCrouch {
        fn entity_size(&self, _: &dyn Host, event: &mut EntitySizeEvent<'_>) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if event.pose == Pose::Crouching {
                event.size[1] = event.size[0] / 0.6 * 0.8;
                event.size[2] = event.size[0];
            }
        }
    }

    #[test]
    #[ignore = "daily"]
    fn pose_fit_uses_the_resolved_crouch_size_including_cloned_motion() {
        use sunrise_client::{
            motion::{MotionWorld, PlayerMotion},
            pose::Pose as MotionPose,
        };
        use sunrise_core::geometry::Vec3;
        for subscribed in [false, true] {
            let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let mods: mod_api::Mods = if subscribed {
                vec![mod_api::ModEntry::new(
                    "short_crouch",
                    Box::new(ShortCrouch(calls.clone())),
                )]
                .into()
            } else {
                Vec::new().into()
            };
            mod_platform::testing::with_mods(mods, || {
                with_game(|game| {
                    game.world
                        .chunks
                        .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
                    let stone = game
                        .pack
                        .assets
                        .registry
                        .default_state("minecraft:stone")
                        .unwrap();
                    game.world.set(0, 65, 0, stone).unwrap();
                    let mut body =
                        PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap();
                    body.shift_key_down = true;
                    body.pose_fit_sizes = game.body_fit_sizes(1, &body, None);
                    assert_eq!(body.pose_fit_sizes.iter().any(Option::is_some), subscribed);
                    assert_eq!(
                        calls.load(std::sync::atomic::Ordering::Relaxed),
                        if subscribed { 3 } else { 0 }
                    );
                    body.size_answer = body.pose_fit_sizes[0];
                    body.pose_fit_sizes = game.body_fit_sizes(1, &body, body.size_answer);
                    assert_eq!(
                        calls.load(std::sync::atomic::Ordering::Relaxed),
                        if subscribed { 5 } else { 0 }
                    );
                    // Motion snapshots own their candidate answers, not a pointer into Game.
                    let mut restored = body.clone();
                    assert_eq!(restored.pose_fit_sizes, body.pose_fit_sizes);
                    body.pose_fit_sizes = [None; 3];
                    let side = game_side!(game);
                    let mut world = MotionWorld {
                        side: &side,
                        entities: &[],
                        player: &mut restored,
                        scene: &mut game.world,
                        assets: &game.pack.assets,
                        collision_binding: game.collision,
                        resources: game.motion,
                        health: &game.health_resources,
                        tag_items: None,
                        loop_context: &game.loop_context,
                        damage_hooks: &game.damage_hooks,
                        block_registries: Some(&game.pack.block_registries),
                    };
                    assert_eq!(
                        world.can_fit_pose(MotionPose::Crouching).unwrap(),
                        subscribed
                    );
                    world.update_local_crouching().unwrap();
                    assert_eq!(world.player.local_crouching, subscribed);
                    world.update_player_pose().unwrap();
                    assert_eq!(
                        world.player.pose,
                        if subscribed {
                            MotionPose::Crouching
                        } else {
                            MotionPose::Swimming
                        }
                    );
                    if subscribed {
                        assert_eq!(world.player.dimensions.height, 0.8);
                        // A scale change after prefill refuses the old candidate answer.
                        let key = world.player.attributes.key("minecraft:scale").unwrap();
                        let handle = world
                            .player
                            .attributes
                            .get_instance(Some(key))
                            .unwrap()
                            .unwrap();
                        world
                            .player
                            .attributes
                            .edit(&handle, |value, observer| {
                                value.set_base_value(0.9, observer)
                            })
                            .unwrap();
                        assert!(!world.can_fit_pose(MotionPose::Crouching).unwrap());
                    }
                })
            });
        }
    }

    fn m_pose(game: &crate::game::Game<'_>, i: usize) -> Pose {
        game.mobs[i].pose()
    }

    /// A client logged in to a memory server, receiving that server's packets by their wire form.
    fn with_loopback_client(test: impl FnOnce(&mut crate::game::Game<'_>)) {
        use crate::game::{
            native_client_session::tests::{finish_login, logged_client},
            native_entity_refs::tests::with_client_game,
        };
        with_client_game(|game| {
            let mut server = sunrise_protocol::connection::Connection::local(logged_client(game));
            server
                .reader
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            finish_login(game, &mut server);
            game.native_lifecycle
                .begin_open(mod_api::Authority::RemoteClient);
            test(game);
        });
    }

    fn deliver(
        game: &mut crate::game::Game<'_>,
        packet: sunrise_protocol::packets::play::Clientbound,
    ) {
        use sunrise_protocol::{buf::Reader, packets::play::Clientbound};
        let body = packet.encode();
        let mut reader = Reader::new(&body);
        let id = reader.varint().unwrap();
        game.handle_packet(Clientbound::decode(id, reader.rest()).unwrap())
            .unwrap();
    }

    fn add_entity(game: &mut crate::game::Game<'_>, id: i32, kind: &str, position: [f64; 3]) {
        use sunrise_protocol::packets::play::{AddEntity, Clientbound};
        let kind = game
            .remote
            .as_ref()
            .unwrap()
            .builtin
            .get("minecraft:entity_type")
            .unwrap()
            .id(kind)
            .unwrap();
        deliver(
            game,
            Clientbound::AddEntity(AddEntity {
                id,
                uuid: id as u128,
                kind,
                position,
                velocity: [0.0; 3],
                pitch: 0,
                yaw: 0,
                head_yaw: 0,
                data: 0,
            }),
        );
    }

    /// `Entity.onSyncedDataUpdated(DATA_POSE)` → `refreshDimensions` on a client: its copy of
    /// a server villager asks when it is added (the constructor's ask), then once when the
    /// server's sleeping pose arrives, from the size the copy had.
    #[test]
    #[ignore = "daily"]
    fn a_client_copy_asks_once_when_its_synched_pose_changes_to_sleeping() {
        use sunrise_protocol::{
            entity_data::{Value, serializer},
            packets::play::{Clientbound, SetEntityData},
        };
        let calls = Calls::default();
        let mods = vec![mod_api::ModEntry::new(
            "grow",
            Box::new(Grow(calls.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_loopback_client(|game| {
                add_entity(game, 66, "minecraft:villager", [8.5, 70.0, 8.5]);
                // EntityType.VILLAGER: 0.6 by 1.95, its eyes at 1.62.
                let standing = [0.6, 1.95, 1.62];
                let grown = [0.6_f32 * 2.0, 1.95_f32 * 2.0, 1.62];
                assert_eq!(
                    calls.lock().unwrap().as_slice(),
                    &[(Pose::Standing, standing, standing)]
                );
                let sleep = || {
                    Clientbound::SetEntityData(SetEntityData {
                        id: 66,
                        values: vec![
                            (6, serializer::POSE, Value::Id(2)),
                            (
                                14,
                                serializer::OPTIONAL_BLOCK_POS,
                                Value::OptPos(Some([8, 70, 8])),
                            ),
                        ],
                    })
                };
                deliver(game, sleep());
                // The same pose again is no size change: the answer holds.
                deliver(game, sleep());
                // LivingEntity.SLEEPING_DIMENSIONS: 0.2 by 0.2, its eyes at 0.2.
                let sleeping = [0.2; 3];
                assert_eq!(
                    calls.lock().unwrap().as_slice(),
                    &[
                        (Pose::Standing, standing, standing),
                        (Pose::Sleeping, grown, sleeping),
                    ]
                );
                let m = game.mobs.iter().find(|m| m.id == 66).unwrap();
                assert_eq!(m.pose(), Pose::Sleeping);
                assert_eq!((m.width(), m.height()), (0.4, 0.4));
            })
        });
    }

    /// A client's copy of a server mob takes the answer for its box, the crosshair's pick and
    /// its hitbox outline (F3+B), and asks again when the server's `minecraft:scale` changes its
    /// size.
    #[test]
    #[ignore = "daily"]
    fn a_client_copys_box_and_hitbox_outline_take_the_answer() {
        use sunrise_client::gizmos::WHITE;
        use sunrise_protocol::packets::play::{AttributeSnapshot, Clientbound, UpdateAttributes};
        let calls = Calls::default();
        let mods = vec![mod_api::ModEntry::new(
            "grow",
            Box::new(Grow(calls.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_loopback_client(|game| {
                let at = [8.5, 70.0, 8.5];
                add_entity(game, 31, "minecraft:cow", at);
                // F3+B, on entries with no file behind them: nothing is saved.
                game.debug_entries = sunrise_client::debug_entries::DebugEntries::default();
                game.debug_entries.toggle_status("entity_hitboxes", false);
                // The box and the outline's white edges, for a size of `width` by `height`.
                let check = |game: &crate::game::Game<'_>, width: f32, height: f32| {
                    let half = f64::from(width) / 2.0;
                    let min = [at[0] - half, at[1], at[2] - half];
                    let max = [at[0] + half, at[1] + f64::from(height), at[2] + half];
                    let m = game.mobs.iter().find(|m| m.id == 31).unwrap();
                    let b = m.bounds();
                    let (lo, hi) = (b.min_position(), b.max_position());
                    assert_eq!([lo.x, lo.y, lo.z], min);
                    assert_eq!([hi.x, hi.y, hi.z], max);
                    let gizmos = game.debug_gizmos(1.0, [0.0; 3]);
                    let ends: Vec<[f64; 3]> = gizmos
                        .lines
                        .iter()
                        .filter(|line| line.color == WHITE)
                        .flat_map(|line| [line.start, line.end])
                        .collect();
                    assert_eq!(ends.len(), 24, "one white cuboid: the cow's outline");
                    let extent = |pick: fn(f64, f64) -> f64, from: f64| {
                        std::array::from_fn(|axis| {
                            ends.iter().map(|end| end[axis]).fold(from, pick)
                        })
                    };
                    assert_eq!(extent(f64::min, f64::INFINITY), min);
                    assert_eq!(extent(f64::max, f64::NEG_INFINITY), max);
                };
                // EntityType.COW: 0.9 by 1.4, its eyes at 1.3; the listener doubles the box.
                check(game, 0.9_f32 * 2.0, 1.4_f32 * 2.0);
                // Aimed over a vanilla cow's back (y 71.4) but into the answered box: the
                // crosshair picks the cow.
                game.title = None;
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
                game.player = Some(
                    sunrise_client::motion::PlayerMotion::new(
                        game.motion,
                        sunrise_core::geometry::Vec3::new(5.5, 70.0, 8.5),
                    )
                    .unwrap(),
                );
                (game.yaw, game.pitch, game.entity_reach) = (-90.0, 0.0, 3.0);
                game.refresh_client_pick(1.0);
                let cow = game.entity(31).unwrap();
                assert!(matches!(
                    game.crosshair_target(),
                    Some(mod_api::CrosshairTarget::Entity { entity, .. }) if entity == cow
                ));
                let scale = game
                    .motion
                    .attributes
                    .id("minecraft:scale")
                    .unwrap()
                    .registry_id();
                deliver(
                    game,
                    Clientbound::UpdateAttributes(UpdateAttributes {
                        id: 31,
                        values: vec![AttributeSnapshot {
                            attribute: scale,
                            base: 2.0,
                            modifiers: Vec::new(),
                        }],
                    }),
                );
                check(game, 0.9_f32 * 2.0 * 2.0, 1.4_f32 * 2.0 * 2.0);
                let calls = calls.lock().unwrap();
                assert_eq!(calls.len(), 2);
                assert_eq!(
                    calls[1],
                    (
                        Pose::Standing,
                        [0.9_f32 * 2.0, 1.4_f32 * 2.0, 1.3],
                        [0.9_f32 * 2.0, 1.4_f32 * 2.0, 1.3_f32 * 2.0]
                    )
                );
            })
        });
    }
}
