//! The standalone tool updates independently of any project's pinned game or SDK.
use crate::{dev, options::Options, sdk_fetch};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use sunrise_addresses::SDK_RELEASES;
use sunrise_release_sign::{self as release, MOD_TOOL, SIGNATURE, SUMS};
use tempfile::NamedTempFile;

const BUILD: &str = env!("SUNRISE_TOOL_BUILT_FROM");
const INFO: &str = "tool.json";
#[cfg(not(windows))]
const EXE: &str = "cargo-sunrise-mod";
#[cfg(windows)]
const EXE: &str = "cargo-sunrise-mod.exe";
const NOTICE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const NOTICE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Deserialize)]
struct Tool {
    commit: String,
}

impl Tool {
    fn read(bytes: &[u8]) -> Result<Self, String> {
        let tool: Self = serde_json::from_slice(bytes).map_err(|e| format!("{INFO}: {e}"))?;
        if !sunrise_mod_package::manifest::revision(&tool.commit) {
            return Err(format!("{INFO} does not name a full commit"));
        }
        Ok(tool)
    }

    fn differs_from(&self, current: &str) -> bool {
        self.commit != current
    }
}

/// Only the book's standalone install can be replaced. Do not rewrite a copy owned by an SDK,
/// Cargo, or a system package manager, even when the user includes it on PATH.
fn standalone_exe() -> Result<PathBuf, String> {
    let (home_var, bin) = if cfg!(windows) {
        ("USERPROFILE", ".sunrise/bin")
    } else {
        ("HOME", ".local/bin")
    };
    let exe = env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(|e| format!("could not locate the running tool: {e}"))?;
    let installed_here = env::var_os(home_var)
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .and_then(|home| home.canonicalize().ok())
        .is_some_and(|home| exe == home.join(bin).join(EXE));
    if !installed_here {
        return Err(format!(
            "{} is not in the standalone install location (${home_var}/{bin}/{EXE}); update it using its original install method",
            exe.display()
        ));
    }
    Ok(exe)
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    Options::new("self-update", args, &[]).finish()?;
    let exe = standalone_exe()?;
    let dir = exe.parent().ok_or("the tool has no install folder")?;
    let asset = format!("tool-{}.tar.xz", dev::computer()?);
    let base = sdk_fetch::Base::Url(SDK_RELEASES.to_owned());
    let info = base.read(MOD_TOOL, INFO, sdk_fetch::SMALL)?;
    let tool = Tool::read(&info)?;
    if !tool.differs_from(BUILD) {
        println!("cargo sunrise-mod is already the newest ({BUILD}).");
        return Ok(());
    }

    let sums = base.read(MOD_TOOL, SUMS, sdk_fetch::SMALL)?;
    let signature = base.read(MOD_TOOL, SIGNATURE, sdk_fetch::SMALL)?;
    let listed = release::verify(
        MOD_TOOL,
        &sums,
        &String::from_utf8_lossy(&signature),
        &sdk_fetch::release_keys(),
    )?;
    let expected = |asset: &str| {
        listed
            .iter()
            .find(|(name, _)| name == asset)
            .map(|(_, sha)| sha.as_str())
            .ok_or_else(|| format!("{MOD_TOOL}/{SUMS} does not list {asset}"))
    };
    // `tool.json` and the archive must belong to the same signed release, including during a publish.
    release::check(INFO, expected(INFO)?, &release::sha256_hex(&info))?;
    let want = expected(&asset)?;
    let mut body = base.open(MOD_TOOL, &asset)?;
    let mut archive = tempfile::tempfile_in(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let archive_error = |e| format!("{MOD_TOOL}/{asset}: {e}");
    io::copy(&mut body, &mut archive).map_err(archive_error)?;
    archive.rewind().map_err(archive_error)?;
    let mut hash = Sha256::new();
    io::copy(&mut archive, &mut hash).map_err(archive_error)?;
    release::check(&asset, want, &format!("{:x}", hash.finalize()))?;
    archive.rewind().map_err(archive_error)?;

    let mut staged = NamedTempFile::new_in(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    unpack_exe(archive, &mut staged).map_err(archive_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staged
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o755))
            .map_err(archive_error)?;
    }
    staged.as_file().sync_all().map_err(archive_error)?;
    install(staged, &exe)?;
    println!("Updated cargo sunrise-mod: {BUILD} -> {}", tool.commit);
    Ok(())
}

/// Only copy the root executable from the already verified archive. No archive paths go to disk.
fn unpack_exe(archive: impl Read, into: &mut impl Write) -> io::Result<()> {
    let xz = lzma_rust2::XzReader::new(archive, false);
    for entry in tar::Archive::new(xz).entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        if entry.header().entry_type().is_file()
            && path.strip_prefix(".").unwrap_or(&path) == Path::new(EXE)
        {
            io::copy(&mut entry, into)?;
            return Ok(());
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("archive does not contain {EXE}"),
    ))
}

fn install(staged: NamedTempFile, exe: &Path) -> Result<(), String> {
    #[cfg(not(windows))]
    let result = staged.persist(exe);
    #[cfg(windows)]
    let result = {
        // Windows cannot replace a mapped executable. Move it aside on the same volume; keep
        // just one backup, which is still mapped until this process exits.
        let previous = exe.with_file_name(".cargo-sunrise-mod.exe.old");
        fs::rename(exe, &previous).map_err(|e| format!("{}: {e}", exe.display()))?;
        let result = staged.persist_noclobber(exe);
        if let Err(error) = &result
            && let Err(restore) = fs::rename(&previous, exe)
        {
            return Err(format!(
                "could not replace {}: {error}; could not restore the old tool from {}: {restore}",
                exe.display(),
                previous.display()
            ));
        }
        result
    };
    result
        .map(drop)
        .map_err(|e| format!("could not replace {}: {}", exe.display(), e.error))
}

pub(crate) fn notice() {
    if !checks_allowed(
        env::var_os("CI").is_some(),
        env::var_os("CARGO_NET_OFFLINE").is_some_and(|v| v == "true"),
        BUILD,
    ) || standalone_exe().is_err()
    {
        return;
    }
    // Claim the day's attempt before the GET, so offline or concurrent invocations don't
    // keep hitting the network. The lock is released before any network I/O.
    let Ok(cache) = dev::cache_dir() else { return };
    if claim_notice_attempt(&cache, SystemTime::now()).is_none() {
        return;
    }
    let url = format!("{SDK_RELEASES}/{MOD_TOOL}/{INFO}");
    let check = || -> Option<Tool> {
        let agent = sunrise_proxy::agent("mod-tool-notice", &url, |builder| {
            builder
                .timeout_connect(NOTICE_TIMEOUT)
                .timeout(NOTICE_TIMEOUT)
        })
        .ok()?;
        let mut bytes = Vec::new();
        agent
            .get(&url)
            .call()
            .ok()?
            .into_reader()
            .take(sdk_fetch::SMALL + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() as u64 > sdk_fetch::SMALL {
            return None;
        }
        Tool::read(&bytes).ok()
    };
    if check().is_some_and(|tool| tool.differs_from(BUILD)) {
        eprintln!("A newer cargo sunrise-mod is out: run cargo sunrise-mod self-update");
    }
}

fn claim_notice_attempt(cache: &Path, now: SystemTime) -> Option<()> {
    fs::create_dir_all(cache).ok()?;
    let path = cache.join("mod-tool-update-check");
    match fs::File::create_new(&path) {
        Ok(_) => return Some(()), // The newly created file's mtime records the attempt.
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(_) => return None,
    }
    if !fs::symlink_metadata(&path).ok()?.is_file() {
        return None;
    }
    let stamp = fs::OpenOptions::new().write(true).open(path).ok()?;
    stamp.try_lock().ok()?;
    let last = stamp.metadata().ok()?.modified().ok()?;
    let age = now.duration_since(last).unwrap_or_default();
    if age < NOTICE_INTERVAL {
        return None;
    }
    stamp.set_modified(now).ok()
}

fn checks_allowed(ci: bool, offline: bool, build: &str) -> bool {
    !ci && !offline && build != "local"
}

#[cfg(test)]
mod tests;
