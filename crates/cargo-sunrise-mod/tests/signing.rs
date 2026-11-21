use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn tool(project: &Path) -> Command {
    let mut tool = Command::new(env!("CARGO_BIN_EXE_cargo-sunrise-mod"));
    tool.current_dir(project);
    tool
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[ignore = "daily"]
fn keygen_writes_registry_key_and_refuses_to_replace_either_file() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path();
    let public = success(
        tool(project)
            .args(["keygen", "--no-password"])
            .output()
            .unwrap(),
    );
    let key = public.trim();
    minisign_verify::PublicKey::from_base64(key).unwrap();
    let secret_file = project.join(".sunrise/signing.key");
    let public_file = project.join(".sunrise/signing.pub");
    let secret = fs::read(&secret_file).unwrap();
    assert_eq!(
        fs::read_to_string(&public_file).unwrap().lines().last(),
        Some(key)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&secret_file).unwrap().permissions().mode() & 0o077,
            0
        );
    }
    let refused = tool(project)
        .args(["keygen", "--no-password"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("signing.key already exists"));
    assert!(refused.stdout.is_empty());
    assert!(fs::read(&secret_file).unwrap() == secret);

    fs::create_dir(project.join("other")).unwrap();
    fs::write(project.join("other/signing.pub"), b"keep me").unwrap();
    let refused = tool(project)
        .args(["keygen", "--no-password", "--out", "other"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert_eq!(
        fs::read(project.join("other/signing.pub")).unwrap(),
        b"keep me"
    );
    assert!(!project.join("other/signing.key").exists());
}
