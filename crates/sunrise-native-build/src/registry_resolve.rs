//! Resolve the staged graph on the game's registry pins without downloading archives.
use super::registry_lock::RegistryLock;
use crate::host::trf;
use std::{collections::BTreeSet, fs, path::Path, process::Command};

/// Prefer the supplied locks, but let Cargo backtrack through their versions if it needs to.
/// A game pin constrains the version on its line; it does not require that line to be present.
pub(super) fn resolve(
    directory: &Path,
    game: &RegistryLock,
    seed: &RegistryLock,
    allowed: Option<&RegistryLock>,
    mut cargo: impl FnMut(&str) -> Command,
) -> Result<String, String> {
    let lock_path = directory.join("Cargo.lock");
    let original_seed = seed.seed("version=4\npackage=[]\n")?;
    let mut update = |package: Option<&str>| {
        let mut command = cargo("update");
        command.args(["--color", "never"]);
        if let Some(package) = package {
            command.args(["-p", package]);
        } else {
            command.arg("--workspace");
        }
        super::run(command, "Resolving registry dependency pins", |_| {})
    };
    let read = || {
        let text = super::read_lock(directory)?;
        let resolved = RegistryLock::parse(&text)?;
        Ok::<_, String>((text, resolved))
    };
    let check = |resolved: &RegistryLock| {
        check_game_pins(game, resolved)?;
        if let Some(allowed) = allowed {
            allowed.check_pins(resolved)?;
        }
        Ok(())
    };
    fs::write(&lock_path, &original_seed).map_err(|e| e.to_string())?;
    let mut rejected = match update(None) {
        Ok(()) => {
            let (text, resolved) = read()?;
            match check(&resolved) {
                Ok(()) => return Ok(text),
                Err(error) => Some((resolved, error)),
            }
        }
        Err(error) if seed.packages.is_empty() => return Err(error),
        Err(_) => None,
    };
    crate::check_cancelled()?;

    let work = tempfile::Builder::new()
        .prefix("sunrise-registry-unlock-")
        .tempdir_in(directory)
        .map_err(|e| e.to_string())?;
    let name = work
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid temporary registry pin directory")?;
    // `cargo update -p` unlocks the named package and stops enforcing its old transitive
    // edges, but still prefers those versions (including yanked pins). This synthetic
    // record is only in the seed, not in any manifest, so it cannot pull in dependencies.
    let pins: Vec<_> = seed
        .packages
        .keys()
        .map(|pin| format!("{} {} ({})", pin.name, pin.version, pin.source))
        .collect();
    let registry = sunrise_addresses::CARGO_CRATES_IO_SOURCE;
    let checksum = "0".repeat(64);
    let unlock = (!pins.is_empty()).then_some(name);
    let unlocked_seed = if unlock.is_some() {
        format!(
            "{original_seed}\n[[package]]\nname='{name}'\nversion='0.0.0'\nsource='{registry}'\nchecksum='{checksum}'\ndependencies={pins:?}\n"
        )
    } else {
        original_seed
    };

    let manifest_path = directory.join("Cargo.toml");
    let original = fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
    let mut manifest: toml::Table = original.parse().map_err(|e| format!("Cargo.toml: {e}"))?;
    let mut excluded = BTreeSet::new();
    let mut patched = false;
    let result = (|| {
        let mut refusal = None;
        loop {
            if let Some((resolved, error)) = rejected {
                let before = excluded.len();
                let unpinned = resolved.packages.keys().filter(|id| {
                    allowed.is_some_and(|allowed| !allowed.packages.contains_key(*id))
                });
                let patches = manifest
                    .entry("patch")
                    .or_insert_with(|| toml::Table::new().into())
                    .as_table_mut()
                    .ok_or("Invalid staged Cargo patches")?
                    .entry("crates-io")
                    .or_insert_with(|| toml::Table::new().into())
                    .as_table_mut()
                    .ok_or("Invalid staged crates.io patches")?;
                for pin in game
                    .moved_pins(&resolved)
                    .map(|(_, selected)| selected)
                    .chain(unpinned)
                {
                    if !excluded.insert(pin.clone()) {
                        continue;
                    }
                    // A patch shadows just this exact release. Give it an impossible local
                    // dependency, so Cargo must backtrack instead of selecting that release.
                    // Unlike an exact requirement on another version, this adds no root edge.
                    let key = format!("{name}-{}", excluded.len());
                    let path = work.path().join(&key);
                    fs::create_dir(&path).map_err(|e| e.to_string())?;
                    fs::write(
                        path.join("Cargo.toml"),
                        format!(
                            "[package]\nname={:?}\nversion='{}'\nedition='2024'\n[lib]\npath='../inert.rs'\n[dependencies]\nsunrise-registry-pin-excluded={{path='..',version='=0.0.1'}}\n",
                            pin.name, pin.version,
                        ),
                    ).map_err(|e| e.to_string())?;
                    patches.insert(
                        key.clone(),
                        toml::Table::from_iter([
                            ("package".into(), pin.name.clone().into()),
                            ("path".into(), format!("{name}/{key}").into()),
                        ])
                        .into(),
                    );
                }
                if excluded.len() == before {
                    // A checksum change or SDK-local conflict is a trust-boundary refusal,
                    // not a registry version choice that can be retried.
                    return Err(error);
                }
                refusal.get_or_insert(error);
                if before == 0 {
                    fs::write(work.path().join("inert.rs"), "").map_err(|e| e.to_string())?;
                    fs::write(work.path().join("Cargo.toml"), "[package]\nname='sunrise-registry-pin-excluded'\nversion='0.0.0'\nedition='2024'\n[lib]\npath='inert.rs'\n").map_err(|e| e.to_string())?;
                }
                patched = true;
                fs::write(&manifest_path, manifest.to_string()).map_err(|e| e.to_string())?;
            }
            fs::write(&lock_path, &unlocked_seed).map_err(|e| e.to_string())?;
            if let Err(error) = update(unlock) {
                crate::check_cancelled()?;
                return Err(match refusal {
                    Some(refusal) => format!("{refusal}\n\n{error}"),
                    None => error,
                });
            }
            let (text, resolved) = read()?;
            match check(&resolved) {
                Ok(()) => return Ok(text),
                Err(error) => rejected = Some((resolved, error)),
            }
        }
    })();
    if !patched {
        return result;
    }
    fs::write(&manifest_path, original).map_err(|e| e.to_string())?;
    result?;

    // Drop unused exclusion patches from the lock with the real manifest restored.
    // This pass has a complete, valid lock to preserve, not a union of mod locks.
    update(None)?;
    let (text, resolved) = read()?;
    check(&resolved)?;
    Ok(text)
}

pub(super) fn check_game_pins(game: &RegistryLock, resolved: &RegistryLock) -> Result<(), String> {
    let mut moved = game.moved_pins(resolved).peekable();
    if moved.peek().is_some() {
        Err(moved.map(|(game, selected)| {
            trf(
                "{crate}: the game locks {game}, but resolution without the game's constraint selected {selected}.",
                &[
                    ("crate", &game.name),
                    ("game", &game.version.to_string()),
                    ("selected", &selected.version.to_string()),
                ],
            )
        }).collect::<Vec<_>>().join("\n"))
    } else {
        Ok(())
    }
}
