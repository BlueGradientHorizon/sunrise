#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, entity::Entity};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::tnt::PrimedTnt;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::tnt::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::entity_rules::EntityRules;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::lan::playerdata::uuid_ints;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result, collisions::SceneCollisions, items::ItemStack, mesher::Vertex, mobs::Who, text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    position::BlockPos,
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::level::flags as level_flags;

impl Game<'_> {
    /// `TntRenderer`: swelling over the last 10 ticks and flashing white every 5 ticks.
    pub(in crate::game) fn tnt_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
        run: std::ops::Range<usize>,
        sprites: Option<[Option<sunrise_client_render::hud::Rect>; 6]>,
    ) {
        let Some(sprites) = sprites else {
            return;
        };
        let range = super::entity_culling::default_range(
            glam::DVec3::splat(PrimedTnt::SIZE),
            self.view_scale(),
        );
        for t in &self.tnts[run] {
            // LevelExtractor: the camera entity is not drawn from its own eyes.
            if self.hides_as_camera(t.id) {
                continue;
            }
            // EntityRenderer.shouldRender.
            if !self.standing_entity_in_view(
                camera,
                super::entity_culling::at(t.previous),
                super::entity_culling::at(t.position),
                alpha,
                PrimedTnt::SIZE,
                PrimedTnt::SIZE,
                range,
                0.0,
            ) {
                continue;
            }
            self.mark_motion(out.len(), crate::game::MOTION_OBJECT | u64::from(t.id));
            let fuse = t.fuse as f32 - alpha as f32 + 1.0;
            if let Some(resolved) = super::native_entity_renderers::tnt_renderer() {
                let partial = alpha as f32;
                let at = glam::DVec3::new(
                    t.previous.x + (t.position.x - t.previous.x) * alpha,
                    t.previous.y + (t.position.y - t.previous.y) * alpha,
                    t.previous.z + (t.position.z - t.previous.z) * alpha,
                );
                let entity: mod_api::EntityRenderState =
                    mod_api::__private::built::EntityRenderState {
                        id: t.id,
                        partial,
                        age: t.ticks as f32 + partial,
                        yaw: t.rotation[0],
                        pitch: t.rotation[1],
                        living: None,
                        tnt_fuse: Some(fuse),
                        orb_value: None,
                        painting: None,
                        item_frame: None,
                        attachments: None,
                    }
                    .into();
                let light = self.world.light(
                    t.position.x.floor() as i32,
                    (t.position.y + 0.5).floor() as i32,
                    t.position.z.floor() as i32,
                );
                let packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);
                let block = [
                    at.x.floor() as i32,
                    at.y.floor() as i32,
                    at.z.floor() as i32,
                ];
                if self.mod_entity_renderer_geometry(
                    &resolved,
                    &entity,
                    "minecraft:tnt",
                    at - camera,
                    Some(at),
                    block,
                    packed,
                    out,
                ) {
                    continue;
                }
            }
            let scale = if fuse < 10.0 {
                let g = (1.0 - fuse / 10.0).clamp(0.0, 1.0);
                1.0 + g * g * g * g * 0.3
            } else {
                1.0
            };
            let lit = fuse >= 0.0 && (fuse / 5.0) as i32 % 2 == 0;
            let base = glam::DVec3::new(
                t.previous.x + (t.position.x - t.previous.x) * alpha,
                t.previous.y + (t.position.y - t.previous.y) * alpha + 0.5,
                t.previous.z + (t.position.z - t.previous.z) * alpha,
            ) - camera;
            let o = base.as_vec3();
            let light = self.world.light(
                t.position.x.floor() as i32,
                (t.position.y + 0.5).floor() as i32,
                t.position.z.floor() as i32,
            );
            let packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);
            let v = |x: f32, y: f32, z: f32| o + glam::Vec3::new(x - 0.5, y - 0.5, z - 0.5) * scale;
            let faces = [
                (
                    [v(0., 1., 1.), v(1., 1., 1.), v(1., 1., 0.), v(0., 1., 0.)],
                    1.0,
                ),
                (
                    [v(0., 0., 0.), v(1., 0., 0.), v(1., 0., 1.), v(0., 0., 1.)],
                    0.5,
                ),
                (
                    [v(1., 0., 0.), v(0., 0., 0.), v(0., 1., 0.), v(1., 1., 0.)],
                    0.8,
                ),
                (
                    [v(0., 0., 1.), v(1., 0., 1.), v(1., 1., 1.), v(0., 1., 1.)],
                    0.8,
                ),
                (
                    [v(0., 0., 0.), v(0., 0., 1.), v(0., 1., 1.), v(0., 1., 0.)],
                    0.6,
                ),
                (
                    [v(1., 0., 1.), v(1., 0., 0.), v(1., 1., 0.), v(1., 1., 1.)],
                    0.6,
                ),
            ];
            for ((corners, shade), sprite) in faces.into_iter().zip(sprites) {
                let Some(r) = sprite else { continue };
                let c = (255.0 * shade) as u32;
                let color = 0xFF00_0000 | c << 16 | c << 8 | c;
                for (p, uv) in
                    corners
                        .iter()
                        .zip([[r.u0, r.v1], [r.u1, r.v1], [r.u1, r.v0], [r.u0, r.v0]])
                {
                    // Overlay 5: OverlayTexture white flash.
                    out.push(Vertex {
                        normal: [0; 4],
                        pos: p.to_array(),
                        uv,
                        layer: if lit { 5 } else { 0 },
                        color,
                        light: packed,

                        glint_uv: [0.0, 0.0],
                    });
                }
            }
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::tnt::tests::on_big_stack;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::tnt::tests::server_game;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::tnt::tests::set_silent;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::tnt::tests::write_silence_output;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::tnt::tests::*;

    pub(in crate::game::client_side) fn particle_count(game: &Game<'_>) -> usize {
        game.particles.0.requested.as_ref().unwrap().len()
    }

    pub(in crate::game) fn run_portal_blast_sulfur_cube_uses_only_positive_cooldown() {
        use sunrise_client::mobs::Mob;
        use sunrise_client::nether_portal::PortalState;
        let mut resources = None;
        let mut game = server_game(&mut resources).expect("set SUNRISE_TEST_RESOURCES");
        let portal = game
            .pack
            .assets
            .registry
            .default_state("minecraft:nether_portal")
            .unwrap();
        for (cooldown, silent) in [
            (1, false),
            (1, true),
            (0, false),
            (0, true),
            (-1, false),
            (-1, true),
        ] {
            game.world.set(8, 64, 8, portal).unwrap();
            let kind = game
                .res
                .mob_types()
                .get("minecraft:sulfur_cube")
                .unwrap()
                .clone();
            let mut cube = Mob::new(9123, kind, Vec3::new(8.5, 64.0, 8.5), 0.0);
            cube.silent = silent;
            cube.extra.set_body(Some("minecraft:tnt".into()));
            // Fuse 1: explodes on this tick.
            cube.extra.sulfur_fuse = sunrise_client::mobs::sulfur_cube::Fuse::set(1);
            game.mobs.push(cube);
            game.portals.entities.insert(
                9123,
                PortalState {
                    cooldown,
                    process: None,
                },
            );
            // The actual level tick orders the portal pass before the mob pass.
            game.audio.0.heard.clear();
            game.handle_entity_portals();
            game.tick_creatures().unwrap();
            assert_eq!(
                game.world.block(8, 64, 8) == portal,
                cooldown > 0,
                "cooldown {cooldown}, Silent={silent}"
            );
            assert_eq!(
                game.audio
                    .0
                    .heard
                    .get("minecraft:entity.generic.explode")
                    .copied()
                    .unwrap_or(0),
                u32::from(!silent),
                "cooldown {cooldown}, Silent={silent}"
            );
        }
        // An unrelated native/sourceless blast never inherits the cube's calculator.
        game.world.set(8, 64, 8, portal).unwrap();
        game.server_explode(
            [8.5, 64.0625, 8.5],
            4.0,
            super::super::explosion::BlockInteraction::Destroy,
            false,
        )
        .unwrap();
        assert_ne!(game.world.block(8, 64, 8), portal);
    }

    pub(in crate::game) fn run_silent_creeper_keeps_the_blast_after_its_source_is_removed() {
        on_big_stack(|| {
            let mut resources = None;
            let Some(mut game) = source_silence_game(&mut resources) else {
                return;
            };
            for silent in [false, true, false] {
                let id = game
                    .spawn_mob(
                        "minecraft:creeper",
                        [8.5, 64.0, 8.5],
                        true,
                        super::super::creatures::SpawnSite::Plain,
                    )
                    .unwrap();
                let mob = game.mobs.iter_mut().find(|m| m.id == id).unwrap();
                mob.silent = silent;
                mob.swell = mob.max_swell;
                mob.extra.flag = true;
                let mut events = Vec::new();
                sunrise_client::mobs::hostile::creeper_tick(mob, &mut events);
                assert!(mob.removed);
                game.mobs.retain(|m| m.id != id);
                game.audio.0.heard.clear();
                let particles = particle_count(&game);
                game.handle_mob_events(events).unwrap();
                assert!(particle_count(&game) > particles);
                assert_eq!(
                    sound_count(&game, "minecraft:entity.generic.explode"),
                    u32::from(!silent)
                );
            }
        });
    }

    pub(in crate::game) fn run_silent_launched_potions_and_bottles_keep_their_impact() {
        on_big_stack(|| {
            let mut resources = None;
            let Some(mut game) = source_silence_game(&mut resources) else {
                return;
            };
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            game.world.set(8, 63, 8, stone).unwrap();
            for kind in ["splash_potion", "lingering_potion", "experience_bottle"] {
                for silent in [false, true, false] {
                    let mut shot = crate::game::potions::thrown_projectile(
                        ItemStack::new(&format!("minecraft:{kind}"), 1)
                            .with_potion("minecraft:swiftness"),
                        Vec3::new(8.5, 64.1, 8.5),
                        false,
                    );
                    shot.velocity = Vec3::new(0.0, -1.0, 0.0);
                    game.add_projectile(shot);
                    let id = game.creatures.projectiles.last().unwrap().id;
                    set_silent(&mut game, id, silent);
                    game.audio.0.heard.clear();
                    let particles = particle_count(&game);
                    let clouds = game.breath_clouds.len();
                    let xp: u32 = game.orbs.iter().map(|o| o.points() * o.count).sum();
                    game.tick_creature_entities().unwrap();
                    assert!(!game.creatures.projectiles.iter().any(|p| p.id == id));
                    assert!(particle_count(&game) > particles, "{kind} keeps its splash");
                    assert_eq!(
                        sound_count(&game, "minecraft:entity.splash_potion.break"),
                        u32::from(!silent),
                        "{kind} Silent={silent}"
                    );
                    assert_eq!(
                        game.breath_clouds.len() - clouds,
                        usize::from(kind == "lingering_potion")
                    );
                    let gained = game.orbs.iter().map(|o| o.points() * o.count).sum::<u32>() - xp;
                    if kind == "experience_bottle" {
                        assert!((3..=11).contains(&gained), "silence keeps the experience");
                    }
                }
            }
        });
    }

    pub(in crate::game) fn run_silent_potions_keep_the_splash_without_its_sound() {
        on_big_stack(|| {
            use sunrise_client::projectiles::{Hit, Projectile, ProjectileKind};
            let mut resources = None;
            let Some(mut game) = source_silence_game(&mut resources) else {
                return;
            };
            let mut rows = Vec::new();
            for kind in ["splash_potion", "lingering_potion"] {
                for potion in ["awkward", "swiftness", "healing"] {
                    for silent in [false, true] {
                        game.audio.0.heard.clear();
                        let stack = ItemStack::new(&format!("minecraft:{kind}"), 1)
                            .with_potion(&format!("minecraft:{potion}"));
                        let mut projectile = Projectile::new(
                            ProjectileKind::potion(stack),
                            Vec3::new(8.5, 64.0, 8.5),
                            None,
                        );
                        projectile.id = game.take_entity_id();
                        set_silent(&mut game, projectile.id, silent);
                        let before = particle_count(&game);
                        let clouds = game.breath_clouds.len();
                        game.projectile_hit(
                            &projectile,
                            Hit::Block {
                                pos: [8, 63, 8],
                                at: projectile.position,
                                face: [0, 1, 0],
                            },
                        )
                        .unwrap();
                        rows.push(format!(
                            "{kind} {potion} {silent} {} {}",
                            particle_count(&game) > before,
                            sound_count(&game, "minecraft:entity.splash_potion.break") != 0
                        ));
                        assert!(
                            particle_count(&game) > before,
                            "{kind} {potion} Silent={silent}: visible splash"
                        );
                        assert_eq!(
                            sound_count(&game, "minecraft:entity.splash_potion.break"),
                            u32::from(!silent),
                            "{kind} {potion} Silent={silent}"
                        );
                        assert_eq!(
                            game.breath_clouds.len() - clouds,
                            usize::from(kind == "lingering_potion" && potion != "awkward"),
                            "silence leaves the lingering cloud intact"
                        );
                    }
                }
            }
            write_silence_output("potions", &rows);
        });
    }

    pub(in crate::game) fn run_silent_tnt_keeps_the_blast_without_its_sound() {
        on_big_stack(|| {
            let mut resources = None;
            let Some(mut game) = source_silence_game(&mut resources) else {
                return;
            };
            let mut rows = Vec::new();
            for silent in [false, true, false] {
                let glass = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:glass")
                    .unwrap();
                game.world.set(8, 64, 8, glass).unwrap();
                game.audio.0.heard.clear();
                game.add_primed_tnt(BlockPos::new(8, 64, 8), 0.0, 1, None, None);
                game.tnts[0].no_gravity = true;
                game.tnts[0].velocity = Vec3::ZERO;
                let id = game.tnts[0].id;
                set_silent(&mut game, id, silent);
                let before = particle_count(&game);
                game.tick_one_tnt(0, &[]).unwrap();
                rows.push(format!(
                    "explosion {silent} {} {}",
                    particle_count(&game) > before,
                    sound_count(&game, "minecraft:entity.generic.explode") != 0
                ));
                assert!(game.tnts.is_empty(), "the fuse still expires");
                assert!(
                    game.res.facts().is_air(game.world.block(8, 64, 8)),
                    "silence keeps block destruction"
                );
                assert!(particle_count(&game) > before, "the blast remains visible");
                assert_eq!(
                    sound_count(&game, "minecraft:entity.generic.explode"),
                    u32::from(!silent),
                    "direct source Silent={silent}"
                );
                game.audio.0.heard.clear();
                game.server_explode(
                    [8.5, 64.0, 8.5],
                    0.0,
                    super::super::explosion::BlockInteraction::Keep,
                    false,
                )
                .unwrap();
                assert_eq!(
                    game.audio.0.heard.get("minecraft:entity.generic.explode"),
                    Some(&1),
                    "a source-free blast stays audible"
                );
            }
            write_silence_output("explosion", &rows);
        });
    }

    pub(in crate::game::client_side) fn sound_count(game: &Game<'_>, event: &str) -> u32 {
        game.audio.0.heard.get(event).copied().unwrap_or(0)
    }

    pub(in crate::game) fn source_silence_game(
        resources: &mut Option<std::sync::Arc<crate::game::Resources>>,
    ) -> Option<Box<Game<'_>>> {
        let mut game = server_game(resources)?;
        // Observe the server's real emission requests without a camera or render tick.
        game.particles.0.requested = Some(Vec::new());
        Some(game)
    }
}
