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

const KEY_ENV: &str = "SUNRISE_MOD_SIGNING_KEY";
const PASSPHRASE_ENV: &str = "SUNRISE_MOD_SIGNING_PASSPHRASE";

#[test]
#[ignore = "daily"]
fn the_registry_verifies_the_detached_signature_of_the_exact_file() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path();
    let public = success(
        tool(project)
            .args(["keygen", "--no-password"])
            .output()
            .unwrap(),
    );
    let key = minisign_verify::PublicKey::from_base64(public.trim()).unwrap();
    let wrong = success(
        tool(project)
            .args(["keygen", "--no-password", "--out", "other"])
            .output()
            .unwrap(),
    );
    let wrong = minisign_verify::PublicKey::from_base64(wrong.trim()).unwrap();
    let bytes = b"signed release asset bytes\x00\xff";
    fs::write(project.join("mod.smod"), bytes).unwrap();
    success(
        tool(project)
            .args(["sign", "mod.smod", "--no-password"])
            .output()
            .unwrap(),
    );
    let first = fs::read_to_string(project.join("mod.smod.minisig")).unwrap();
    let signature = minisign_verify::Signature::decode(&first).unwrap();
    key.verify(bytes, &signature, false).unwrap();
    assert!(key.verify(b"changed bytes", &signature, false).is_err());
    assert!(wrong.verify(bytes, &signature, false).is_err());
    let refused = tool(project)
        .args(["sign", "mod.smod", "--no-password"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert_eq!(
        fs::read_to_string(project.join("mod.smod.minisig")).unwrap(),
        first
    );

    let secret = fs::read_to_string(project.join(".sunrise/signing.key")).unwrap();
    fs::remove_file(project.join(".sunrise/signing.key")).unwrap();
    fs::write(project.join("build.tar.xz"), bytes).unwrap();
    let output = tool(project)
        .args(["sign", "build.tar.xz", "--key-env"])
        .env(KEY_ENV, &secret)
        .env_remove(PASSPHRASE_ENV)
        .output()
        .unwrap();
    let secrets = secret.lines().nth(1).unwrap().as_bytes();
    assert!(
        output
            .stdout
            .windows(secrets.len())
            .all(|window| window != secrets)
    );
    assert!(
        output
            .stderr
            .windows(secrets.len())
            .all(|window| window != secrets)
    );
    success(output);
    let text = fs::read_to_string(project.join("build.tar.xz.minisig")).unwrap();
    key.verify(
        bytes,
        &minisign_verify::Signature::decode(&text).unwrap(),
        false,
    )
    .unwrap();
}

#[test]
#[ignore = "daily"]
fn ci_unlocks_an_encrypted_key_only_with_its_explicit_passphrase() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path();
    let passphrase = "fixture passphrase";
    let pair = minisign::KeyPair::generate_encrypted_keypair(Some(passphrase.into())).unwrap();
    let secret = pair.sk.to_box(None).unwrap().into_string();
    let bytes = b"release descriptor";
    fs::write(project.join("descriptor.json"), bytes).unwrap();
    let sign = |password: Option<&str>| {
        let mut cmd = tool(project);
        cmd.args(["sign", "descriptor.json", "--key-env"])
            .env(KEY_ENV, &secret)
            .env_remove(PASSPHRASE_ENV);
        if let Some(password) = password {
            cmd.env(PASSPHRASE_ENV, password);
        }
        let output = cmd.output().unwrap();
        for pipe in [&output.stdout, &output.stderr] {
            for value in [secret.lines().nth(1).unwrap(), passphrase] {
                assert!(
                    pipe.windows(value.len())
                        .all(|window| window != value.as_bytes())
                );
            }
        }
        output
    };
    for password in [None, Some("not the passphrase")] {
        let output = sign(password);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(PASSPHRASE_ENV));
        assert!(!project.join("descriptor.json.minisig").exists());
    }
    success(sign(Some(passphrase)));
    let key = minisign_verify::PublicKey::from_base64(&pair.pk.to_base64()).unwrap();
    let signature = fs::read_to_string(project.join("descriptor.json.minisig")).unwrap();
    key.verify(
        bytes,
        &minisign_verify::Signature::decode(&signature).unwrap(),
        false,
    )
    .unwrap();
}
