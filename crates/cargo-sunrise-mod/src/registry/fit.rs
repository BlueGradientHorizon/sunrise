//! Publish fit from workflow-supplied SDK files. This route never fetches a build or runs a mod.
use crate::sdk_cmds::{Subject, compare_fit, compare_game, package_subject};
use crate::sdk_fetch::{BUILD_DATA, SURFACE};
use mod_surface::{Surface, uses::Fit};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use sunrise_mod_package::{
    index::{BuildVerdict, FitVerdict, Index, Kind},
    sdk,
};
use sunrise_native_build::game_uses;

struct Build {
    sdk: sdk::Index,
    surface: Surface,
    surface_bytes: Vec<u8>,
}

impl Build {
    fn read(directory: &Path, commit: &str) -> Result<Self, String> {
        let path = directory.join(sdk::INDEX);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let sdk =
            sdk::Index::read(&bytes, commit).map_err(|e| format!("{}: {e}", path.display()))?;
        let path = directory.join(SURFACE);
        let surface_bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        sdk.verify(SURFACE, surface_bytes.as_slice())
            .map_err(|e| e.to_string())?;
        let surface =
            Surface::read(&surface_bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self {
            sdk,
            surface,
            surface_bytes,
        })
    }
}

/// Group release tags by commit: the Nightly and a stable release can share a build.
pub(super) fn publish(
    index: &mut Index,
    files: &Path,
    sdk_data: Option<&Path>,
) -> Result<(), String> {
    let releases = index.releases.as_deref().unwrap_or_default();
    let mut targets = BTreeMap::<&str, Vec<(&str, Option<&str>)>>::new();
    for release in releases {
        targets
            .entry(&release.commit)
            .or_default()
            .push((&release.tag, Some(&release.line)));
    }
    if let Some(nightly) = &index.nightly_build {
        targets
            .entry(&nightly.commit)
            .or_default()
            .push(("nightly", None));
    }
    let Some((&first_commit, first_keys)) = targets.first_key_value() else {
        return Ok(());
    };
    let sdk_data = sdk_data.ok_or_else(|| {
        format!(
            "Sunrise {} ({first_commit}): give --sdk-data <dir> with the build's SDK files",
            first_keys[0].0
        )
    })?;
    let mut builds = BTreeMap::new();
    for (&commit, keys) in &targets {
        let name = keys[0].0;
        let build = Build::read(&sdk_data.join(commit), commit)
            .map_err(|error| format!("Sunrise {name} ({commit}): {error}"))?;
        builds.insert(commit, build);
    }
    let mut packages = HashMap::new();
    for (commit, keys) in targets {
        let build = &builds[commit];
        let data = game_uses::Data::read(&sdk_data.join(commit).join(BUILD_DATA), commit)
            .map_err(|error| format!("Sunrise {} ({commit}): {error}", keys[0].0))?;
        for project in &mut index.projects {
            for version in &mut project.versions {
                let Some(tag) = &version.sunrise else {
                    continue;
                };
                let made_for = releases.iter().find(|r| r.tag == *tag).ok_or_else(|| {
                    format!(
                        "{} {}: use --releases with Sunrise release {tag:?} to compute fit",
                        project.id, version.version
                    )
                })?;
                let mut applicable = keys
                    .iter()
                    .filter(|(_, line)| line.is_none_or(|line| line == made_for.line))
                    .peekable();
                if applicable.peek().is_none() {
                    continue;
                }
                let fit = if commit == made_for.commit {
                    FitVerdict::MadeFor
                } else if version.game != build.sdk.build.game {
                    rebuild("Minecraft version changed")
                } else if project.kind == Kind::NativeMod {
                    // Assembly has already verified the package. An unreadable uses list is not
                    // evidence of fit, just as a package without a list is not.
                    let (subject, baseline) = packages
                        .entry(version.file.sha256.clone())
                        .or_insert_with(|| {
                            (
                                package_subject(&files.join(&version.file.sha256)).ok(),
                                None,
                            )
                        });
                    let known = |pin: &str| {
                        builds
                            .get(pin)
                            .map(|b| b.surface_bytes.as_slice())
                            .ok_or_else(|| format!("no SDK data for {pin}"))
                    };
                    native(subject, baseline, build, &data, known)?
                } else if project.kind == Kind::Mod && version.api != build.sdk.content_api {
                    rebuild("content mod API changed")
                } else {
                    FitVerdict::Fits
                };
                let mut targets: Vec<_> = version
                    .native_binaries
                    .iter()
                    .filter(|b| b.build == commit)
                    .map(|b| b.target.clone())
                    .collect();
                targets.sort();
                let row = BuildVerdict { fit, targets };
                version
                    .verdicts
                    .extend(applicable.map(|(key, _)| ((*key).to_owned(), row.clone())));
            }
        }
    }
    Ok(())
}

fn native<'a>(
    subject: &mut Option<Subject>,
    baseline: &mut Option<Result<(), String>>,
    build: &Build,
    data: &game_uses::Data,
    baseline_surface: impl FnOnce(&str) -> Result<&'a [u8], String>,
) -> Result<FitVerdict, String> {
    let Some(Subject::Table {
        manifest, table, ..
    }) = subject
    else {
        return Ok(rebuild("made for one build only"));
    };
    let game = compare_game(manifest, table, &build.sdk.mixins.rev, || {
        Ok(data.entries(manifest))
    });
    let comparison = compare_fit(table, &build.surface, game, baseline, baseline_surface)?;
    Ok(match comparison.report.verdict {
        Fit::Fits if comparison.compared.unjudged.is_empty() && !comparison.compared.unrecorded => {
            FitVerdict::Fits
        }
        Fit::DoesNotFit(mut misfits) => {
            misfits.dedup_by(|a, b| a.id == b.id);
            FitVerdict::Rebuild {
                changed: misfits.into_iter().take(5).map(|m| m.id).collect(),
                reason: None,
            }
        }
        _ => rebuild("made for one build only"),
    })
}

fn rebuild(reason: &str) -> FitVerdict {
    FitVerdict::Rebuild {
        changed: Vec::new(),
        reason: Some(reason.to_owned()),
    }
}
