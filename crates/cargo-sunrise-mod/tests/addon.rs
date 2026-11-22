use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::Path,
    process::{Command, Output},
    thread,
};
use sunrise_mod_package::{
    Package,
    index::{self, Kind},
};

#[test]
#[ignore = "daily"]
fn builds_and_checks_a_native_addon_with_local_and_registry_libraries() {
    // book-quickstart assembles and signs this run's SDK, like the native run smoke test.
    let target = Path::new(env!("CARGO_TARGET_TMPDIR"));
    fs::create_dir_all(target).unwrap();
    let scratch = tempfile::tempdir_in(target).unwrap();
    let root = scratch.path().canonicalize().unwrap();
    let library = root.join("test_library");
    let addon = root.join("test_addon");
    let tool = |dir: &Path, args: &[&str], registry: Option<&str>| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_cargo-sunrise-mod"));
        cmd.current_dir(dir)
            .env("SUNRISE_MOD_CACHE", root.join("cache"))
            .env("SUNRISE_HOME", root.join("home"))
            .env("SUNRISE_LAUNCHER_HOME", root.join("launcher"))
            .env("SUNRISE_LAUNCHER_NO_SELF_UPDATE", "1")
            .env("SUNRISE_NO_AUDIO", "1")
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("NO_PROXY", "127.0.0.1")
            .env("no_proxy", "127.0.0.1")
            .args(args);
        if let Some(registry) = registry {
            cmd.env("SUNRISE_MOD_REGISTRY_TEST_BASE", registry);
        }
        cmd.output().unwrap()
    };
    let passed = |output: Output| {
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    passed(tool(
        &root,
        &["new", "test_library", "--template", "native"],
        None,
    ));
    passed(tool(
        &root,
        &["new", "test_addon", "--template", "native"],
        None,
    ));
    fs::write(
        library.join("src/lib.rs"),
        r#"
#[derive(Default)]
struct Library;
#[mod_api::sunrise_mod]
impl mod_api::Hooks for Library {}
pub fn magic() -> u32 { 1729 }
"#,
    )
    .unwrap();
    fs::write(
        addon.join("src/lib.rs"),
        r#"
#[derive(Default)]
struct Addon;
#[mod_api::sunrise_mod]
impl mod_api::Hooks for Addon {
    fn register(&self, _: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        assert_eq!(test_library::magic(), 1729);
        std::process::exit(0);
    }
}
"#,
    )
    .unwrap();
    // The alias must be the mod ID, not the Cargo package's name.
    let cargo = library.join("Cargo.toml");
    let text = fs::read_to_string(&cargo).unwrap();
    fs::write(&cargo, text.replace("test_library", "check-library-crate")).unwrap();
    writeln!(
        fs::OpenOptions::new()
            .append(true)
            .open(addon.join("sunrise-mod.toml"))
            .unwrap(),
        "\n[dependencies]\ntest_library = '=0.1.0'"
    )
    .unwrap();
    let path = library.to_str().unwrap();
    for command in ["build", "check"] {
        passed(tool(&addon, &[command, "--with", path], None));
    }
    let source = addon.join("src/lib.rs");
    let code = fs::read_to_string(&source).unwrap();
    fs::write(
        &source,
        format!("{code}\nfn wrong() -> &'static str {{ test_library::magic() }}"),
    )
    .unwrap();
    let refused = tool(&addon, &["check", "--with", path], None);
    let error = String::from_utf8_lossy(&refused.stderr);
    assert!(
        !refused.status.success()
            && error.contains("mismatched types")
            && error.contains(&source.display().to_string()),
        "{error}"
    );
    fs::write(&source, code).unwrap();

    let manifest = library.join("sunrise-mod.toml");
    let original = fs::read_to_string(&manifest).unwrap();
    let pin = Package::read(&library)
        .unwrap()
        .manifest
        .native
        .unwrap()
        .sunrise;
    let other_pin = "a".repeat(40);
    fs::write(&manifest, original.replace(&pin, &other_pin)).unwrap();
    let refused = tool(&addon, &["check", "--with", path], None);
    let error = String::from_utf8_lossy(&refused.stderr);
    assert!(
        !refused.status.success()
            && error.contains("test_library")
            && error.contains(&other_pin)
            && error.contains("uses list"),
        "{error}"
    );
    fs::write(&manifest, original).unwrap();

    let bytes = Package::pack_directory(&library).unwrap();
    let package = root.join("test_library.smod");
    fs::write(&package, &bytes).unwrap();
    let facts = index::inspect(&Kind::NativeMod, &package).unwrap();
    let payload = serde_json::to_vec(&serde_json::json!({
        "schema": index::SCHEMA,
        "projects": [{
            "id": "test_library", "kind": "native-mod", "name": "Test Library", "summary": "Fixture library",
            "license": "MIT", "source": "https://example.org/source",
            "versions": [{"version": facts.version, "api": facts.api, "game": facts.game,
                "dependencies": facts.dependencies, "file": {"name": "test_library.smod", "size": bytes.len(),
                    "sha256": index::sha256(&bytes), "url": "https://example.org/test_library.smod"}}]
        }]
    })).unwrap();
    let parsed = index::Index::parse(std::str::from_utf8(&payload).unwrap()).unwrap();
    assert!(parsed.left_out.is_empty(), "{:?}", parsed.left_out);
    let http = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let base = format!("http://{}", http.local_addr().unwrap());
    let registry = thread::spawn(move || {
        // Both commands read the current index; only build downloads the package.
        for (expected, body) in [
            ("/index.json", &payload),
            ("/files/test_library/test_library.smod", &bytes),
            ("/index.json", &payload),
        ] {
            let (mut stream, _) = http.accept().unwrap();
            let mut input = BufReader::new(&mut stream);
            let mut line = String::new();
            input.read_line(&mut line).unwrap();
            assert_eq!(
                line.split_whitespace().take(2).collect::<Vec<_>>(),
                ["GET", expected]
            );
            while line != "\r\n" {
                line.clear();
                assert_ne!(input.read_line(&mut line).unwrap(), 0);
            }
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        }
    });
    for command in ["build", "check"] {
        let stdout = passed(tool(&addon, &[command], Some(&base)));
        assert!(
            stdout.contains("Using library test_library 0.1.0 from the Sunrise mod registry"),
            "{stdout}"
        );
    }
    registry.join().unwrap();
    let output = addon.join("target/sunrise-mod/test_addon.smod");
    assert_eq!(Package::read(&output).unwrap().manifest.id, "test_addon");
    assert_eq!(
        Package::entry(&output, "test_library/src/lib.rs", 4096).unwrap(),
        None
    );

    // `run` also carries the dependencies beside the SDK's executable. Registration needs only
    // the marker and the registries table and exits before a world opens.
    let resources = root.join("resources");
    fs::create_dir(&resources).unwrap();
    for name in ["sunrise-resources.json", "registries.json"] {
        fs::write(resources.join(name), "{}").unwrap();
    }
    let run = addon.join("target/sunrise-mod/run");
    fs::create_dir_all(run.join("game")).unwrap();
    fs::write(run.join("game/eula.txt"), "eula=true\n").unwrap();
    passed(tool(
        &addon,
        &[
            "run",
            "--with",
            package.to_str().unwrap(),
            "--resources",
            resources.to_str().unwrap(),
            "--",
            "--dedicated",
        ],
        None,
    ));
    let records = fs::read_to_string(run.join("app/native-mods.json")).unwrap();
    assert!(
        records.contains("test_addon") && records.contains("test_library"),
        "{records}"
    );
}
