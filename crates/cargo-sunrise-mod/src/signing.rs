//! Author-held Minisign keys and detached signatures; unrelated to Sunrise's SDK release key.
use crate::{options::Options, release};
use minisign::KeyPair;
use std::{fs, io, io::Write, path::Path};

const KEY_NAME: &str = "signing.key";
const PUBLIC_NAME: &str = "signing.pub";
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
