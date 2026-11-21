use super::{PAGE, run, success};
use serde_json::json;
use std::{fs, io::Read, path::Path};
use sunrise_mod_package::{
    index::{
        self,
        registry::{
            Entry,
            builds::{Archive, Descriptor},
        },
    },
    sdk,
};

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const SOURCE_RELEASE: &str = "26.3-0.1.0";
const BUILD_RELEASE: &str = "26.3-0.1.1";
const DESCRIPTOR: &str = "release_probe-1.0.0+26.3-0.1.1.binaries.json";
const ROW: &str = "mods/release_probe.toml";
const SOURCE: &str = "release_probe-1.0.0.smod";
const REPO: &str = "https://github.com/test-author/release_probe";
const ASSET_BASE: &str = "https://github.com/test-author/release_probe/releases/download/v1.0.0";

struct Assets {
    root: tempfile::TempDir,
    descriptor: Descriptor,
    descriptor_hash: String,
    key: String,
}

impl Assets {
    fn new(signer: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path();
        let archives = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../sunrise-mod-package/tests/index/stable-native");
        let fixture = archives.join(signer);
        let source = fs::read(fixture.join("source.smod")).unwrap();
        fs::write(dir.join("source.smod"), &source).unwrap();
        fs::copy(
            fixture.join("source.smod.minisig"),
            dir.join("source.smod.minisig"),
        )
        .unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(source)).unwrap();
        let mut manifest = format!("homepage='{REPO}'\nlicense='MIT'\n");
        zip.by_name("sunrise-mod.toml")
            .unwrap()
            .read_to_string(&mut manifest)
            .unwrap();
        fs::write(dir.join("sunrise-mod.toml"), manifest).unwrap();
        let key = fs::read_to_string(fixture.join("fixture.pub"))
            .unwrap()
            .trim()
            .to_owned();
        let desc_bytes = fs::read(fixture.join("valid.binaries.json")).unwrap();
        let descriptor_hash = index::sha256(&desc_bytes);
        let descriptor: Descriptor = serde_json::from_slice(&desc_bytes).unwrap();
        let assets = dir.join("assets");
        fs::create_dir(&assets).unwrap();
        fs::write(assets.join(DESCRIPTOR), desc_bytes).unwrap();
        fs::copy(
            fixture.join("valid.binaries.json.minisig"),
            assets.join(format!("{DESCRIPTOR}.minisig")),
        )
        .unwrap();
        for archive in &descriptor.archives {
            fs::copy(archives.join(&archive.name), assets.join(&archive.name)).unwrap();
            fs::write(
                assets.join(format!("{}.minisig", archive.name)),
                &archive.signature,
            )
            .unwrap();
        }
        fs::write(
            dir.join("releases.json"),
            json!([
                {"tag": SOURCE_RELEASE, "commit": COMMIT},
                {"tag": BUILD_RELEASE, "commit": &descriptor.build}
            ])
            .to_string(),
        )
        .unwrap();
        let sdk_data = dir.join("sdk-data").join(&descriptor.build);
        fs::create_dir_all(&sdk_data).unwrap();
        fs::write(sdk_data.join("surface.json"), r#"{"hash":"fixture"}"#).unwrap();
        for side in [sdk::Side::Client, sdk::Side::Server] {
            for kind in [sdk::Kind::Api, sdk::Kind::Runtime] {
                fs::write(
                    sdk_data.join(sdk::Asset::name(kind, "x86_64-unknown-linux-gnu", side)),
                    b"unused SDK slice",
                )
                .unwrap();
            }
        }
        let index = sdk::Index::of_folder(
            &sdk_data,
            sdk::Build {
                commit: descriptor.build.clone(),
                game: "26.3".into(),
                date: "2026-11-20".into(),
                nightly: None,
            },
            descriptor.rust.clone(),
        )
        .unwrap();
        fs::write(sdk_data.join(sdk::INDEX), index.to_json()).unwrap();
        Self {
            root,
            descriptor,
            descriptor_hash,
            key,
        }
    }

    fn path(&self) -> &Path {
        self.root.path()
    }

    fn publish(&self, out: &str, checkout: Option<&str>, binaries: bool) -> std::process::Output {
        let mut args = vec![
            "publish",
            "--out",
            out,
            "--package",
            "source.smod",
            "--releases",
            "releases.json",
        ];
        if let Some(checkout) = checkout {
            args.extend(["--registry", checkout]);
        } else {
            args.extend(PAGE);
            args.extend([
                "--sunrise",
                SOURCE_RELEASE,
                "--key",
                &self.key,
                "--signature",
                "source.smod.minisig",
            ]);
        }
        if binaries {
            args.extend([
                "--binaries",
                "assets",
                "--asset-base",
                ASSET_BASE,
                "--sdk-data",
                "sdk-data",
            ]);
        }
        run(self.path(), &args)
    }

    fn assert_assets_staged(&self, dest: &str) {
        let root = self.path();
        let out = root.join(dest);
        let staged = |name: &str, hash: &str| {
            let original = fs::read(root.join("assets").join(name)).unwrap();
            assert_eq!(fs::read(out.join(name)).unwrap(), original);
            assert_eq!(fs::read(out.join("files").join(hash)).unwrap(), original);
        };
        staged(DESCRIPTOR, &self.descriptor_hash);
        for Archive { name, sha256, .. } in &self.descriptor.archives {
            staged(name, sha256);
        }
    }
}

#[test]
#[ignore = "daily"]
fn describe_builds_binds_each_signed_host_without_compiling() {
    let fixture = Assets::new("author-publish");
    let root = fixture.path();
    let assets = root.join("assets");
    let descriptor = assets.join(DESCRIPTOR);
    fs::remove_file(&descriptor).unwrap();
    let args = [
        "build",
        "--describe-binaries",
        "assets",
        "--sunrise-release",
        BUILD_RELEASE,
        "--releases",
        "releases.json",
        "--sdk-data",
        "sdk-data",
    ];
    success(run(root, &args));
    // The result is exactly the expected input to signing, with both host keys and sidecars.
    assert_eq!(
        index::sha256(&fs::read(&descriptor).unwrap()),
        fixture.descriptor_hash
    );
    assert!(!root.join("target").exists());
    fs::remove_file(&descriptor).unwrap();
    let name = &fixture.descriptor.archives[1].name;
    fs::remove_file(assets.join(name)).unwrap();
    let refused = run(root, &args);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains(name.as_str()));
    assert!(!descriptor.exists());
}

#[test]
#[ignore = "daily"]
fn signed_binaries_stage_with_the_original_source_for_new_and_existing_versions() {
    let fixture = Assets::new("author-publish");
    let root = fixture.path();
    let entry = |folder: &str| -> Entry {
        toml::from_str(&fs::read_to_string(root.join(folder).join(ROW)).unwrap()).unwrap()
    };
    success(fixture.publish("new", None, true));
    fixture.assert_assets_staged("new");
    assert_eq!(
        fs::read(root.join("new").join(SOURCE)).unwrap(),
        fs::read(root.join("source.smod")).unwrap()
    );

    success(fixture.publish("reviewed", None, false));
    let original = fs::read(root.join("reviewed").join(ROW)).unwrap();
    // The working checkout targets the rebuild; the reviewed release stays signed at its old pin.
    let path = root.join("sunrise-mod.toml");
    let manifest = fs::read_to_string(&path)
        .unwrap()
        .replace(COMMIT, &fixture.descriptor.build);
    fs::write(path, manifest).unwrap();
    success(fixture.publish("append", Some("reviewed"), true));
    fixture.assert_assets_staged("append");
    let append = entry("append");
    let mut restored = append.clone();
    assert_eq!(
        restored.versions[0].builds.pop().unwrap(),
        entry("new").versions[0].builds[0]
    );
    assert_eq!(restored, entry("reviewed"));
    assert_eq!(fs::read(root.join("reviewed").join(ROW)).unwrap(), original);
    assert!(!root.join("append").join(SOURCE).exists());
    assert!(
        !root
            .join("append/files")
            .join(&append.versions[0].sha256)
            .exists()
    );

    let again = fixture.publish("again", Some("append"), true);
    assert!(!again.status.success());
    assert!(String::from_utf8_lossy(&again.stderr).contains("append-only"));
    assert!(!root.join("again").exists());

    let archive = root
        .join("assets")
        .join(&fixture.descriptor.archives[0].name);
    let mut corrupted = fs::read(&archive).unwrap();
    corrupted[0] ^= 1;
    fs::write(archive, corrupted).unwrap();
    let refused = fixture.publish("bad", Some("reviewed"), true);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("SHA-256"));
    assert!(!root.join("bad").exists());
}

#[test]
#[ignore = "daily"]
fn an_asset_write_failure_leaves_no_registry_entry_or_pr_draft() {
    let fixture = Assets::new("author-publish");
    let out = fixture.path().join("blocked");
    let archive = &fixture.descriptor.archives[0].name;
    // Replacing a directory with a staged regular file fails after verification.
    fs::create_dir_all(out.join(archive)).unwrap();
    let refused = fixture.publish("blocked", None, true);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains(archive.as_str()));
    assert!(!out.join(ROW).exists());
    assert!(!out.join("PULL_REQUEST.md").exists());
}

#[test]
#[ignore = "daily"]
fn a_reviewed_exact_source_can_add_binaries_without_repacking() {
    // The original signed fixture has no uses list. A new source still needs one.
    let fixture = Assets::new("");
    let root = fixture.path();
    let refused = fixture.publish("new", None, true);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("no uses.json"));
    assert!(!root.join("new").exists());

    let hash = index::sha256(&fs::read(root.join("source.smod")).unwrap());
    let signature = fs::read_to_string(root.join("source.smod.minisig")).unwrap();
    fs::create_dir_all(root.join("reviewed/mods")).unwrap();
    let reviewed = root.join("reviewed").join(ROW);
    let original = format!(
        "id='release_probe'\nkind='native-mod'\nname='Release probe'\nrepo='test-author/release_probe'\n\
         owners=['test-author']\nkey='{}'\n\n[[versions]]\nversion='1.0.0'\n game='26.3'\n\
         file='{SOURCE}'\nsunrise='{SOURCE_RELEASE}'\npublished='2026-11-20'\n\
         sha256='{hash}'\nsignature='''{signature}'''\n",
        fixture.key
    );
    fs::write(&reviewed, &original).unwrap();
    success(fixture.publish("append", Some("reviewed"), true));
    fixture.assert_assets_staged("append");
    let mut appended: Entry =
        toml::from_str(&fs::read_to_string(root.join("append").join(ROW)).unwrap()).unwrap();
    assert_eq!(
        appended.versions[0].builds.pop().unwrap().sunrise,
        BUILD_RELEASE
    );
    assert_eq!(appended, toml::from_str(&original).unwrap());
    assert_eq!(fs::read_to_string(reviewed).unwrap(), original);
    assert!(!root.join("append").join(SOURCE).exists());
}
