use super::*;
use crate::{Package, content::Content};

#[test]
fn entity_sounds_admission_preserves_inherit_id_and_mute_and_rejects_bad_ids() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bird.smod");
    let manifest = "id = \"bird\"\nname = \"Bird\"\nversion = \"1.0\"\napi = \"0.2\"\nside = \"both\"\ncapabilities = [\"registries\"]\n";
    for slot in ["ambient", "hurt", "death", "step"] {
        let file = format!(
            r#"{{"template":"minecraft:wandering_trader","sounds":{{"{slot}":"bird:bad id"}}}}"#
        );
        std::fs::write(
            &path,
            Package::write_files(
                manifest,
                &[(
                    "data/bird/sunrise/entity_type/vulture.json",
                    file.as_bytes(),
                )],
            )
            .unwrap(),
        )
        .unwrap();
        let package = Package::read(&path).unwrap();
        let error = Content::read(&path, &package.manifest).expect_err("bad sound admitted");
        assert!(
            error.contains("bird:vulture") && error.contains(slot),
            "{error}"
        );
    }
    let sounds: EntitySounds =
        serde_json::from_str(r#"{"hurt":"bird:hurt","step":"none"}"#).unwrap();
    assert_eq!(sounds.ambient, None);
    assert_eq!(sounds.hurt, Some(SoundOverride::Id("bird:hurt".into())));
    assert!(matches!(sounds.step, Some(StepOverride::Mute(_))));
    sounds.validate("bird:vulture").unwrap();
    for bad in ["hurt", "Bird:hurt", "bird:", ":hurt", "bird:a:b"] {
        let sounds = EntitySounds {
            hurt: Some(SoundOverride::Id(bad.into())),
            ..Default::default()
        };
        assert!(sounds.validate("bird:vulture").is_err(), "{bad}");
    }
}

#[test]
fn entity_sounds_thrown_type_refuses_sounds_with_exact_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bird.smod");
    let manifest = "id = \"bird\"\nname = \"Bird\"\nversion = \"1.0\"\napi = \"0.2\"\nside = \"both\"\ncapabilities = [\"registries\"]\n";
    let declaration = br#"{"template":"minecraft:snowball","sounds":{"hurt":"bird:hurt"}}"#;
    std::fs::write(
        &path,
        Package::write_files(
            manifest,
            &[("data/bird/sunrise/entity_type/bolt.json", declaration)],
        )
        .unwrap(),
    )
    .unwrap();
    let package = Package::read(&path).unwrap();
    let error = Content::read(&path, &package.manifest).expect_err("thrown sounds admitted");
    assert_eq!(
        error,
        "data/bird/sunrise/entity_type/bolt.json: only a living type has sounds (its template is minecraft:snowball)"
    );
}

#[test]
fn entity_sounds_numbers_admit_finite_values_and_refuse_the_rest() {
    let sounds: EntitySounds = serde_json::from_str(
        r#"{"step":{"sound":"bird:step","volume":1.0,"pitch":0.8},"volume":-0.5,"voice_pitch":0.0,"ambient_interval":1}"#,
    )
    .unwrap();
    assert_eq!(
        sounds.step,
        Some(StepOverride::Sound(StepSound {
            sound: "bird:step".into(),
            volume: 1.0,
            pitch: 0.8,
        }))
    );
    // Vanilla bounds none of these, so neither does the declaration.
    sounds.validate("bird:vulture").unwrap();
    // A plain id and "none" keep their meanings.
    let plain: EntitySounds = serde_json::from_str(r#"{"step":"bird:step"}"#).unwrap();
    assert_eq!(plain.step, Some(StepOverride::Id("bird:step".into())));
    let mute: EntitySounds = serde_json::from_str(r#"{"step":"none"}"#).unwrap();
    assert_eq!(mute.step, Some(StepOverride::Mute(Mute::None)));
    // The object's id is checked as a plain one is.
    let bad: EntitySounds =
        serde_json::from_str(r#"{"step":{"sound":"step","volume":1.0,"pitch":1.0}}"#).unwrap();
    let error = bad.validate("bird:vulture").unwrap_err();
    assert!(error.contains("sounds.step"), "{error}");

    // 1e39 is past f32. The reader refuses it in a plain key; in the step object, buffered
    // for its untagged forms, it arrives as infinity and the check refuses it.
    for (json, key) in [
        (
            r#"{"step":{"sound":"bird:step","volume":1e39,"pitch":1.0}}"#,
            Some("step.volume"),
        ),
        (
            r#"{"step":{"sound":"bird:step","volume":1.0,"pitch":-1e39}}"#,
            Some("step.pitch"),
        ),
        (r#"{"volume":1e39}"#, None),
        (r#"{"voice_pitch":1e39}"#, None),
        (r#"{"ambient_interval":0}"#, Some("ambient_interval")),
        (r#"{"ambient_interval":-80}"#, Some("ambient_interval")),
    ] {
        match serde_json::from_str::<EntitySounds>(json) {
            Err(error) => assert!(key.is_none(), "{json}: {error}"),
            Ok(sounds) => {
                let error = sounds.validate("bird:vulture").unwrap_err();
                let key = key.unwrap_or_else(|| panic!("{json}: read, then {error}"));
                assert!(
                    error.starts_with(&format!("bird:vulture: sounds.{key}: ")),
                    "{json}: {error}"
                );
            }
        }
    }
    // An unknown key in the step object is refused.
    assert!(
        serde_json::from_str::<EntitySounds>(
            r#"{"step":{"sound":"bird:step","volume":1.0,"pitch":1.0,"range":16}}"#
        )
        .is_err()
    );
}
