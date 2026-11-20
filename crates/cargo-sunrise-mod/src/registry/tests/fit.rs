use super::{RELEASE_PIN, RELEASE_TAG, Registry};
use mod_surface::uses::{Entry, USES_FILE, USES_SCHEMA, UsesTable};
use serde_json::{Value, json};
use std::{fs, path::Path};
use sunrise_mod_package::{build_data, sdk};

const DAY: &str = "2026-11-20";
const TARGET: &str = "sunrise_client::game::example";

pub(super) fn sdk(root: &Path, commit: &str, functions: Value) {
    let dir = root.join(commit);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("surface.json"),
        json!({
            "schema": mod_surface::SCHEMA, "hash": "test-surface",
            "functions": [{"id": "fn:example", "hash": "api", "signature": "fn example()"}]
        })
        .to_string(),
    )
    .unwrap();
    let build = sdk::Build {
        commit: commit.into(),
        game: "26.3".into(),
        date: DAY.into(),
        nightly: None,
    };
    let index = sdk::Index::of_folder(&dir, build, "1.99.1".into()).unwrap();
    fs::write(dir.join(sdk::INDEX), index.to_json()).unwrap();
    let data = dir.join("build-data");
    fs::create_dir(&data).unwrap();
    let build = build_data::Build {
        commit: commit.into(),
    };
    for (name, body) in [
        (
            build_data::FUNCTIONS,
            json!({"crates": {}, "functions": functions, "refused": {}}),
        ),
        (build_data::INTERNALS, json!({"internals": []})),
        (build_data::LOOPS, json!({"loops": []})),
    ] {
        fs::write(
            data.join(name),
            serde_json::to_vec(&build_data::Document::new(&build, body)).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn registry_judges_earlier_releases_on_the_line_and_the_current_nightly() {
    let r = Registry::new();
    let root = r.0.path();
    let exact = "26.3-0.2.0";
    let older = "a".repeat(40);
    let other_line = "b".repeat(40);
    let nightly = "c".repeat(40);
    let releases = root.join("releases.json");
    fs::write(
        &releases,
        json!([
            {"tag": RELEASE_TAG, "commit": older}, {"tag": exact, "commit": RELEASE_PIN},
            {"tag": "26.3-1.0.0", "commit": other_line}
        ])
        .to_string(),
    )
    .unwrap();
    let nightly_file = root.join("nightly-build.json");
    let nightly_build = json!({"date": DAY, "commit": nightly});
    fs::write(&nightly_file, nightly_build.to_string()).unwrap();
    let data = root.join("sdk");
    for (commit, body) in [
        (&*older, "old"),
        (RELEASE_PIN, "old"),
        (&*other_line, "old"),
        (&*nightly, "new"),
    ] {
        sdk(
            &data,
            commit,
            json!({TARGET: {
                "module": "sunrise_client::game", "file": "crates/sunrise-client/src/game.rs",
                "span": [0, 1], "signature_hash": "sig", "sha256": body
            }}),
        );
    }

    let rev = sunrise_mod_package::mixins_tool::REV;
    let manifest = format!(
        "id='patched'\nname='Patched'\nversion='1.0.0'\napi='0.2'\n[native]\ngame='26.3'\nsunrise='{RELEASE_PIN}'\nmixins='{rev}'\n\
        [[native.mixin]]\nid='patch'\ntarget='{TARGET}'\nkind='HEAD'\nhelper='src/helper.rs'\n"
    );
    let game_item = format!("game:fn:{TARGET}");
    let uses = UsesTable {
        schema: USES_SCHEMA,
        made_for: RELEASE_PIN.into(),
        entries: vec![
            Entry::plain("fn:example".into(), "api".into()),
            Entry::plain(game_item.clone(), "sig:old".into()),
        ],
    }
    .to_json();
    r.add_package(
        "patched",
        "1.0.0",
        "26.3",
        "native-mod",
        &[
            (sunrise_mod_package::manifest::MANIFEST, &manifest),
            ("Cargo.toml", "[package]\nname='patched'\nversion='1.0.0'\n"),
            ("src/lib.rs", ""),
            ("src/helper.rs", ""),
            (USES_FILE, &uses),
        ],
    );
    let sample: Value = serde_json::from_str(include_str!(
        "../../../../sunrise-mod-package/tests/index/index-v1.json"
    ))
    .unwrap();
    let signature = sample["projects"][0]["versions"][0]["file"]["signature"]
        .as_str()
        .unwrap();
    let triple = "x86_64-unknown-linux-gnu";
    r.edit("patched", |t| format!("{t}sunrise='{exact}'\n\
        [[versions.native_binaries]]\nbuild='{RELEASE_PIN}'\ntarget='{triple}'\n\
        [versions.native_binaries.file]\nurl='https://example.test/native.tar.xz'\nname='native.tar.xz'\n\
        size=1\nsha256='{}'\nsignature='''{signature}'''\n", "a".repeat(64)));
    r.add("legacy", "1.0.0", "26.3", "");
    r.add_package(
        "unlisted",
        "1.0.0",
        "26.3",
        "native-mod",
        &[
            (sunrise_mod_package::manifest::MANIFEST, &format!(
                "id='unlisted'\nname='Unlisted'\nversion='1.0.0'\napi='0.2'\n[native]\ngame='26.3'\nsunrise='{RELEASE_PIN}'\n"
            )),
            ("Cargo.toml", "[package]\nname='unlisted'\nversion='1.0.0'\n"),
            ("src/lib.rs", ""),
        ],
    );
    r.edit("unlisted", |t| format!("{t}sunrise='{exact}'\n"));

    let index = r
        .build(&[
            "--releases",
            releases.to_str().unwrap(),
            "--nightly-build",
            nightly_file.to_str().unwrap(),
            "--sdk-data",
            data.to_str().unwrap(),
        ])
        .unwrap();
    assert_eq!(
        serde_json::to_value(index.nightly_build.as_ref().unwrap()).unwrap(),
        nightly_build
    );
    let version = &index.project("patched").unwrap().versions[0];
    assert_eq!(
        serde_json::to_value(&version.verdicts).unwrap(),
        json!({
            (RELEASE_TAG): {"verdict": "fits", "targets": []},
            (exact): {"verdict": "made-for", "targets": [triple]},
            "nightly": {"verdict": "rebuild", "targets": [], "changed": [game_item]}
        })
    );
    assert_eq!(
        serde_json::to_value(&index.project("unlisted").unwrap().versions[0].verdicts[RELEASE_TAG])
            .unwrap(),
        json!({"verdict": "rebuild", "targets": [], "reason": "made for one build only"})
    );
    assert!(
        index.project("legacy").unwrap().versions[0]
            .verdicts
            .is_empty()
    );
}

#[test]
fn a_missing_sdk_file_names_the_release_even_if_no_mod_uses_the_data() {
    let r = Registry::new();
    let path = r.0.path().join("releases.json");
    fs::write(
        &path,
        json!([{"tag": RELEASE_TAG, "commit": RELEASE_PIN}]).to_string(),
    )
    .unwrap();
    let dir = r.0.path().join("sdk");
    sdk(&dir, RELEASE_PIN, json!({}));
    fs::remove_file(
        dir.join(RELEASE_PIN)
            .join("build-data")
            .join(build_data::LOOPS),
    )
    .unwrap();
    let err = r
        .build(&[
            "--releases",
            path.to_str().unwrap(),
            "--sdk-data",
            dir.to_str().unwrap(),
        ])
        .unwrap_err();
    for word in [RELEASE_TAG, RELEASE_PIN, build_data::LOOPS] {
        assert!(err.contains(word), "{err}");
    }
}
