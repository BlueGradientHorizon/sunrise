//! Author-held Minisign keys and detached signatures; unrelated to Sunrise's SDK release key.
use crate::{
    bounded::{self, Symlinks},
    options::Options,
    release,
};
use minisign::{KeyPair, PublicKey, SecretKey};
use std::{env, fs, io, io::Write, path::Path};

const KEY_NAME: &str = "signing.key";
const PUBLIC_NAME: &str = "signing.pub";
const KEY_ENV: &str = "SUNRISE_MOD_SIGNING_KEY";
const PASSPHRASE_ENV: &str = "SUNRISE_MOD_SIGNING_PASSPHRASE";
const KEY_LIMIT: u64 = 16 << 10;

pub(crate) fn keygen(args: &[String]) -> Result<(), String> {
    let mut options = Options::new("keygen", args, &["--out"]);
    let out = options.value("--out")?.unwrap_or(".sunrise");
    let no_password = options.switch("--no-password");
    options.finish()?;
    let out = release::output_dir(Path::new(out))?;
    let secret = out.join(KEY_NAME);
    let public = out.join(PUBLIC_NAME);
    require_absent(&secret)?;
    require_absent(&public)?;
    let pair = if no_password {
        KeyPair::generate_unencrypted_keypair()
    } else {
        let password = rpassword::prompt_password("Minisign passphrase: ")
            .map_err(|e| format!("could not read the passphrase: {e}"))?;
        if password.is_empty() {
            return Err("the passphrase must not be empty; use --no-password only if you intend to store an unencrypted key".into());
        }
        let again = rpassword::prompt_password("Repeat the Minisign passphrase: ")
            .map_err(|e| format!("could not read the passphrase: {e}"))?;
        if password != again {
            return Err("the passphrases do not match".into());
        }
        KeyPair::generate_encrypted_keypair(Some(password))
    }
    .map_err(|_| "could not generate a Minisign key pair")?;
    let secret_box = pair
        .sk
        .to_box(None)
        .map_err(|_| "could not encode the new secret key")?;
    let public_box = pair
        .pk
        .to_box()
        .map_err(|_| "could not encode the new public key")?;
    fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    write_new(&secret, &secret_box.to_bytes())?;
    write_new(&public, &public_box.to_bytes())
        .map_err(|e| format!("{e}; the private key was saved at {}", secret.display()))?;
    eprintln!(
        "Wrote {} and {}. Keep the private key out of Git and release assets.",
        secret.display(),
        public.display()
    );
    println!("{}", pair.pk.to_base64());
    Ok(())
}

pub(crate) fn sign(args: &[String]) -> Result<(), String> {
    let mut options = Options::new("sign", args, &["--key"]);
    let key_file = options.value("--key")?;
    let from_env = options.switch("--key-env");
    let no_password = options.switch("--no-password");
    let file = options.at_most_one()?;
    options.finish()?;
    let file = Path::new(file.ok_or_else(|| crate::help::of("sign").expect("sign has help"))?);
    if from_env && (key_file.is_some() || no_password) {
        return Err("--key-env cannot be combined with --key or --no-password".into());
    }
    let name = file.file_name().ok_or("name a file to sign")?;
    let parent = file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let into = release::output_dir(parent)?;
    let mut sidecar = name.to_os_string();
    sidecar.push(".minisig");
    let sidecar = into.join(sidecar);
    require_absent(&sidecar)?;
    let bytes = bounded::read_bytes(file, sunrise_mod_package::index::MAX_FILE, Symlinks::Refuse)?;
    let secret = if from_env {
        let text =
            env::var(KEY_ENV).map_err(|_| format!("--key-env needs a text key in {KEY_ENV}"))?;
        if text.len() as u64 > KEY_LIMIT {
            return Err(format!("{KEY_ENV} is too large for a Minisign key"));
        }
        match env::var(PASSPHRASE_ENV) {
            Ok(password) if !password.is_empty() => {
                SecretKey::from_box(text.into(), Some(password))
            }
            Err(env::VarError::NotPresent) => SecretKey::from_unencrypted_box(text.into()),
            _ => return Err(format!("{PASSPHRASE_ENV} must be nonempty text when set")),
        }
        .map_err(|_| {
            format!(
                "could not unlock the Minisign key in {KEY_ENV}; check the key and {PASSPHRASE_ENV}"
            )
        })?
    } else {
        let key_path = Path::new(key_file.unwrap_or(".sunrise/signing.key"));
        let text = String::from_utf8(bounded::read_bytes(key_path, KEY_LIMIT, Symlinks::Refuse)?)
            .map_err(|_| format!("{}: expected Minisign key text", key_path.display()))?;
        let boxed = text.into();
        if no_password {
            SecretKey::from_unencrypted_box(boxed)
        } else {
            let password = rpassword::prompt_password("Minisign passphrase: ")
                .map_err(|e| format!("could not read the passphrase: {e}"))?;
            SecretKey::from_box(boxed, Some(password))
        }
        .map_err(|_| format!("{}: could not unlock the Minisign key; use --no-password for a passwordless key file", key_path.display()))?
    };
    let public = PublicKey::from_secret_key(&secret).map_err(|_| "invalid Minisign signing key")?;
    let signature = minisign::sign(Some(&public), &secret, bytes.as_slice(), None, None)
        .map_err(|_| "could not make the Minisign signature")?;
    write_new(&sidecar, &signature.to_bytes())?;
    println!("Wrote {}", sidecar.display());
    Ok(())
}

/// Refuse an existing name before prompting, including a dangling symlink. `write_new` enforces
/// the same no-replace policy at the write in case another process creates the name meanwhile.
fn require_absent(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(format!(
            "{} already exists; not overwritten",
            path.display()
        )),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// A private temporary file (0600 on Unix) becomes visible at the requested name only once complete.
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().expect("output inside a folder");
    let mut temp =
        tempfile::NamedTempFile::new_in(parent).map_err(|e| format!("{}: {e}", path.display()))?;
    temp.write_all(bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    temp.persist_noclobber(path)
        .map_err(|e| format!("{}: {}", path.display(), e.error))?;
    Ok(())
}
