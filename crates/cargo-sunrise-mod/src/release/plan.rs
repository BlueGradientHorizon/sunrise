//! One release decision and its verified SDK metadata, before any build or signing job runs.
use super::{AssetName, releases, version};
use crate::{bounded, internals::atomic_write, options::Options, sdk_fetch};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};
use sunrise_mod_package::{
    index::{
        self, SunriseRelease,
        registry::{self, builds},
    },
    sdk,
};
use sunrise_native_build::{api, prebuilt};
use sunrise_release_sign::PublicKey;

pub(crate) const USAGE: &str = "release plan --releases <file> --sdk-data <new-dir> --out <file> (--tag <vVERSION> | --registry <checkout> --index <file> --id <mod-id> --repo <owner/name> [--version <version> --target <stable-tag>])";

#[derive(Debug, Serialize)]
struct Output<'a> {
    schema: u32,
    tool_commit: &'static str,
    plan: Option<Plan<'a>>,
}

#[derive(Debug, Serialize)]
struct Plan<'a> {
    #[serde(flatten)]
    release: Selection<'a>,
    rust: String,
    hosts: Vec<Host>,
    descriptor: Option<String>,
    source_asset: String,
}

#[derive(Debug, Serialize)]
struct Selection<'a> {
    id: String,
    version: String,
    author_tag: String,
    source: &'a SunriseRelease,
    target: &'a SunriseRelease,
    #[serde(flatten)]
    mode: Mode,
}

#[derive(Debug, Serialize)]
#[serde(tag = "mode", rename_all = "kebab-case")]
enum Mode {
    SourceOnly { reason: String },
    Initial,
    Rebuild { reviewed_source: ReviewedSource },
}

#[derive(Debug, Serialize)]
struct ReviewedSource {
    url: String,
    sha256: String,
    size: u64,
    key: String,
    signature: String,
    owners: Vec<String>,
    owner_ids: Vec<u64>,
}

#[derive(Debug, Serialize)]
struct Host {
    name: String,
    triple: String,
    runner: &'static str,
    side: sdk::Side,
    archive: Option<String>,
    source_producer: bool,
}

#[derive(Clone, Copy)]
struct Rebuild<'a> {
    id: &'a str,
    repo: &'a str,
    pair: Option<(&'a str, &'a str)>,
}

pub(crate) fn run(project: &Path, args: &[String]) -> Result<(), String> {
    let Some((command, args)) = args.split_first() else {
        return Err(format!("use `cargo sunrise-mod {USAGE}`"));
    };
    if command != "plan" {
        return Err(format!(
            "unknown release command {command:?}; use `cargo sunrise-mod {USAGE}`"
        ));
    }
    if crate::help::asked(args) {
        print!(
            "{}",
            crate::help::of("release").expect("release has a help entry")
        );
        return Ok(());
    }
    let mut flags = Options::new(
        "release plan",
        args,
        &[
            "--releases",
            "--sdk-data",
            "--out",
            "--tag",
            "--registry",
            "--index",
            "--id",
            "--repo",
            "--version",
            "--target",
        ],
    )
    .usage(USAGE);
    let mut one = |name: &str| match flags.values(name)?.as_slice() {
        [] => Ok(None),
        [value] => Ok(Some(*value)),
        _ => Err(format!("release plan takes {name} only once")),
    };
    let list = one("--releases")?.ok_or("release plan needs --releases <file>")?;
    let data = one("--sdk-data")?.ok_or("release plan needs --sdk-data <new-dir>")?;
    let out = one("--out")?.ok_or("release plan needs --out <file>")?;
    let tag = one("--tag")?;
    let registry = one("--registry")?;
    let published = one("--index")?;
    let id = one("--id")?;
    let repo = one("--repo")?;
    let wanted = one("--version")?;
    let target = one("--target")?;
    flags.finish()?;
    let releases = releases(Path::new(list))?;
    let selection = match (tag, registry, published, id, repo, wanted, target) {
        (Some(tag), None, None, None, None, None, None) => Some(initial(project, tag, &releases)?),
        (None, Some(checkout), Some(published), Some(id), Some(repo), version, target)
            if version.is_some() == target.is_some() =>
        {
            if !registry::valid_repo(repo) {
                return Err(format!("{repo:?}: expected --repo owner/name"));
            }
            let checkout =
                registry::load(Path::new(checkout)).map_err(|errors| errors.join("\n"))?;
            let published =
                index::Index::parse(&bounded::read(Path::new(published), index::MAX_INDEX)?)?;
            rebuild(
                &checkout,
                &published,
                &releases,
                Rebuild {
                    id,
                    repo,
                    pair: version.zip(target),
                },
            )?
        }
        _ => return Err(format!("use `cargo sunrise-mod {USAGE}`")),
    };
    stage(
        selection,
        &sdk_fetch::Base::from_env(),
        &sdk_fetch::keys(),
        Path::new(data),
        Path::new(out),
    )
}

fn initial<'a>(
    project: &Path,
    tag: &str,
    releases: &'a [SunriseRelease],
) -> Result<Selection<'a>, String> {
    let bundle =
        sunrise_native_build::bundle_in(project)?.ok_or("release plan needs a native mod")?;
    let version = version(&bundle.manifest)?;
    if tag != format!("v{version}") {
        return Err(format!(
            "tag {tag:?} must match the manifest version: v{version}"
        ));
    }
    let native = bundle.native();
    let mut named = releases.iter().filter(|r| {
        r.commit == native.sunrise
            && index::stable_tag(&r.tag).expect("validated release tag").0 == native.game
    });
    let Some(source) = named.next() else {
        return Err(format!(
            "the mod pins {}, which is not a published stable release for Minecraft {}",
            native.sunrise, native.game
        ));
    };
    if let Some(other) = named.next() {
        return Err(format!(
            "the mod's pin names both Sunrise {} and {}; it must name exactly one stable release",
            source.tag, other.tag
        ));
    }
    let project = api::Project::read(&bundle).map_err(|e| e.to_string())?;
    let mode = match prebuilt::release_project(&project) {
        Ok(()) => {
            match bounded::read_bytes(
                &bundle.dir.join("Cargo.lock"),
                16 << 20,
                bounded::Symlinks::Refuse,
            ) {
                Ok(lock) => prebuilt::check_release_lock(&lock)?,
                Err(e) if e.is_not_found() => (), // The initial build can write a missing lock without updating one.
                Err(e) => return Err(e.into()),
            }
            Mode::Initial
        }
        Err(reason) => Mode::SourceOnly { reason },
    };
    Ok(Selection {
        id: bundle.manifest.id,
        version,
        author_tag: tag.to_owned(),
        source,
        target: source,
        mode,
    })
}

fn rebuild<'a>(
    checkout: &registry::Checkout,
    published: &index::Index,
    releases: &'a [SunriseRelease],
    request: Rebuild<'_>,
) -> Result<Option<Selection<'a>>, String> {
    let Rebuild { id, repo, pair } = request;
    let no_pair = || match pair {
        Some((version, target)) => Err(format!(
            "{id} {version}: no eligible reviewed binary rebuild for Sunrise {target}"
        )),
        None => Ok(None),
    };
    let Some(entry) = checkout.entries.iter().find(|entry| entry.id == id) else {
        return no_pair();
    };
    let Some(key) = entry.key.as_deref().map(str::trim) else {
        return no_pair();
    };
    let Some(project) = published.project(id) else {
        return no_pair();
    };
    if entry.kind != index::Kind::NativeMod
        || project.kind != entry.kind
        || project.key.as_deref() != Some(key)
    {
        return no_pair();
    }

    let mut versions = entry
        .versions
        .iter()
        .filter_map(|row| {
            let version = sunrise_mod_package::manifest::parse_version(&row.version).ok()?;
            (row.version == version.to_string() && pair.is_none_or(|(want, _)| want == row.version))
                .then_some((version, row))
        })
        .collect::<Vec<_>>();
    versions.sort_unstable_by(|(a, _), (b, _)| b.cmp(a));

    // The schedule only considers each line's current tip. Once covered it does not work
    // backwards through past releases; a historical target requires an explicit dispatch.
    let compare = |release: &SunriseRelease| {
        sunrise_mod_package::manifest::parse_version(
            release
                .tag
                .split_once('-')
                .expect("validated release tag")
                .1,
        )
        .expect("validated release version")
    };
    let by_tag: BTreeMap<_, _> = releases
        .iter()
        .map(|r| (r.tag.as_str(), (compare(r), r)))
        .collect();
    let mut tips = BTreeMap::new();
    if pair.is_none() {
        for ranked in by_tag.values() {
            let tip = tips.entry(ranked.1.line.as_str()).or_insert(ranked);
            if ranked.0 > tip.0 {
                *tip = ranked;
            }
        }
    }
    let explicit = pair
        .map(|(_, tag)| {
            by_tag
                .get(tag)
                .ok_or_else(|| format!("Sunrise {tag:?} is not in --releases"))
        })
        .transpose()?;

    for (_, row) in versions {
        let (Some(source_tag), Some(signature)) = (&row.sunrise, &row.signature) else {
            continue;
        };
        if row.builds.is_empty()
            || checkout.revoked.iter().any(|r| r.covers(id, &row.sha256))
            || published.revoked(id, &row.sha256).is_some()
        {
            continue;
        }
        let Some(listed) = project.versions.iter().find(|v| {
            v.version == row.version
                && v.game == row.game
                && v.sunrise.as_ref() == Some(source_tag)
                && v.file.sha256 == row.sha256
                && v.file.signature.as_ref() == Some(signature)
                && !v.native_binaries.is_empty()
        }) else {
            continue;
        };
        let Ok((source_repo, author_tag, asset)) = entry.stable_source_asset(row) else {
            continue;
        };
        if !source_repo.eq_ignore_ascii_case(repo) || asset != listed.file.name {
            continue;
        }
        let Some((source_version, source)) = by_tag.get(source_tag.as_str()) else {
            continue;
        };
        if !published
            .releases
            .as_deref()
            .unwrap_or_default()
            .contains(source)
        {
            continue; // The reviewed source's stable tag must still resolve to the published pin.
        }
        let Some((target_version, target)) =
            explicit.or_else(|| tips.get(source.line.as_str()).copied())
        else {
            continue;
        };
        let covers_target = |tag: &str| {
            tag == target.tag
                || by_tag
                    .get(tag)
                    .is_some_and(|(_, r)| r.commit == target.commit)
        };
        if source.line != target.line
            || target_version <= source_version
            || target.commit == source.commit
            || row.builds.iter().any(|row| covers_target(&row.sunrise))
            || listed
                .native_binaries
                .iter()
                .any(|build| build.build == target.commit)
            || listed.verdicts.iter().any(|(tag, verdict)| {
                matches!(verdict.fit, index::FitVerdict::Breaks { .. }) && covers_target(tag)
            })
        {
            continue;
        }
        return Ok(Some(Selection {
            id: id.to_owned(),
            version: row.version.clone(),
            author_tag: author_tag.to_owned(),
            source,
            target,
            mode: Mode::Rebuild {
                reviewed_source: ReviewedSource {
                    url: row.file.clone(),
                    sha256: row.sha256.clone(),
                    size: listed.file.size,
                    key: key.to_owned(),
                    signature: signature.clone(),
                    owners: entry.owners.clone(),
                    owner_ids: entry.owner_ids.clone(),
                },
            },
        }));
    }
    no_pair()
}

/// Resolve aliases in existing parents, then compare the names that the write will create.
/// A future SDK subdirectory does not exist yet, so canonicalizing the whole path would miss it.
fn output_identity(path: &Path) -> Result<PathBuf, String> {
    let invalid = |error| format!("{}: {error}", path.display());
    let absolute = std::path::absolute(path).map_err(invalid)?;
    let leaf = absolute
        .file_name()
        .ok_or_else(|| format!("{}: name an output", path.display()))?;
    let parent = absolute
        .parent()
        .expect("an absolute path with a file name has a parent");
    for ancestor in parent.ancestors() {
        match dunce::canonicalize(ancestor) {
            Ok(mut resolved) => {
                let suffix = parent.strip_prefix(ancestor).expect("from ancestors");
                for part in suffix.components() {
                    match part {
                        Component::ParentDir => {
                            resolved.pop();
                        }
                        Component::Normal(part) => resolved.push(part),
                        Component::CurDir => (),
                        _ => unreachable!("a relative suffix"),
                    }
                }
                resolved.push(leaf);
                #[cfg(windows)]
                let resolved = resolved
                    .components()
                    .map(|part| {
                        part.as_os_str()
                            .to_string_lossy()
                            .trim_end_matches(['.', ' '])
                            .to_uppercase()
                    })
                    .collect();
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(invalid(error)),
        }
    }
    Err(format!("{}: no existing parent", path.display()))
}

fn stage(
    selection: Option<Selection<'_>>,
    base: &sdk_fetch::Base,
    keys: &[PublicKey],
    data: &Path,
    out: &Path,
) -> Result<(), String> {
    if output_identity(out)?.starts_with(output_identity(data)?) {
        return Err("--out must be outside --sdk-data".into());
    }
    let (mut output, mut staged) = (
        Output {
            schema: 1,
            tool_commit: env!("SUNRISE_TOOL_BUILT_FROM"),
            plan: None,
        },
        None,
    );
    if let Some(selection) = selection {
        let data = super::output_dir(data)?;
        match fs::symlink_metadata(&data) {
            Ok(_) => {
                return Err(format!(
                    "{} must be a new SDK-data directory",
                    data.display()
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(format!("{}: {e}", data.display())),
        }
        let parent = data
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let stage =
            tempfile::tempdir_in(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        let with_build_data = !matches!(selection.mode, Mode::SourceOnly { .. });
        let fetch = |r: &SunriseRelease| {
            let sdk =
                sdk_fetch::release_data(base, keys, &r.commit, stage.path(), with_build_data)?;
            let game = index::stable_tag(&r.tag).expect("validated release tag").0;
            if sdk.build.game != game {
                return Err(format!(
                    "Sunrise {} has a signed SDK for Minecraft {}, not {game}",
                    r.tag, sdk.build.game
                ));
            }
            Ok(sdk)
        };
        if selection.source.commit != selection.target.commit {
            fetch(selection.source)?;
        }
        let sdk = fetch(selection.target)?;
        output.plan = Some(complete(selection, sdk)?);
        staged = Some((stage, data));
    }
    let bytes = serde_json::to_vec_pretty(&output).map_err(|e| e.to_string())?;
    if let Some((stage, data)) = &staged {
        fs::rename(stage.path(), data).map_err(|e| format!("{}: {e}", data.display()))?;
    }
    if let Err(error) = atomic_write(out, bytes) {
        if let Some((_, data)) = staged {
            let _ = fs::remove_dir_all(data);
        }
        return Err(error);
    }
    Ok(())
}

fn complete(selection: Selection<'_>, sdk: sdk::Index) -> Result<Plan<'_>, String> {
    let sources_only = matches!(selection.mode, Mode::SourceOnly { .. });
    let rebuild = matches!(selection.mode, Mode::Rebuild { .. });
    let id = &selection.id;
    let version = &selection.version;
    let tag = &selection.target.tag;
    let AssetName(source_asset) = AssetName::checked(format!("{id}-{version}.smod"))?;
    let descriptor = (!sources_only)
        .then(|| AssetName::descriptor(id, version, tag).map(|name| name.0))
        .transpose()?;
    let mut hosts = builds::hosts(&sdk)?
        .into_iter()
        .map(|name| {
            let (triple, side) = name
                .strip_suffix("-server")
                .map_or((name.as_str(), sdk::Side::Client), |t| {
                    (t, sdk::Side::Server)
                });
            // The released game SDK and rolling signed mod-tool both use these native toolchains.
            let runner = match triple {
                "x86_64-unknown-linux-gnu" => "ubuntu-22.04",
                "x86_64-pc-windows-gnu" => "windows-latest",
                "aarch64-apple-darwin" => "macos-15",
                _ => return Err(format!("no release workflow runner for SDK target {name}")),
            };
            let archive = (!sources_only)
                .then(|| AssetName::archive(id, version, tag, &name).map(|name| name.0))
                .transpose()?;
            Ok(Host {
                triple: triple.to_owned(),
                name,
                runner,
                side,
                archive,
                source_producer: false,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if !rebuild {
        let source_host = hosts
            .iter()
            .position(|h| h.side == sdk::Side::Client && h.runner == "ubuntu-22.04")
            .or_else(|| hosts.iter().position(|h| h.side == sdk::Side::Client))
            .ok_or("the SDK has no client host to make the source package")?;
        if sources_only {
            hosts = vec![hosts.swap_remove(source_host)];
            hosts[0].source_producer = true;
        } else {
            hosts[source_host].source_producer = true;
        }
    }
    Ok(Plan {
        release: selection,
        rust: sdk.toolchain.rust,
        hosts,
        descriptor,
        source_asset,
    })
}

#[cfg(test)]
mod tests;
