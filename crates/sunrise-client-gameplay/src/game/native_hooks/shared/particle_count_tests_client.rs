#[allow(unused_imports)]
pub(crate) use super::super::pickups::{Collector, Taken};
#[allow(unused_imports)]
pub(crate) use crate::game::native_hooks::shared::particle_count_tests::ClearAt;
#[allow(unused_imports)]
pub(crate) use crate::game::native_hooks::shared::particle_count_tests::ClearInGui;
#[allow(unused_imports)]
pub(crate) use crate::game::native_hooks::shared::particle_count_tests::ClearInModel;
#[allow(unused_imports)]
pub(crate) use crate::game::native_hooks::shared::particle_count_tests::FrameGeometry;
#[allow(unused_imports)]
pub(crate) use crate::game::native_hooks::shared::particle_count_tests::FrameMod;
#[allow(unused_imports)]
pub(crate) use crate::game::native_hooks::shared::particle_count_tests::frame_geometry;
#[allow(unused_imports)]
pub(crate) use crate::game::native_hooks::shared::particle_count_tests::host_clear;
#[allow(unused_imports)]
pub(crate) use crate::game::native_hooks::shared::particle_count_tests::orb;
#[allow(unused_imports)]
pub(crate) use crate::game::native_hooks::shared::particle_count_tests::with_frame_scene;
#[allow(unused_imports)]
pub(crate) use crate::game::native_hooks::shared::particle_count_tests::*;
#[allow(unused_imports)]
pub(crate) use mod_api::ClientHost;
#[allow(unused_imports)]
pub(crate) use std::time::Duration;
#[allow(unused_imports)]
pub(crate) use sunrise_client::particles::FireworkExplosion;
#[allow(unused_imports)]
pub(crate) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(crate) use sunrise_level::particle_options::{
    ParticleOptions, ParticleType,
};
#[allow(unused_imports)]
pub(crate) use sunrise_sim::entities::ExperienceOrb;

/// `particle_count` and `clear_particles` through the client host. The count covers every
/// live group. A clear reads 0 at once and empties every group, its queue and emitters
/// before the next frame is drawn, with no tick in between (a paused or frozen game),
/// or at the next particle tick.
#[test]
#[ignore = "daily"]
fn a_client_mod_counts_and_clears_particles() {
    super::super::native_entity_refs::tests::with_client_game(|game| {
        let host: &dyn ClientHost = game;
        assert_eq!(host.particle_count(), 0);
        fill(game);
        assert_eq!(game.client.pickups.0.live.len(), 1, "the pickup is live");
        let engine = game.client.particles.0.group_count();
        assert_eq!(engine, 5, "three flames, one starter and one guardian");
        let host: &dyn ClientHost = game;
        assert_eq!(host.particle_count() as usize, engine + 1);
        // A clear asked between frames: it reads 0 at once, and the next frame clears
        // every group without a particle tick.
        game.client.particles.0.tracking_emitter(
            ParticleOptions::simple(ParticleType::Crit),
            sunrise_client::particles::TrackingTarget::Entity(0),
            sunrise_client::particles::TrackingBody {
                feet: [0.0; 3],
                width: 1.0,
                height: 1.0,
            },
        );
        assert!(host_clear(game));
        let host: &dyn ClientHost = game;
        assert_eq!(host.particle_count(), 0);
        game.extras_frame(Duration::ZERO);
        assert_eq!(game.client.particles.0.group_count(), 0);
        assert!(game.client.pickups.0.live.is_empty());
        assert!(game.client.particles.0.elder_guardians().is_empty());
        // The emitter and the queue went too: a tick brings nothing back.
        game.tick_particles();
        assert_eq!(game.client_particle_count(), 0);
        // A clear asked before a particle tick lands at that tick.
        fill(game);
        assert!(host_clear(game));
        game.tick_particles();
        assert_eq!(game.client.particles.0.group_count(), 0);
        assert!(game.client.pickups.0.live.is_empty());
        let host: &dyn ClientHost = game;
        assert_eq!(host.particle_count(), 0);
    });
}

#[test]
#[ignore = "daily"]
fn a_model_callback_clear_discards_particle_geometry_already_extracted() {
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "clear_model",
        Box::new(ClearInModel(calls.clone())),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        super::super::native_entity_refs::tests::with_client_game(|game| {
            game.options.execution_role = super::ExecutionRole::Client;
            fill(game);
            let camera = glam::DVec3::from_array(game.client.particles.0.camera);
            let mut out = Vec::new();
            game.client_particle_geometry(camera, 0.0, 70.0, None, &mut out);
            let previous = out.len();
            assert!(previous > 0);
            let positions: Vec<_> = out.iter().map(|v| v.pos).collect();
            let motion = game.client.entity_motion.borrow().clone();
            // An actual picked-up item resolves a conditional model while the
            // preceding ordinary-particle pass has already produced geometry.
            let renderer =
                std::sync::Arc::get_mut(&mut game.client.client_pack.item_renderer).unwrap();
            std::sync::Arc::make_mut(&mut renderer.models)
                .definitions
                .insert(
                    "minecraft:stick".into(),
                    sunrise_assets::item_model::ClientItem::from_json(
                        r#"{"model":{
                    "type":"minecraft:condition", "property":"test:clear",
                    "on_true":{"type":"minecraft:model","model":"minecraft:item/stick"},
                    "on_false":{"type":"minecraft:model","model":"minecraft:item/stick"}
                }}"#,
                    )
                    .unwrap(),
                );
            game.pack.item_models = std::sync::Arc::clone(&renderer.models);
            let at = game.client.particles.0.camera;
            let item = sunrise_sim::entities::ItemEntity::new(
                1,
                sunrise_items::items::ItemStack::new("minecraft:stick", 1),
                Vec3::new(at[0], at[1], at[2]),
                Vec3::ZERO,
                0.0,
            );
            game.add_pickup_toward(Taken::Item(Box::new(item)), Collector::Player, at);
            game.tick_particles();
            game.client_particle_geometry(camera, 0.0, 70.0, None, &mut out);
            assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);
            assert_eq!(out.iter().map(|v| v.pos).collect::<Vec<_>>(), positions);
            assert_eq!(&game.client.entity_motion.borrow()[..motion.len()], &motion);
            assert_eq!(game.client_particle_count(), 0);
            game.tick_particles();
            assert_eq!(game.client_particle_count(), 0);
        });
    });
}

pub(in crate::game) fn add_frame_particles(game: &mut crate::game::ClientGame<'_>) {
    fill(game);
    for _ in 0..20 {
        game.client.particles.0.add(
            &ParticleOptions::simple(ParticleType::Flame),
            game.client.particles.0.camera,
            [0.0; 3],
        );
    }
    let at = game.client.particles.0.camera;
    let item = sunrise_sim::entities::ItemEntity::new(
        1,
        sunrise_items::items::ItemStack::new("minecraft:stick", 1),
        Vec3::new(at[0], at[1], at[2]),
        Vec3::ZERO,
        0.0,
    );
    game.add_pickup_toward(Taken::Item(Box::new(item)), Collector::Player, at);
    game.tick_particles();
}

/// Fills every particle group Sunrise keeps apart: three flames, a multi-burst firework
/// starter (`NO_RENDER`), an item pickup (`ITEM_PICKUP`) and the elder guardian
/// apparition (`ELDER_GUARDIANS`), each live after one particle tick.
pub fn fill(game: &mut crate::game::ClientGame<'_>) {
    let at = game.client.particles.0.camera;
    for _ in 0..3 {
        game.client
            .particles
            .0
            .add(&ParticleOptions::simple(ParticleType::Flame), at, [0.0; 3]);
    }
    let bursts = vec![FireworkExplosion::default(); 4];
    game.client
        .particles
        .0
        .fireworks(at, [0.0; 3], bursts, at, true);
    let v = Vec3::new(at[0], at[1], at[2]);
    game.add_pickup_toward(Taken::Orb(Box::new(orb(v))), Collector::Player, at);
    game.client.particles.0.elder_guardian();
    game.tick_particles();
}

/// A real offscreen renderer and a world for it to draw; the event loop that returns
/// holds the window.
pub(in crate::game) fn frame_scene(
    game: &mut crate::game::ClientGame<'_>,
) -> winit::event_loop::EventLoop<()> {
    let mut builder = winit::event_loop::EventLoop::builder();
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_x11().with_any_thread(true);
    }
    let event_loop = builder.build().unwrap();
    #[allow(deprecated)]
    let window = std::sync::Arc::new(
        event_loop
            .create_window(
                winit::window::Window::default_attributes()
                    .with_visible(false)
                    .with_inner_size(winit::dpi::PhysicalSize::new(320, 240)),
            )
            .unwrap(),
    );
    game.common.options.offscreen = true;
    game.client.renderer.0 = Some(game.new_renderer(window).unwrap());
    game.client.title.0 = None;
    game.client.settings.extras.dynamic_lights = true;
    game.client.settings.sunrise.particle_culling =
        sunrise_client::graphics::ParticleCullingMode::Off;
    let mut chunk = sunrise_level::chunk::Chunk::empty(0, 0);
    chunk.lit = true;
    game.common.world.chunks.insert((0, 0), std::sync::Arc::new(chunk));
    let mut player = sunrise_sim::motion::PlayerMotion::new(game.common.motion, Vec3::ZERO).unwrap();
    player.position = Vec3::new(0.5, 64.0, 0.5);
    game.common.previous_position = player.position;
    game.common.player = Some(player);
    game.client.particles.0.camera = [0.5, 65.5, 1.5];
    // Ordinary (non-particle) entity geometry and an unrelated item light.
    game.common.dropped.push(sunrise_sim::entities::ItemEntity::new(
        2,
        sunrise_items::items::ItemStack::new("minecraft:torch", 1),
        Vec3::new(0.5, 65.0, 4.5),
        Vec3::ZERO,
        0.0,
    ));
    // Only the picked-up stick resolves this custom property. The torch's
    // ordinary model must survive the clear unchanged.
    let renderer = std::sync::Arc::get_mut(&mut game.client.client_pack.item_renderer).unwrap();
    std::sync::Arc::make_mut(&mut renderer.models)
        .definitions
        .insert(
            "minecraft:stick".into(),
            sunrise_assets::item_model::ClientItem::from_json(
                r#"{"model":{
            "type":"minecraft:condition", "property":"test:clear",
            "on_true":{"type":"minecraft:model","model":"minecraft:item/stick"},
            "on_false":{"type":"minecraft:model","model":"minecraft:item/stick"}
        }}"#,
            )
            .unwrap(),
        );
    game.common.pack.item_models = std::sync::Arc::clone(&renderer.models);
    event_loop
}

#[test]
#[ignore = "daily"]
fn late_clear_removes_native_particle_lights_at_the_frame_consumer() {
    let name = "game::client_side::native_hooks_client::particle_count_tests::late_clear_removes_native_particle_lights_at_the_frame_consumer";
    if super::super::native_entity_refs::tests::run_fresh_cases(
        name,
        "SUNRISE_PARTICLE_LIGHT_CHILD",
        &["gui", "model"],
    ) {
        return;
    }
    let clear_at = if std::env::var("SUNRISE_PARTICLE_LIGHT_CHILD").unwrap() == "gui" {
        ClearAt::Gui
    } else {
        ClearAt::Model
    };
    with_frame_scene(|game, at, calls| {
        let manifest = br#"schema = 1
id = "particle_frame"
name = "Particle Frame"
version = "1.0.0"
api = "0.1"
[material]
file = "material.wgsl"
[[particle_lights]]
particles = ["flame"]
color = [1.0, 0.0, 0.0]
reach = 4.0
[[lights]]
blocks = ["minecraft:torch"]
color = [0.0, 1.0, 0.0]
"#;
        let pack = sunrise_shader_api::Pack::from_files(std::collections::BTreeMap::from([
            (sunrise_shader_api::MANIFEST.into(), manifest.to_vec()),
            (
                "material.wgsl".into(),
                b"fn material(input: MaterialInput) -> Surface { return Surface(input.albedo); }"
                    .to_vec(),
            ),
        ]))
        .unwrap();
        let report = sunrise_shader_api::check(&pack).unwrap();
        game.client
            .renderer
            .0
            .as_mut()
            .unwrap()
            .set_native_shader_pack(Some((
                report,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Default::default(),
            )))
            .unwrap();
        let mut baseline = None;
        game.render_with(|_, frame| {
            assert_eq!(frame.colored_lights.len(), 2, "item and mod light");
            assert!(
                frame
                    .colored_lights
                    .iter()
                    .any(|l| l[1] == [0.0, 1.0, 0.0, 1.0])
            );
            assert!(
                frame
                    .colored_lights
                    .iter()
                    .any(|l| l[1] == [0.0, 0.0, 1.0, 1.0])
            );
            baseline = Some((frame_geometry(frame), frame.colored_lights.to_vec()));
            Ok(true)
        })
        .unwrap();
        let baseline = baseline.unwrap();
        add_frame_particles(game);
        game.render_with(|_, frame| {
            assert_eq!(
                frame.colored_lights.len(),
                sunrise_client::dynamic_light::MAX
            );
            assert!(
                frame
                    .colored_lights
                    .iter()
                    .all(|l| l[1] == [1.0, 0.0, 0.0, 1.0]),
                "nearby flames displace unrelated lights before clearing"
            );
            Ok(true)
        })
        .unwrap();
        *at.lock().unwrap() = clear_at;
        game.render_with(|_, frame| {
            assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);
            assert_eq!(
                frame.colored_lights, baseline.1,
                "cleared flames contribute no light; item/mod lights restored"
            );
            assert_eq!(
                frame_geometry(frame),
                baseline.0,
                "particle tail rolls back, ordinary torch stays"
            );
            Ok(true)
        })
        .unwrap();
        assert_eq!(game.client_particle_count(), 0);
    });
}

#[test]
#[ignore = "daily"]
fn late_gui_clear_prevents_same_frame_particle_geometry_without_a_tick() {
    let mods: mod_api::Mods =
        vec![mod_api::ModEntry::new("clear_gui", Box::new(ClearInGui))].into();
    mod_platform::testing::with_mods(mods, || {
        super::super::native_entity_refs::tests::with_client_game(|game| {
            game.options.execution_role = super::ExecutionRole::Client;
            fill(game);
            game.extras_frame(Duration::ZERO);
            let camera = glam::DVec3::from_array(game.client.particles.0.camera);
            let mut before = Vec::new();
            game.client_particle_geometry(camera, 0.0, 70.0, None, &mut before);
            assert!(
                !before.is_empty(),
                "live guardian, pickup and ordinary particles draw"
            );
            game.native_render_gui_pre(&mut false);
            assert_eq!(mod_api::prelude::EffectsClientHost::particle_count(game), 0);
            let (mut after, _, _, _) =
                game.entity_geometry(camera, 0.0, None, Vec::new(), &mut Vec::new());
            game.client_particle_geometry(camera, 0.0, 70.0, None, &mut after);
            assert!(
                after.is_empty(),
                "no stale pickup, guardian or ordinary quads"
            );
            assert_eq!(game.client_particle_count(), 0);
            game.tick_particles();
            assert_eq!(game.client_particle_count(), 0);
        });
    });
}
