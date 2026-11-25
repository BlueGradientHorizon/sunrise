#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game,
    commands::Target,
    entity::Entity,
    pick_entity::{
        aabb_extents,
        pick_sections::{PickKind, PickMember},
    },
    remote::Remote,
    text_displays::{RemoteTextDisplay, Shows},
    transactions::After,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use core::ops::ControlFlow;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    BlockFace, BlockShapeKind, BlockUpdateFlags, ClipBlock, ClipFluid, ClipHit, DimensionKey,
    EntityRayHit, EntityRef, LevelReadError, LevelRef, NeighbourUpdate,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::join_seq::{
    JoinList, JoinSeq, Joined,
    residency::table::{EntityLookup, Location, SectionSearch},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::{Aabb, Vec3};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_items::items::ItemStack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::registry::BlockStateId;

impl Game<'_> {
    /// `LevelRenderer.destroyBlockProgress` for this client's own view only.
    pub(in crate::game) fn level_set_client_break_progress(
        &mut self,
        level: LevelRef,
        breaker: i32,
        pos: [i32; 3],
        progress: i32,
    ) -> bool {
        if self.native_client_level() != Some(level) {
            return false;
        }
        self.destroy_block_progress(breaker, pos, progress);
        true
    }

    /// Client-side `Entity.getType`: `minecraft:zombie`.
    pub(in crate::game) fn client_entity_kind(
        &self,
        entity: EntityRef,
    ) -> Option<mod_api::EntityTypeKey> {
        match self.native_entity_read_target(entity) {
            Some(target) => self.native_entity_kind_key(target),
            None => {
                mod_api::__private::resolved_entity_type_key(self.native_client_tracked(entity)?.0)
            }
        }
    }

    /// The level the entity is in on the client (`Entity.level`).
    pub(in crate::game) fn client_entity_level(&self, entity: EntityRef) -> Option<LevelRef> {
        (self.native_client_entity_target(entity).is_some()
            || self.native_client_tracked(entity).is_some())
        .then(|| LevelRef::new(entity.epoch(), self.native_level_key()))
    }

    pub(in crate::game) fn client_item_entity(
        &self,
        _entity: EntityRef,
    ) -> Option<mod_api::ItemEntityRef> {
        None
    }

    /// Visit intersecting, readable entities in `ClientLevel.getEntities` query order.
    pub(in crate::game) fn client_level_visit_entities_in(
        &self,
        level: LevelRef,
        min: [f64; 3],
        max: [f64; 3],
        visit: &mut dyn FnMut(EntityRef) -> ControlFlow<()>,
    ) -> Result<ControlFlow<()>, LevelReadError> {
        if !self.client_level_handle_current(level) {
            return Err(LevelReadError::Stale);
        }
        if !min.iter().chain(&max).all(|v| v.is_finite())
            || self.native_client_level() != Some(level)
        {
            return Err(LevelReadError::Unavailable);
        }
        let bounds = Aabb::new(
            Vec3::new(min[0], min[1], min[2]),
            Vec3::new(max[0], max[1], max[2]),
        );
        let search = SectionSearch::new(bounds);
        for member in self.client_section_candidates(search) {
            let entity = EntityRef::new(level.epoch(), member.id);
            if self
                .client_query_body(entity, member)
                .is_some_and(|body| bounds.intersects(body))
                && visit(entity).is_break()
            {
                return Ok(ControlFlow::Break(()));
            }
        }
        if let Some(dragon) = self.dragon.as_ref().filter(|d| d.id != 0) {
            let mut cursor = None;
            while let Some(id) = self.dragon.next_part(&mut cursor) {
                if let Some(part) = crate::game::dragon::dragon_part_index(dragon.id, id)
                    && bounds.intersects(dragon.part_geometry(part).1)
                {
                    let entity = EntityRef::new(level.epoch(), id);
                    if self.native_client_entity_target(entity).is_some()
                        && visit(entity).is_break()
                    {
                        return Ok(ControlFlow::Break(()));
                    }
                }
            }
        }
        Ok(ControlFlow::Continue(()))
    }

    /// Resolve a client section member's box without consulting server residency.
    pub(in crate::game::client_side) fn client_query_body(
        &self,
        entity: EntityRef,
        member: PickMember,
    ) -> Option<Aabb> {
        match member.kind {
            PickKind::RemoteDisplay => {
                let [x, y, z] = self.side_client_display_position(entity)?;
                let p = Vec3::new(x, y, z);
                Some(Aabb::new(p, p))
            }
            // Indexed only for picking; not readable box-query bodies.
            PickKind::Cushion
            | PickKind::Interaction
            | PickKind::RemoteGeneral
            | PickKind::RemoteKnot
            | PickKind::LocalKnot
            | PickKind::Crystal => None,
            PickKind::LocalPlayer if self.mode_id() != 3 => {
                self.player.as_ref().map(|p| p.bounds())
            }
            PickKind::LocalPlayer => None,
            _ => {
                let remote = self.remote.as_ref()?;
                remote.tracked(member.id as i32)?;
                let id = member.id;
                if let Some(dragon) = self.dragon.as_ref().filter(|d| d.id == id) {
                    return Some(dragon.body_bounds());
                }
                if let Some(frame) = self.item_frames.iter().find(|f| f.id == id) {
                    return Some(aabb_extents(frame.bounds()));
                }
                if let Some(painting) = self.paintings.iter().find(|p| p.id == id) {
                    return Some(aabb_extents(painting.bounds()));
                }
                fn list_bounds<T: Entity>(items: &[T], id: u32) -> Option<Aabb> {
                    items
                        .iter()
                        .find(|e| e.id() == id && !e.is_removed())
                        .map(Entity::bounds)
                }
                list_bounds(&self.mobs, id)
                    .or_else(|| list_bounds(&self.dropped, id))
                    .or_else(|| list_bounds(&self.orbs, id))
                    .or_else(|| list_bounds(&self.boats, id))
                    .or_else(|| list_bounds(&self.carts, id))
                    .or_else(|| list_bounds(&self.armor_stands, id))
                    .or_else(|| list_bounds(&self.tnts, id))
                    .or_else(|| list_bounds(&self.falling, id))
                    .or_else(|| list_bounds(&self.creatures.projectiles, id))
                    .or_else(|| list_bounds(&self.rockets, id))
                    .or_else(|| list_bounds(&self.tridents, id))
                    .or_else(|| list_bounds(&self.resident_entities.general.rows, id))
            }
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::api_values::TryIntoApi;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::FRESH_CASE_LIMIT;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::bind_acting_guest;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::bind_local_player;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::collect_all_entities;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::collect_entities_in;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::fixture_root;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::prepare_entity_ticking_neighborhood;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::record_key;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::resolve_parts;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::run_fresh_cases;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::run_with_game_stack;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::test_window;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::with_client_game;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::with_game;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::with_game_at;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::with_game_role;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::with_game_role_and_facts;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_entity_refs::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use core::ops::ControlFlow;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use mod_api::__private::game::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use mod_api::{EntityRef, LevelReadError, LevelRef};

    #[test]
    #[ignore = "daily"]
    fn client_box_queries_use_live_display_positions_and_section_order() {
        use super::Game;
        use mod_api::Host;
        use std::sync::Arc;
        use sunrise_client::chunk::Chunk;
        use sunrise_protocol::{
            connection::Pipe,
            entity_data::Value,
            packets::play::{self, Clientbound as C},
        };

        const CASE: &str = "SUNRISE_DISPLAY_BOX_QUERY_CHILD";
        if run_fresh_cases(
            "game::client_side::native_entity_refs_client::tests::client_box_queries_use_live_display_positions_and_section_order",
            CASE,
            &["client"],
        ) {
            return;
        }

        const LO: [f64; 3] = [13.0, 65.0, 2.0];
        const HI: [f64; 3] = [19.0, 66.0, 4.0];
        const ADMISSIONS: [(i32, &str, [f64; 3]); 4] = [
            (31, "minecraft:item_display", [17.0, 65.5, 3.0]),
            (92, "minecraft:block_display", [15.5, 65.5, 3.0]),
            (55, "minecraft:pig", [14.5, 65.5, 3.0]),
            (11, "minecraft:text_display", [13.5, 65.5, 3.0]),
        ];
        let chunks = |game: &mut Game<'_>| {
            for x in 0..=1 {
                game.world
                    .chunks
                    .insert((x, 0), Arc::new(Chunk::empty(x, 0)));
            }
        };
        let ids = |host: &dyn Host, level, min, max| {
            collect_entities_in(host, level, min, max)
                .unwrap()
                .into_iter()
                .map(|e| e.id())
                .collect::<Vec<_>>()
        };
        match std::env::var(CASE).unwrap().as_str() {
            "client" => with_client_game(|game| {
                let (client, _server) = Pipe::pair();
                game.connect_local(client, "DisplayBoxTest").unwrap();
                let remote = game.remote.as_mut().unwrap();
                remote.logged_in = true;
                remote.entity_id = 7;
                chunks(game);
                for (id, name, position) in ADMISSIONS {
                    let kind = game
                        .remote
                        .as_ref()
                        .unwrap()
                        .builtin
                        .get("minecraft:entity_type")
                        .unwrap()
                        .id(name)
                        .unwrap();
                    game.handle_packet(C::AddEntity(play::AddEntity {
                        id,
                        uuid: id as u128,
                        kind,
                        position,
                        velocity: [0.0; 3],
                        yaw: 0,
                        pitch: 0,
                        head_yaw: 0,
                        data: 0,
                    }))
                    .unwrap();
                }
                let level = game.level("minecraft:overworld").unwrap();
                assert_eq!(ids(game, level, HI, LO), [92, 55, 11, 31]);
                let mut seen = Vec::new();
                assert_eq!(
                    level.visit_entities_in(game, LO, HI, &mut |entity| {
                        seen.push(entity.id());
                        let (_, kind, at) = ADMISSIONS
                            .into_iter()
                            .find(|(id, _, _)| *id as u32 == entity.id())
                            .unwrap();
                        assert_eq!(
                            entity.kind(game).and_then(|k| k.id(game)).as_deref(),
                            Some(kind)
                        );
                        assert_eq!(entity.position(game), Some(at));
                        if entity.id() != 55 {
                            assert_eq!(entity.bounding_box(game), Some([at; 2]));
                        }
                        ControlFlow::Continue(())
                    }),
                    Ok(ControlFlow::Continue(()))
                );
                assert_eq!(seen, [92, 55, 11, 31]);

                let block = game.entity(92).unwrap();
                game.text_displays.0.get_mut(&92).unwrap().metadata(&[
                    (10, 1, Value::Int(3)),
                    (20, 2, Value::Float(4.0)),
                    (21, 2, Value::Float(4.0)),
                ]);
                game.handle_packet(C::EntityPositionSync(play::EntityPositionSync {
                    id: 92,
                    path: play::PositionPath::Linear([18.5, 65.5, 3.0]),
                    yaw: 0.0,
                    pitch: 0.0,
                    on_ground: false,
                }))
                .unwrap();
                assert_eq!(block.position(game), Some([15.5, 65.5, 3.0]));
                assert_eq!(block.bounding_box(game), Some([[15.5, 65.5, 3.0]; 2]));
                assert_eq!(ids(game, level, LO, HI), [92, 55, 11, 31]);

                game.tick_text_displays(true);
                assert_eq!(ids(game, level, LO, HI), [55, 11, 31, 92]);
                let tight_lo = [16.25, 65.0, 2.5];
                let tight_hi = [16.75, 66.0, 3.5];
                let mut seen = Vec::new();
                let stopped = level.visit_entities_in(game, LO, HI, &mut |entity| {
                    seen.push(entity.id());
                    assert_eq!(ids(game, level, tight_lo, tight_hi), [92]);
                    ControlFlow::Break(())
                });
                assert_eq!(stopped, Ok(ControlFlow::Break(())));
                assert_eq!(seen, [55]);
                assert_eq!(block.position(game), Some([16.5, 65.5, 3.0]));
                assert_eq!(block.bounding_box(game), Some([[16.5, 65.5, 3.0]; 2]));
                assert!(ids(game, level, [16.5, 65.0, 2.5], tight_hi).is_empty());
                let culling_only_lo = [16.5, 67.0, 2.5];
                let culling_only_hi = [16.75, 68.0, 3.5];
                assert!(ids(game, level, culling_only_lo, culling_only_hi).is_empty());
                let (lo, hi) = game.text_displays.0[&92].interpolated_bounding_box(1.0);
                let culling = super::aabb_extents((lo.to_array(), hi.to_array()));
                let culling_only = super::aabb_extents((culling_only_lo, culling_only_hi));
                assert!(culling.intersects(culling_only));
                let mut picked = Vec::new();
                game.for_each_pickable_in_bounds(super::aabb_extents((LO, HI)), |id, _, _| {
                    picked.push(id)
                });
                assert_eq!(picked, [55]);

                game.handle_packet(C::RemoveEntities(play::RemoveEntities { ids: vec![92] }))
                    .unwrap();
                assert!(game.entity(92).is_none());
                assert_eq!(block.position(game), None);
                assert_eq!(block.bounding_box(game), None);
                assert_eq!(ids(game, level, LO, HI), [55, 11, 31]);
            }),
            _ => unreachable!("unexpected display query case"),
        }
    }

    /// Crossings in one normal client tick land in `ClientLevel.tickEntities` order whichever
    /// Sunrise loop moved them and whether or not a read-only tick listener splits the displays
    /// off; a tracked player is filed before its `player_tick_post`; a rider snapped with its
    /// display is filed at its seat before it ticks.
    #[test]
    #[ignore = "daily"]
    fn client_crossings_land_in_tick_order_and_before_hooks_read_them() {
        use super::Game;
        use mod_api::{Hooks, Host, HostSide, ModEntry, PlayerTickPostEvent};
        use std::sync::{Arc, Mutex};
        use sunrise_client::chunk::Chunk;
        use sunrise_core::geometry::Vec3;
        use sunrise_protocol::{
            connection::Pipe,
            entity_data::Value,
            packets::play::{self, Clientbound as C},
        };

        const CASE: &str = "SUNRISE_CLIENT_CROSSING_CHILD";
        if run_fresh_cases(
            "game::client_side::native_entity_refs_client::tests::client_crossings_land_in_tick_order_and_before_hooks_read_them",
            CASE,
            &["order", "order_listened", "tracked_player", "rider"],
        ) {
            return;
        }

        /// Asks for each block display's tick alone, reading nothing; answers the box around
        /// player 18 from its `player_tick_post`.
        struct Probe(Arc<Mutex<Vec<Vec<u32>>>>);
        #[mod_api::__subscriptions]
        impl Hooks for Probe {
            fn register(
                &self,
                registrar: &mut dyn mod_api::Registrar,
                _: &mod_api::RegistrationContext,
            ) {
                registrar.entity_tick_each("minecraft:block_display");
            }
            fn entity_tick_listened(
                &self,
                _: HostSide<'_>,
                event: &mut mod_api::EntityTickListenedEvent,
            ) {
                event.listened = true;
            }
            fn player_tick_post(&self, host: HostSide<'_>, event: &mut PlayerTickPostEvent) {
                let HostSide::Client(host) = host else { return };
                if event.player.entity().id() != 18 {
                    return;
                }
                let level = host.level("minecraft:overworld").unwrap();
                let found = collect_entities_in(&*host, level, [30.0, 0.0, -0.5], [31.0, 1.8, 0.5])
                    .unwrap()
                    .into_iter()
                    .map(|e| e.id())
                    .collect();
                self.0.lock().unwrap().push(found);
            }
        }

        let ids = |host: &dyn Host, min, max| {
            let level = host.level("minecraft:overworld").unwrap();
            collect_entities_in(host, level, min, max)
                .unwrap()
                .into_iter()
                .map(|e| e.id())
                .collect::<Vec<_>>()
        };
        let connect = |game: &mut Game<'_>| {
            let (client, server) = Pipe::pair();
            std::mem::forget(server);
            game.connect_local(client, "CrossingTest").unwrap();
            let remote = game.remote.as_mut().unwrap();
            remote.logged_in = true;
            remote.entity_id = 7;
            for x in -1..=4 {
                game.world
                    .chunks
                    .insert((x, 0), Arc::new(Chunk::empty(x, 0)));
            }
        };
        let add = |game: &mut Game<'_>, id: i32, name: &str, position: [f64; 3]| {
            let kind = game
                .remote
                .as_ref()
                .unwrap()
                .builtin
                .get("minecraft:entity_type")
                .unwrap()
                .id(name)
                .unwrap();
            game.handle_packet(C::AddEntity(play::AddEntity {
                id,
                uuid: id as u128,
                kind,
                position,
                velocity: [0.0; 3],
                yaw: 0,
                pitch: 0,
                head_yaw: 0,
                data: 0,
            }))
            .unwrap();
        };
        let sync = |game: &mut Game<'_>, id: i32, to: [f64; 3]| {
            game.handle_packet(C::EntityPositionSync(play::EntityPositionSync {
                id,
                path: play::PositionPath::Linear(to),
                yaw: 0.0,
                pitch: 0.0,
                on_ground: false,
            }))
            .unwrap();
        };
        let heard = Arc::new(Mutex::new(Vec::new()));
        let case = std::env::var(CASE).unwrap();
        let mods = if case == "order" {
            Vec::new()
        } else {
            vec![ModEntry::new("probe", Box::new(Probe(heard.clone())))]
        };
        mod_platform::testing::with_mods(mods.into(), || {
            with_client_game(|game| {
                connect(game);
                match case.as_str() {
                    "order" | "order_listened" => {
                        // The pig joined first, so vanilla moves it first: both enter section 1
                        // on this tick, the pig ahead of the display the display loop moved.
                        add(game, 70, "minecraft:pig", [14.5, 65.5, 3.0]);
                        add(game, 71, "minecraft:block_display", [13.5, 65.5, 3.0]);
                        game.text_displays.0.get_mut(&71).unwrap().metadata(&[(
                            10,
                            1,
                            Value::Int(3),
                        )]);
                        sync(game, 70, [20.5, 65.5, 3.0]);
                        sync(game, 71, [22.5, 65.5, 3.0]);
                        game.tick_remote_entities(true);
                        assert_eq!(
                            game.entity(71).unwrap().position(game),
                            Some([16.5, 65.5, 3.0])
                        );
                        assert_eq!(ids(game, [16.0, 65.0, 2.5], [17.0, 66.0, 3.5]), [70, 71]);
                    }
                    "tracked_player" => {
                        let player = game
                            .remote
                            .as_ref()
                            .unwrap()
                            .builtin
                            .get("minecraft:entity_type")
                            .unwrap()
                            .id("minecraft:player")
                            .unwrap();
                        game.handle_packet(C::PlayerInfoUpdate(play::PlayerInfoUpdate {
                            actions: 1,
                            entries: vec![play::PlayerInfo {
                                uuid: 9,
                                name: Some("Player18".into()),
                                ..Default::default()
                            }],
                        }))
                        .unwrap();
                        game.handle_packet(C::AddEntity(play::AddEntity {
                            id: 18,
                            uuid: 9,
                            kind: player,
                            position: [0.5, 0.0, 0.0],
                            velocity: [0.0; 3],
                            pitch: 0,
                            yaw: 0,
                            head_yaw: 0,
                            data: 0,
                        }))
                        .unwrap();
                        // A tracked player's interpolation carries it halfway on this step, to
                        // x 30.5: section 1, beyond the padded range of a box around its new
                        // position.
                        sync(game, 18, [60.5, 0.0, 0.0]);
                        game.tick_remote_entities(true);
                        assert_eq!(*heard.lock().unwrap(), [vec![18]]);
                    }
                    "rider" => {
                        let mut local =
                            sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::ZERO)
                                .unwrap();
                        local.entity_id = 7;
                        local.position = Vec3::new(15.5, 65.5, 3.0);
                        game.player = Some(local);
                        game.bind_local_player_residency();
                        add(game, 92, "minecraft:block_display", [15.5, 65.5, 3.0]);
                        game.handle_packet(C::SetPassengers(play::SetPassengers {
                            vehicle: 92,
                            passengers: vec![7],
                        }))
                        .unwrap();
                        let near =
                            |game: &Game<'_>| ids(game, [39.5, 60.0, 2.0], [41.5, 70.0, 4.0]);
                        assert!(near(game).is_empty());
                        // No teleport duration: the display snaps, and its rider with it.
                        sync(game, 92, [40.5, 65.5, 3.0]);
                        let seat = game.player.as_ref().unwrap().position;
                        use sunrise_client::join_seq::residency::table::SectionKey;
                        assert_eq!(
                            SectionKey::at(seat.components()),
                            SectionKey::at([40.5, 65.5, 3.0])
                        );
                        assert_eq!(near(game), [92, 7]);
                    }
                    _ => unreachable!("unexpected crossing case"),
                }
            });
        });
    }
}
