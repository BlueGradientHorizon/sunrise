use super::{RELEASE_PIN, RELEASE_TAG, Registry, fit};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
use sunrise_mod_package::{
    index::{FitVerdict, Index, registry, sha256},
    manifest::API,
    sdk,
};

const ID: &str = "release_probe";
const VERSION: &str = "1.0.0";
const TARGET: &str = "x86_64-unknown-linux-gnu";
const BUILD_TAG: &str = "26.3-0.1.1";
const BUILD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const URL: &str = "https://github.com/test-author/release_probe/releases/download/v1.0.0";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../sunrise-mod-package/tests/index/stable-native")
}

struct Release(Registry);

impl Release {
    fn new(source: &str) -> Self {
        let r = Registry::new();
        let root = r.0.path();
        let placeholder = r.add(ID, VERSION, "26.3", "");
        let bytes = fs::read(fixtures().join(source)).unwrap();
        let signature = fs::read_to_string(fixtures().join(format!("{source}.minisig"))).unwrap();
        let hash = Self::keep(&r, &bytes);
        let key = fs::read_to_string(fixtures().join("fixture.pub")).unwrap();
        r.edit(ID, |t| {
            t.replace("kind='mod'", "kind='native-mod'")
                .replace(
                    "[[versions]]",
                    &format!("key='{}'\n\n[[versions]]", key.trim()),
                )
                .replace(&placeholder, &hash)
                + &format!("sunrise='{RELEASE_TAG}'\nsignature='''{signature}'''\n")
        });
        r.make_base();
        for commit in [RELEASE_PIN, BUILD] {
            let sdk_data = root.join("sdk");
            let dir = sdk_data.join(commit);
            fs::create_dir_all(&dir).unwrap();
            for side in [sdk::Side::Client, sdk::Side::Server] {
                for kind in [sdk::Kind::Runtime, sdk::Kind::Api] {
                    fs::write(
                        dir.join(sdk::Asset::name(kind, TARGET, side)),
                        b"not built in registry CI",
                    )
                    .unwrap();
                }
            }
            fit::sdk(
                &sdk_data,
                commit,
                mod_surface::SCHEMA,
                "api",
                API,
                json!({}),
            );
        }
        fs::write(
            root.join("releases.json"),
            json!([
                {"tag": RELEASE_TAG, "commit": RELEASE_PIN}, {"tag": BUILD_TAG, "commit": BUILD}
            ])
            .to_string(),
        )
        .unwrap();
        for target in [TARGET.to_owned(), format!("{TARGET}-server")] {
            let name = registry::builds::archive_name(ID, VERSION, BUILD_TAG, &target);
            Self::keep(&r, &fs::read(fixtures().join(name)).unwrap());
        }
        Self(r)
    }

    fn keep(r: &Registry, bytes: &[u8]) -> String {
        let hash = sha256(bytes);
        fs::write(r.0.path().join("files").join(&hash), bytes).unwrap();
        hash
    }

    fn add_build(&self, descriptor: &[u8], signature: &str) {
        let hash = Self::keep(&self.0, descriptor);
        let name = registry::builds::descriptor_name(ID, VERSION, BUILD_TAG);
        self.0.edit(ID, |t| {
            format!(
                "{t}[[versions.builds]]\nsunrise='{BUILD_TAG}'\n\
            binaries='{URL}/{name}'\nsha256='{hash}'\nsignature='''{signature}'''\n"
            )
        });
    }

    fn use_descriptor(&self, name: &str) {
        let filename = format!("{name}.binaries.json");
        self.add_build(
            &fs::read(fixtures().join(&filename)).unwrap(),
            &fs::read_to_string(fixtures().join(format!("{filename}.minisig"))).unwrap(),
        );
    }

    fn flags(&self) -> [String; 4] {
        let root = self.0.0.path();
        [
            "--releases".into(),
            root.join("releases.json").to_str().unwrap().into(),
            "--sdk-data".into(),
            root.join("sdk").to_str().unwrap().into(),
        ]
    }

    fn check(&self) -> Result<(), String> {
        self.0.run(
            "check",
            &["base", "head"],
            &self.flags().each_ref().map(String::as_str),
        )
    }

    fn build(&self) -> Index {
        self.0
            .build(&self.flags().each_ref().map(String::as_str))
            .unwrap()
    }
}

#[test]
fn signed_build_appends_to_the_source_and_supplies_exact_fit_on_its_own_sdk() {
    let release = Release::new("source.smod");
    let without = release.build();
    let source = &without.project(ID).unwrap().versions[0];
    let verdict = &source.verdicts[BUILD_TAG];
    assert!(matches!(verdict.fit, FitVerdict::Exact { .. }));
    assert!(verdict.targets.is_empty());
    release.use_descriptor("valid");
    release.check().unwrap();
    let with = release.build();
    let version = &with.project(ID).unwrap().versions[0];
    assert_eq!(&version.file, &source.file);
    assert!(matches!(
        version.verdicts[BUILD_TAG].fit,
        FitVerdict::Exact { .. }
    ));
    assert_eq!(
        version.verdicts[BUILD_TAG].targets,
        [TARGET, &format!("{TARGET}-server")]
    );
    assert!(version.verdicts[RELEASE_TAG].targets.is_empty());
    let fixture =
        Index::parse(&fs::read_to_string(fixtures().join("schema-1.json")).unwrap()).unwrap();
    assert_eq!(
        version.native_binaries,
        fixture.project(ID).unwrap().versions[0].native_binaries
    );
}

#[test]
fn registry_refuses_invalid_signed_build_submissions() {
    for (source, descriptor, message) in [
        ("source.smod", "missing-host", "must cover every SDK host"),
        (
            "source.smod",
            "wrong-host",
            "identity.json does not match host",
        ),
        (
            "source.smod",
            "wrong-archive-signer",
            "signature doesn't match",
        ),
        (
            "no-lock.smod",
            "valid",
            "source package must carry Cargo.lock",
        ),
        ("features.smod", "valid", "E_NATIVE_FEATURES"),
    ] {
        let release = Release::new(source);
        release.use_descriptor(descriptor);
        let error = release.check().unwrap_err();
        assert!(error.contains(message), "{source} {descriptor}: {error}");
    }
}

#[test]
fn archive_bytes_must_match_even_in_the_correct_digest_slot() {
    let release = Release::new("source.smod");
    release.use_descriptor("valid");
    let name = registry::builds::archive_name(ID, VERSION, BUILD_TAG, TARGET);
    let mut bytes = fs::read(fixtures().join(name)).unwrap();
    let original = sha256(&bytes);
    bytes[0] ^= 1;
    fs::write(release.0.0.path().join("files").join(original), bytes).unwrap();
    assert!(
        release
            .check()
            .unwrap_err()
            .contains("archive size or SHA-256")
    );
}

#[test]
fn descriptor_authentication_binds_version_build_and_host_before_listing_archives() {
    let bytes = fs::read(fixtures().join("valid.binaries.json")).unwrap();
    let signature = fs::read_to_string(fixtures().join("valid.binaries.json.minisig")).unwrap();
    for (pointer, altered) in [
        ("/version", "1.0.1"),
        ("/build", RELEASE_PIN),
        ("/archives/0/target", "x86_64-unknown-linux-gnu-server"),
    ] {
        let mut descriptor: Value = serde_json::from_slice(&bytes).unwrap();
        *descriptor.pointer_mut(pointer).unwrap() = altered.into();
        let release = Release::new("source.smod");
        release.add_build(&serde_json::to_vec(&descriptor).unwrap(), &signature);
        let error = release.check().unwrap_err();
        assert!(
            error.contains("signature doesn't match"),
            "{pointer}: {error}"
        );
        let root = release.0.0.path();
        let head = registry::load(&root.join("head")).unwrap();
        assert!(registry::builds::assets(&head.entries, Some(&root.join("files"))).is_err());
    }
    let release = Release::new("source.smod");
    let other = fs::read_to_string(fixtures().join("other-signer.minisig")).unwrap();
    release.add_build(&bytes, &other);
    assert!(
        release
            .check()
            .unwrap_err()
            .contains("signature doesn't match")
    );
}

#[test]
fn list_supplies_descriptor_then_each_verified_archive_url() {
    let release = Release::new("source.smod");
    release.use_descriptor("valid");
    let root = release.0.0.path();
    let head = registry::load(&root.join("head")).unwrap();
    let assets = registry::builds::assets(&head.entries, None).unwrap();
    assert_eq!(assets.len(), 1);
    assert!(assets[0].url.ends_with(&format!(
        "/{}",
        registry::builds::descriptor_name(ID, VERSION, BUILD_TAG)
    )));
    assert_eq!(assets[0].size, None);
    let assets = registry::builds::assets(&head.entries, Some(&root.join("files"))).unwrap();
    assert_eq!(assets.len(), 2);
    for (asset, target) in assets.iter().zip([TARGET, &format!("{TARGET}-server")]) {
        let name = registry::builds::archive_name(ID, VERSION, BUILD_TAG, target);
        let bytes = fs::read(fixtures().join(&name)).unwrap();
        assert_eq!(asset.url, format!("{URL}/{name}"));
        assert_eq!(asset.size, Some(bytes.len() as u64));
        assert_eq!(asset.sha256, sha256(&bytes));
    }
}

#[test]
fn a_verified_archive_does_not_override_a_known_break() {
    let release = Release::new("source.smod");
    release.use_descriptor("valid");
    let sdk = release.0.0.path().join("sdk").join(BUILD).join(sdk::INDEX);
    let mut value: Value = serde_json::from_slice(&fs::read(&sdk).unwrap()).unwrap();
    value["content_api"] = "999".into();
    fs::write(sdk, value.to_string()).unwrap();
    let error = release.check().unwrap_err();
    assert!(
        error.contains("source does not fit: content-api"),
        "{error}"
    );
}
