use std::{fs, path::Path, process::Command};

#[test]
#[ignore = "daily"]
fn a_standalone_copy_updates_from_the_signed_rolling_release() {
    let home = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let (home_var, bin) = if cfg!(windows) {
        ("USERPROFILE", ".sunrise/bin")
    } else {
        ("HOME", ".local/bin")
    };
    let exe = format!("cargo-sunrise-mod{}", std::env::consts::EXE_SUFFIX);
    let installed = home.path().join(bin).join(&exe);
    let sdk_tool = home.path().join("sdk/tool").join(&exe);
    for path in [&installed, &sdk_tool] {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_cargo-sunrise-mod"), path).unwrap();
    }
    let command = |path: &Path| {
        let mut cmd = Command::new(path);
        cmd.current_dir(home.path()).env(home_var, home.path());
        cmd
    };
    let denied = command(&sdk_tool).arg("self-update").output().unwrap();
    assert!(!denied.status.success());
    assert!(
        String::from_utf8_lossy(&denied.stderr).contains("not in the standalone install location")
    );

    let version = |path: &Path| {
        let out = command(path).arg("--version").output().unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap()
    };
    let old = version(&installed);
    let old = old.split_whitespace().nth(2).unwrap();
    let info_url = format!("{}/mod-tool/tool.json", sunrise_addresses::SDK_RELEASES);
    let info = sunrise_proxy::get(&info_url, &[], None)
        .unwrap()
        .into_string()
        .unwrap();
    let info: serde_json::Value = serde_json::from_str(&info).unwrap();
    let new = info["commit"].as_str().unwrap();
    assert_ne!(old, new, "the fixture must need an update");

    let update = command(&installed)
        .args(["sunrise-mod", "self-update"])
        .output()
        .unwrap();
    assert!(
        update.status.success(),
        "{}",
        String::from_utf8_lossy(&update.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&update.stdout).trim(),
        format!("Updated cargo sunrise-mod: {old} -> {new}")
    );
    assert_eq!(version(&installed).split_whitespace().nth(2), Some(new));
    assert_eq!(version(&sdk_tool).split_whitespace().nth(2), Some(old));
}
