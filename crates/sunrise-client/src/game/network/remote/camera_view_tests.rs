//! #3948: the shader-pack uniforms tied to the view read the camera entity (the entity a
//! spectator's click or the server's `SetCamera` attached the view to), and the client keeps the
//! other living entities' effects for it (`ClientPacketListener.handleUpdateMobEffect`).
use crate::game::{Game, native_entity_refs::tests::with_client_game};
use std::sync::Arc;
use sunrise_client::{
    camera::CameraType, chunk::Chunk, gfx::pack_uniforms::ViewEntity, level_loading::Reason,
    motion::PlayerMotion, options_file::V,
};
use sunrise_core::geometry::Vec3;
use sunrise_protocol::{
    connection::Pipe,
    packets::play::{self, Clientbound as C, SpawnInfo},
};

/// The local player's entity id.
const OWN: i32 = 7;
/// A zombie, living.
const ZOMBIE: i32 = 31;
/// An oak boat, not living.
const BOAT: i32 = 40;

fn spawn(dimension: &str) -> SpawnInfo {
    SpawnInfo {
        dimension_type: 0,
        dimension: dimension.into(),
        seed: 0,
        game_type: 3,
        previous_game_type: None,
        debug: false,
        flat: true,
        last_death: None,
        portal_cooldown: 0,
        sea_level: 63,
    }
}

/// A spectator client in the overworld, holding a zombie at (10, 64, 12) and an oak boat.
fn with_spectator(test: impl FnOnce(&mut Game<'_>)) {
    with_client_game(|game| {
        let (client, _server) = Pipe::pair();
        game.connect_local(client, "CameraViewTest").unwrap();
        game.enter_remote_dimension(&spawn("minecraft:overworld"), false, Reason::Other);
        let remote = game.remote.as_mut().unwrap();
        remote.logged_in = true;
        remote.entity_id = OWN;
        game.player = Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap());
        add(game, ZOMBIE, "minecraft:zombie", [10.0, 64.0, 12.0]);
        add(game, BOAT, "minecraft:oak_boat", [-6.5, 64.0, 3.5]);
        assert_eq!(game.mobs.len(), 1);
        assert_eq!(game.boats.len(), 1);
        test(game);
    });
}

fn add(game: &mut Game<'_>, id: i32, kind: &str, position: [f64; 3]) {
    let kind = registry_id(game, "minecraft:entity_type", kind);
    game.handle_packet(C::AddEntity(play::AddEntity {
        id,
        uuid: id as u128,
        kind,
        position,
        velocity: [0.0; 3],
        pitch: 0,
        yaw: 0,
        head_yaw: 0,
        data: 0,
    }))
    .unwrap();
}

fn registry_id(game: &Game<'_>, registry: &str, name: &str) -> i32 {
    let remote = game.remote.as_ref().unwrap();
    remote.builtin.get(registry).unwrap().id(name).unwrap()
}

fn update(game: &mut Game<'_>, id: i32, effect: &str, duration: i32, flags: i8) {
    let effect = registry_id(game, "minecraft:mob_effect", effect);
    game.handle_packet(C::UpdateMobEffect(play::UpdateMobEffect {
        id,
        effect,
        amplifier: 0,
        duration,
        flags,
    }))
    .unwrap();
}

fn remove(game: &mut Game<'_>, id: i32, effect: &str) {
    let effect = registry_id(game, "minecraft:mob_effect", effect);
    game.handle_packet(C::RemoveMobEffect(play::RemoveMobEffect { id, effect }))
        .unwrap();
}

fn set_camera(game: &mut Game<'_>, camera_id: i32) {
    game.handle_packet(C::SetCamera(play::SetCamera { camera_id }))
        .unwrap();
}

/// The view's values but its light, read at the partial tick 0.25.
fn view(game: &Game<'_>) -> ViewEntity {
    ViewEntity {
        eye_light: [0; 2],
        ..game.pack_view_entity(glam::DVec3::new(0.5, 65.62, 0.5), 0.25)
    }
}

fn kept(game: &Game<'_>, id: i32) -> Option<Vec<(&'static str, i32)>> {
    let r = game.remote.as_ref().unwrap();
    let fx = r.entity_effects.get(&id)?;
    Some(fx.effects.list.iter().map(|e| (e.id, e.duration)).collect())
}

const PLAYER_VIEW: ViewEntity = ViewEntity {
    living: true,
    blindness: 1.0,
    night_vision: 0.0,
    darkness: 0.0,
    mood: 0.375,
    eye_light: [0; 2],
};

/// While the view is on a spectated zombie, `blindness`, `nightVision` and `darknessFactor`
/// read the effects the server sent for it, counting down and fading on its ticks, and the mood
/// is 0; on the player's own view they are the player's, and the lightmap keeps the player's
/// all along.
#[test]
#[ignore = "daily"]
fn the_view_reads_the_spectated_entitys_effects() {
    with_spectator(|game| {
        game.ambience.moodiness = 0.375;
        game.settings
            .file
            .set("darknessEffectScale", V::Double(0.5));
        update(game, OWN, "minecraft:blindness", 100, 0);
        // Not blended (no flag 8): darkness is at its target at once.
        update(game, ZOMBIE, "minecraft:night_vision", 201, 0);
        update(game, ZOMBIE, "minecraft:darkness", 600, 0);
        assert_eq!(view(game), PLAYER_VIEW, "the player is the camera");

        set_camera(game, ZOMBIE);
        let zombie = ViewEntity {
            living: true,
            blindness: 0.0,
            night_vision: 1.0,
            darkness: 0.5,
            mood: 0.0,
            eye_light: [0; 2],
        };
        assert_eq!(view(game), zombie);
        // The lightmap's night vision and darkness stay the player's (none).
        assert_eq!(game.night_vision_intensity(0.25), 0.0);
        assert_eq!(game.darkness_lighting(0.25).0, 0.0);

        // GameRenderer.getNightVisionScale once 200 ticks are left: it flickers.
        game.tick_remote_entities(true);
        let flicker = 0.7 + ((200.0f32 - 0.25) * std::f32::consts::PI * 0.2).sin() * 0.3;
        assert_eq!(view(game).night_vision, flicker);
        // A frozen level ticks no mob: nothing counts down.
        game.tick_remote_entities(false);
        assert_eq!(view(game).night_vision, flicker);

        update(game, ZOMBIE, "minecraft:blindness", 40, 0);
        assert_eq!(view(game).blindness, 1.0);
        remove(game, ZOMBIE, "minecraft:blindness");
        assert_eq!(view(game).blindness, 0.0);
        // Darkness fades out over its next ticks: 1 to 0 at a quarter of the way, times the
        // option's 0.5, then gone.
        remove(game, ZOMBIE, "minecraft:darkness");
        assert_eq!(view(game).darkness, 0.5);
        game.tick_remote_entities(true);
        assert_eq!(view(game).darkness, 0.375);
        game.tick_remote_entities(true);
        assert_eq!(view(game).darkness, 0.0);

        set_camera(game, OWN);
        assert_eq!(view(game), PLAYER_VIEW, "back on the player");
    });
}

/// An entity that isn't living has no effects to read and looks nowhere, and a living one with
/// none reads none; neither has the player's mood or blindness.
#[test]
#[ignore = "daily"]
fn a_view_on_another_entity_has_none_of_the_players_values() {
    with_spectator(|game| {
        game.ambience.moodiness = 0.375;
        update(game, OWN, "minecraft:blindness", 100, 0);
        // Not kept: a boat is not a LivingEntity.
        update(game, BOAT, "minecraft:blindness", 100, 0);
        assert_eq!(kept(game, BOAT), None);
        set_camera(game, BOAT);
        let none = ViewEntity {
            living: false,
            blindness: 0.0,
            night_vision: 0.0,
            darkness: 0.0,
            mood: 0.0,
            eye_light: [0; 2],
        };
        assert_eq!(view(game), none);
        set_camera(game, ZOMBIE);
        assert_eq!(
            view(game),
            ViewEntity {
                living: true,
                ..none
            }
        );
        // The boat leaves while it is the camera: the view is the player's again.
        set_camera(game, BOAT);
        game.handle_packet(C::RemoveEntities(play::RemoveEntities { ids: vec![BOAT] }))
            .unwrap();
        assert_eq!(view(game), PLAYER_VIEW);
    });
}

/// The other entities' effects are kept one of a kind, for living entities the level holds,
/// and go with the entity: when it is removed, added again under its id, or the level is
/// replaced; a respawn in the same level keeps them.
#[test]
#[ignore = "daily"]
fn other_entities_effects_go_with_the_entity() {
    with_spectator(|game| {
        // Not held, or not an effect the registry names: nothing.
        update(game, 99, "minecraft:speed", 100, 0);
        assert_eq!(kept(game, 99), None);
        game.handle_packet(C::UpdateMobEffect(play::UpdateMobEffect {
            id: ZOMBIE,
            effect: 9_999,
            amplifier: 0,
            duration: 100,
            flags: 0,
        }))
        .unwrap();
        assert_eq!(kept(game, ZOMBIE), None);
        // forceAddEffect: the second of a kind replaces the first.
        update(game, ZOMBIE, "minecraft:speed", 100, 0);
        update(game, ZOMBIE, "minecraft:speed", 30, 0);
        update(game, ZOMBIE, "minecraft:glowing", -1, 0);
        assert_eq!(
            kept(game, ZOMBIE),
            Some(vec![("speed", 30), ("glowing", -1)])
        );
        // The player's own effects are not the zombie's.
        assert!(game.potions.effects.list.is_empty());
        remove(game, ZOMBIE, "minecraft:speed");
        assert_eq!(kept(game, ZOMBIE), Some(vec![("glowing", -1)]));

        game.handle_packet(C::RemoveEntities(play::RemoveEntities {
            ids: vec![ZOMBIE],
        }))
        .unwrap();
        assert_eq!(kept(game, ZOMBIE), None);

        add(game, ZOMBIE, "minecraft:zombie", [10.0, 64.0, 12.0]);
        update(game, ZOMBIE, "minecraft:speed", 100, 0);
        add(game, ZOMBIE, "minecraft:zombie", [10.0, 64.0, 12.0]);
        assert_eq!(
            kept(game, ZOMBIE),
            None,
            "ClientLevel.addEntity removes it first"
        );

        // An armor stand is living.
        add(game, 50, "minecraft:armor_stand", [2.0, 64.0, 2.0]);
        update(game, 50, "minecraft:slowness", 100, 0);
        assert_eq!(kept(game, 50), Some(vec![("slowness", 100)]));

        update(game, ZOMBIE, "minecraft:speed", 100, 0);
        game.handle_packet(C::Respawn(play::Respawn {
            spawn: spawn("minecraft:overworld"),
            keep: 0,
        }))
        .unwrap();
        assert_eq!(kept(game, ZOMBIE), Some(vec![("speed", 100)]));
        game.handle_packet(C::Respawn(play::Respawn {
            spawn: spawn("minecraft:the_nether"),
            keep: 0,
        }))
        .unwrap();
        assert!(game.remote.as_ref().unwrap().entity_effects.is_empty());
    });
}

/// `eyeBrightness` reads the light at the camera entity's eye, where a third-person camera
/// backs off from: not at the camera's own block.
#[test]
#[ignore = "daily"]
fn the_view_light_is_at_the_camera_entitys_eye() {
    with_spectator(|game| {
        set_camera(game, ZOMBIE);
        game.apply_camera_type(CameraType::ThirdPersonBack);
        let (camera, _, _) = game.camera_view(0.0).unwrap();
        let eye = glam::DVec3::from_array(game.view.eye);
        let (at_eye, at_camera) = (eye.floor().as_ivec3(), camera.floor().as_ivec3());
        assert_ne!(at_eye, at_camera);
        // Sky 5, block 12 at the eye; sky 3, block 0 at the camera.
        set_light(game, at_eye, 0x5c);
        set_light(game, at_camera, 0x30);
        assert_eq!(game.pack_view_entity(eye, 0.0).eye_light, [12, 5]);
        assert_eq!(game.pack_view_entity(camera, 0.0).eye_light, [0, 3]);
    });
}

/// Puts `light` (`sky << 4 | block`) at one block, every other block of its section dark.
fn set_light(game: &mut Game<'_>, at: glam::IVec3, light: u8) {
    let key = (at.x >> 4, at.z >> 4);
    let layout = game.world.layout();
    if game.world.chunks.get(&key).is_none() {
        let chunk = Chunk::with_layout(key.0, key.1, layout);
        game.world.chunks.insert(key, Arc::new(chunk));
    }
    let chunk = Arc::make_mut(game.world.chunks.get_mut(&key).unwrap());
    let s = chunk.section_index(at.y).unwrap();
    let section = Arc::make_mut(&mut chunk.sections[s]);
    let mut all = [0u8; 4096];
    for (i, v) in all.iter_mut().enumerate() {
        *v = section.light(i);
    }
    all[sunrise_client::chunk::index(at.x, at.y, at.z)] = light;
    section.light = sunrise_client::chunk::Light::from_slice(&all);
}
