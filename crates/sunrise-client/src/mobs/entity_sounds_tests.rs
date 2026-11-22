use super::*;
use crate::mobs::{Damage, DamageKind, Mob, MobEvent, MobTypes, MobWorld};
use mod_api::Side;
use std::{path::PathBuf, sync::Arc};
use sunrise_core::geometry::Vec3;
use sunrise_mod_package::{Package, content::Content, enabled::EnabledMods};

fn fixture(template: &str, sounds: serde_json::Value, run: impl FnOnce(&MobWorld, &MobTypes)) {
    let base = std::env::var_os("SUNRISE_TEST_RESOURCES")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.local/sunrise-client/26.3")
        });
    if !base.join("entity-types.json").exists() {
        assert!(
            std::env::var_os("SUNRISE_TEST_RESOURCES").is_none(),
            "missing requested resources"
        );
        eprintln!("skipped: no prepared resources for entity sounds");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bird.smod");
    let manifest = "id = \"bird\"\nname = \"Bird\"\nversion = \"1.0\"\napi = \"0.2\"\nside = \"both\"\ncapabilities = [\"registries\"]\n";
    let declaration = serde_json::json!({"template":template,"sounds":sounds}).to_string();
    std::fs::write(
        &path,
        Package::write_files(
            manifest,
            &[(
                "data/bird/sunrise/entity_type/vulture.json",
                declaration.as_bytes(),
            )],
        )
        .unwrap(),
    )
    .unwrap();
    let package = Package::read(&path).unwrap();
    let mods = [crate::mod_content::ContentMod {
        id: "bird".into(),
        content: Content::read(&path, &package.manifest).unwrap(),
        jars: vec![],
    }];
    let inventory = EnabledMods::new([("bird".into(), Side::Both)]);
    let root = crate::mod_content::resource_root(&base, dir.path(), &mods, dir.path(), &inventory)
        .unwrap();
    assert!(crate::mobs::world::with_resource_test_world(
        &root,
        &[],
        true,
        true,
        |world, registry| {
            world
                .chunks
                .insert((0, 0), Arc::new(crate::chunk::Chunk::empty(0, 0)));
            world
                .set(0, 0, 0, registry.default_state("minecraft:stone").unwrap())
                .unwrap();
        },
        run
    ));
}
fn mob(types: &MobTypes, id: &str) -> Mob {
    let mut mob = Mob::new(
        1,
        Arc::new(types.types[id].clone()),
        Vec3::new(0.5, 1.0, 0.5),
        0.0,
    );
    mob.ground.on_ground = true;
    mob.ground.supporting_block = Some(sunrise_core::position::BlockPos::new(0, 0, 0));
    mob
}
// Use the production audio capture consumed by server packet delivery, then replay
// the captured ids through seeded client audio. Muted slots reach neither sink.
fn ids(events: &[MobEvent]) -> Vec<String> {
    let source = sunrise_assets::resource_source::ResourceSource::directory(PathBuf::from(
        "/nonexistent/sunrise-content-entity-sounds",
    ));
    let mut server = crate::audio::Audio::silent(&source);
    let mut remote = crate::audio::Audio::silent(&source);
    server.capture = Some(vec![]);
    for event in events {
        match event {
            MobEvent::Sound(id, at, volume, pitch) => {
                server.play(&full_id(id), Some(*at), *volume, *pitch)
            }
            MobEvent::SoundId(id, at, volume, pitch) => server.play(id, Some(*at), *volume, *pitch),
            _ => {}
        }
    }
    let captured = server.capture.take().unwrap();
    for sound in &captured {
        remote.play_seeded(
            &sound.event,
            Some(sound.position),
            sound.volume,
            sound.pitch,
            42,
        );
    }
    let ids: Vec<_> = captured.into_iter().map(|sound| sound.event).collect();
    assert_eq!(remote.heard.values().sum::<u32>() as usize, ids.len());
    assert_eq!(server.heard, remote.heard);
    assert!(ids.iter().all(|id| !id.contains("minecraft:minecraft:")));
    ids
}
fn ambient(m: &mut Mob, w: &MobWorld) -> Vec<String> {
    let mut events = vec![];
    crate::mobs::ai::ambient_sound(m, w, &mut events);
    ids(&events)
}
fn steps(m: &mut Mob, w: &MobWorld) -> Vec<String> {
    let mut events = vec![];
    m.movement_emission(w, Vec3::new(2.0, 0.0, 0.0), &mut events);
    ids(&events)
}
fn hit(m: &mut Mob, amount: f32) -> Vec<String> {
    let mut fixture = crate::health::TestLevel::new(0);
    assert!(m.damage(
        &mut fixture.level(),
        amount,
        Damage::of(DamageKind::Generic)
    ));
    ids(&std::mem::take(&mut m.outbox))
}

#[test]
fn entity_sounds_inherit_template_ambient_and_stone_footsteps() {
    fixture(
        "minecraft:wandering_trader",
        serde_json::json!({}),
        |w, types| {
            let mut m = mob(types, "bird:vulture");
            assert_eq!(
                ambient(&mut m, w),
                ["minecraft:entity.wandering_trader.ambient"]
            );
        },
    );
    fixture("minecraft:villager", serde_json::json!({}), |w, types| {
        let mut template = mob(types, "minecraft:villager");
        let mut content = mob(types, "bird:vulture");
        assert_eq!(steps(&mut template, w), ["minecraft:block.stone.step"]);
        assert_eq!(steps(&mut content, w), ["minecraft:block.stone.step"]);
        content.extra.flag2 = true;
        template.extra.flag2 = true;
        assert_eq!(
            ambient(&mut content, w),
            ["minecraft:entity.villager.trade"]
        );
        assert_eq!(
            ambient(&mut template, w),
            ["minecraft:entity.villager.trade"]
        );
    });
}
#[test]
fn entity_sounds_override_each_trigger_with_exact_full_ids() {
    fixture(
        "minecraft:wandering_trader",
        serde_json::json!({"ambient":"bird:entity.vulture.ambient","hurt":"bird:hurt","death":"bird:death","step":"bird:step"}),
        |w, types| {
            assert_eq!(
                ambient(&mut mob(types, "bird:vulture"), w),
                ["bird:entity.vulture.ambient"]
            );
            assert_eq!(hit(&mut mob(types, "bird:vulture"), 1.0), ["bird:hurt"]);
            assert_eq!(hit(&mut mob(types, "bird:vulture"), 1000.0), ["bird:death"]);
            assert_eq!(steps(&mut mob(types, "bird:vulture"), w), ["bird:step"]);
        },
    );
}
#[test]
fn entity_sounds_mute_all_four_without_emitting_sound_events() {
    fixture(
        "minecraft:villager",
        serde_json::json!({"ambient":"none","hurt":"none","death":"none","step":"none"}),
        |w, types| {
            assert!(ambient(&mut mob(types, "bird:vulture"), w).is_empty());
            assert!(hit(&mut mob(types, "bird:vulture"), 1.0).is_empty());
            assert!(hit(&mut mob(types, "bird:vulture"), 1000.0).is_empty());
            assert!(steps(&mut mob(types, "bird:vulture"), w).is_empty());
        },
    );
}
#[test]
fn entity_sounds_partial_override_keeps_other_template_slots() {
    fixture(
        "minecraft:wandering_trader",
        serde_json::json!({"hurt":"bird:hurt"}),
        |w, types| {
            assert_eq!(
                ambient(&mut mob(types, "bird:vulture"), w),
                ["minecraft:entity.wandering_trader.ambient"]
            );
            assert_eq!(hit(&mut mob(types, "bird:vulture"), 1.0), ["bird:hurt"]);
            assert_eq!(
                hit(&mut mob(types, "bird:vulture"), 1000.0),
                ["minecraft:entity.wandering_trader.death"]
            );
            assert_eq!(
                steps(&mut mob(types, "bird:vulture"), w),
                ["minecraft:block.stone.step"]
            );
        },
    );
}
#[test]
fn entity_sounds_explicit_minecraft_id_is_not_prefixed_twice() {
    fixture(
        "minecraft:villager",
        serde_json::json!({"ambient":"minecraft:entity.wandering_trader.ambient"}),
        |w, types| {
            assert_eq!(
                ambient(&mut mob(types, "bird:vulture"), w),
                ["minecraft:entity.wandering_trader.ambient"]
            );
        },
    );
}

#[test]
fn entity_sounds_step_only_override_and_mute_preserve_ambient() {
    for step in ["bird:step", "none"] {
        fixture(
            "minecraft:villager",
            serde_json::json!({"step":step}),
            |w, types| {
                assert_eq!(
                    ambient(&mut mob(types, "bird:vulture"), w),
                    ["minecraft:entity.villager.ambient"]
                );
                let heard = steps(&mut mob(types, "bird:vulture"), w);
                if step == "none" {
                    assert!(heard.is_empty());
                } else {
                    assert_eq!(heard, ["bird:step"]);
                }
            },
        );
    }
}

#[test]
fn entity_sounds_malformed_row_reports_entity_id() {
    for sounds in [serde_json::json!({"hurt":42}), serde_json::Value::Null] {
        let row = serde_json::json!({"sounds":sounds});
        let error = Sounds::load("bird:vulture", &row).unwrap_err().to_string();
        assert!(error.contains("bird:vulture"), "{error}");
        let dir = tempfile::tempdir().unwrap();
        let mut row = row;
        row["template"] = serde_json::json!("minecraft:villager");
        std::fs::write(
            dir.path().join("entity-types.json"),
            serde_json::to_vec(&serde_json::json!({"bird:vulture":row})).unwrap(),
        )
        .unwrap();
        let attributes =
            sunrise_world::attributes::entity::Catalog::from_json(&serde_json::json!({
                "data_version": sunrise_world::DATA_VERSION,
                "attributes": [],
                "entities": {},
            }))
            .unwrap();
        let Err(error) = MobTypes::load_for_server(dir.path(), &attributes) else {
            panic!("malformed sounds were admitted by the entity loader");
        };
        assert!(error.to_string().contains("bird:vulture"), "{error}");
    }
    assert!(Sounds::load("bird:vulture", &serde_json::json!({})).is_ok());
}

/// The played sounds with their volume and pitch, as the server sends them.
fn played(events: &[MobEvent]) -> Vec<(String, f32, f32)> {
    events
        .iter()
        .filter_map(|event| match event {
            MobEvent::Sound(id, _, volume, pitch) => {
                Some((full_id(id).into_owned(), *volume, *pitch))
            }
            MobEvent::SoundId(id, _, volume, pitch) => Some((id.clone(), *volume, *pitch)),
            _ => None,
        })
        .collect()
}

#[test]
fn entity_sounds_declared_step_plays_at_its_volume_and_pitch() {
    // An override of `playStepSound` calling `playSound(step, volume, pitch)`.
    fixture(
        "minecraft:villager",
        serde_json::json!({"step":{"sound":"bird:step","volume":1.0,"pitch":0.8}}),
        |w, types| {
            let mut events = vec![];
            let mut m = mob(types, "bird:vulture");
            m.movement_emission(w, Vec3::new(2.0, 0.0, 0.0), &mut events);
            assert_eq!(played(&events), [("bird:step".to_owned(), 1.0, 0.8)]);
        },
    );
    // The plain id keeps the block step's `soundType.getVolume() * 0.15F` and pitch.
    fixture(
        "minecraft:villager",
        serde_json::json!({"step":"bird:step"}),
        |w, types| {
            let mut events = vec![];
            let mut m = mob(types, "bird:vulture");
            m.movement_emission(w, Vec3::new(2.0, 0.0, 0.0), &mut events);
            assert_eq!(played(&events), [("bird:step".to_owned(), 0.15, 1.0)]);
        },
    );
}

/// A row of `entity-types.json` for a content villager, loaded with no vanilla data.
fn vulture(sounds: serde_json::Value) -> Mob {
    let dir = tempfile::tempdir().unwrap();
    let row = serde_json::json!({"template":"minecraft:villager","sounds":sounds});
    std::fs::write(
        dir.path().join("entity-types.json"),
        serde_json::to_vec(&serde_json::json!({"bird:vulture":row})).unwrap(),
    )
    .unwrap();
    let attributes = sunrise_world::attributes::entity::Catalog::from_json(&serde_json::json!({
        "data_version": sunrise_world::DATA_VERSION,
        "attributes": [],
        "entities": {},
    }))
    .unwrap();
    // Loading types seeds the process-wide sulfur cube lookup, which its tests need filled.
    crate::mobs::sulfur_cube::test_archetype_items();
    let types = MobTypes::load_for_server(dir.path(), &attributes).unwrap();
    Mob::new(
        1,
        Arc::new(types.types["bird:vulture"].clone()),
        Vec3::ZERO,
        0.0,
    )
}

#[test]
fn entity_sounds_declared_volume_voice_pitch_and_interval_replace_the_templates() {
    let mut declared =
        vulture(serde_json::json!({"volume":0.25,"voice_pitch":0.5,"ambient_interval":200}));
    assert_eq!(crate::mobs::ai::sound_volume(&declared), 0.25);
    assert_eq!(declared.ambient_interval(), 200);
    let adult: Vec<f32> = (0..64).map(|_| declared.voice_pitch()).collect();
    assert!(adult.iter().all(|p| (0.3..=0.7).contains(p)), "{adult:?}");
    // The baby's higher pitch applies on top: 1.5 times the base, spread as before.
    declared.extra.age = -24000;
    let baby: Vec<f32> = (0..64).map(|_| declared.voice_pitch()).collect();
    assert!(baby.iter().all(|p| (0.55..=0.95).contains(p)), "{baby:?}");

    // Omitted keys keep a villager's: `getSoundVolume` 1.0, the 80-tick interval,
    // and the same pitch draws as the template's.
    let mut omitted = vulture(serde_json::json!({}));
    assert_eq!(crate::mobs::ai::sound_volume(&omitted), 1.0);
    assert_eq!(omitted.ambient_interval(), 80);
    use sunrise_core::random::RandomSource;
    let mut random = sunrise_core::random::LegacyRandom::new(7);
    omitted.random = random.clone();
    let pitches: Vec<f32> = (0..64).map(|_| omitted.voice_pitch()).collect();
    let vanilla: Vec<f32> = (0..64)
        .map(|_| (random.next_float() - random.next_float()) * 0.2 + 1.0)
        .collect();
    assert_eq!(pitches, vanilla);
}
