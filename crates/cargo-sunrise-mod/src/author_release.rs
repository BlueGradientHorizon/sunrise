//! The author-side release flow. GitHub runs the shared workflow after the tag is pushed.
use crate::{internals, options::Options, read_manifest};
use mod_surface::child::ChildEntry;
use semver::Version;
use serde::Deserialize;
use std::{
    ffi::OsStr,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use sunrise_mod_package::{Manifest, manifest::MANIFEST};
use toml_edit::DocumentMut;

pub(crate) const USAGE: &str = "release [<version>|patch|minor|major] [--check] | release setup";
const WORKFLOW: &str = "sunrise-org/sunrise-mods/.github/workflows/release.yml@release-v";
const DEFAULT_WORKFLOW: &str = "sunrise-org/sunrise-mods/.github/workflows/release.yml@release-v1";
const CALLER: &str = ".github/workflows/sunrise-release.yml";
const NOT_REGISTERED: &str =
    "This repository isn't registered yet: register it at https://playsunrise.net/mods/submit";

type Fetch<'a> = &'a dyn Fn(&str) -> Result<Option<String>, String>;

fn fetch(url: &str) -> Result<Option<String>, String> {
    match sunrise_proxy::get(url, &[("User-Agent", "cargo-sunrise-mod")], None) {
        Ok(response) => response
            .into_string()
            .map(Some)
            .map_err(|e| format!("{url}: {e}")),
        Err(e) if e.status() == Some(404) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn registry_file(fetch: Fetch<'_>, name: &str) -> Result<String, String> {
    let url = format!("{}/{name}", sunrise_addresses::MOD_REGISTRY_METADATA);
    fetch(&url)?.ok_or_else(|| format!("{url}: not found"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Watched {
    #[serde(default)]
    repository: Vec<Repository>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Repository {
    id: u64,
    name: String,
    package: Vec<WatchedPackage>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WatchedPackage {
    path: String,
    #[serde(rename = "project")]
    _project: String,
}

#[derive(Deserialize)]
struct GitHubRepository {
    id: u64,
}

fn github_name(origin: &str) -> Option<String> {
    let url;
    let path = if origin.contains("://") {
        url = url::Url::parse(origin).ok()?;
        if !matches!(url.scheme(), "https" | "ssh")
            || !url.host_str()?.eq_ignore_ascii_case("github.com")
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return None;
        }
        url.path().strip_prefix('/')?
    } else {
        let (authority, path) = origin.split_once(':')?;
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        if !host.eq_ignore_ascii_case("github.com") {
            return None;
        }
        path
    };
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.split_once('/')?;
    let valid = |part: &str| {
        !matches!(part, "" | "." | "..")
            && part
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    };
    (valid(owner) && valid(repo)).then(|| path.to_owned())
}

fn registered(fetch: Fetch<'_>, origin: &str, path: &str) -> Result<(), String> {
    let name = github_name(origin).ok_or(NOT_REGISTERED)?;
    let watched: Watched = toml::from_str(&registry_file(fetch, "watched.toml")?)
        .map_err(|e| format!("registry/watched.toml: {e}"))?;
    let repo = if let Some(repo) = watched
        .repository
        .iter()
        .find(|r| r.name.eq_ignore_ascii_case(&name))
    {
        repo
    } else {
        let url = format!("https://api.github.com/repos/{name}");
        let text = fetch(&url)?.ok_or(NOT_REGISTERED)?;
        let GitHubRepository { id } =
            serde_json::from_str(&text).map_err(|e| format!("{url}: {e}"))?;
        watched
            .repository
            .iter()
            .find(|r| r.id == id)
            .ok_or(NOT_REGISTERED)?
    };
    if !repo.package.iter().any(|p| p.path == path) {
        return Err(NOT_REGISTERED.into());
    }
    Ok(())
}

fn workflow(fetch: Fetch<'_>) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Workflow {
        uses: String,
    }
    let Workflow { uses } = toml::from_str(&registry_file(fetch, "release-workflow.toml")?)
        .map_err(|e| format!("registry/release-workflow.toml: {e}"))?;
    if !uses
        .strip_prefix(WORKFLOW)
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(format!(
            "registry/release-workflow.toml: expected uses = \"{WORKFLOW}<number>\""
        ));
    }
    Ok(uses)
}

fn caller(prefix: &str, path: &str, uses: &str) -> String {
    // Ordinary paths stay plain YAML. Quote strings that YAML might coerce, strip or interpret.
    let path = if path == "."
        || (path.starts_with(|c: char| c.is_ascii_alphabetic())
            && path
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.-/".contains(&c))
            && !matches!(
                path.to_ascii_lowercase().as_str(),
                "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "y" | "n"
            )) {
        path.into()
    } else {
        serde_json::json!(path).to_string()
    };
    format!(
        "\
# Written by `cargo sunrise-mod release`, which rewrites it: edits here are replaced.
name: Sunrise release
on:
  push:
    tags: ['{prefix}v*']
permissions: {{}}
jobs:
  release:
    permissions:
      contents: write
      id-token: write
      attestations: write
    uses: {uses}
    with:
      path: {path}
"
    )
}

pub(crate) fn scaffold(root: &Path) -> Result<(), String> {
    Change::write(CALLER, caller("", ".", DEFAULT_WORKFLOW)).apply(root)
}

/// A package and its caller are both addressed relative to the Git root.
struct Project {
    root: PathBuf,
    package: PathBuf,
    path: String,
    prefix: String,
    caller: PathBuf,
    manifest: Manifest,
}

impl Project {
    fn read(package: &Path, git: &Git) -> Result<Self, String> {
        let root = git.text(package, ["rev-parse", "--show-toplevel"])?;
        let canonical =
            |path: &Path| dunce::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()));
        let root = canonical(Path::new(&root))?;
        let package = canonical(package)?;
        let relative = package
            .strip_prefix(&root)
            .map_err(|_| "the package must be in the Git working tree")?;
        let mut pieces = Vec::new();
        for part in relative.iter() {
            pieces.push(part.to_str().ok_or("the package path must be UTF-8")?);
        }
        let manifest = read_manifest(&package)?;
        let (path, prefix, caller) = if pieces.is_empty() {
            (".".into(), String::new(), CALLER.into())
        } else {
            let path = pieces.join("/");
            if path.contains("${{") || path.contains('\\') {
                return Err(
                    "the package path cannot be passed literally to the release workflow".into(),
                );
            }
            let id = &manifest.id;
            (
                path,
                format!("{id}/"),
                format!(".github/workflows/sunrise-release-{id}.yml").into(),
            )
        };
        let package = relative.to_path_buf();
        Ok(Self {
            root,
            package,
            path,
            prefix,
            caller,
            manifest,
        })
    }

    fn workflow_changes(&self, uses: &str) -> Result<Vec<Change>, String> {
        let root = &self.root;
        for folder in [".github", ".github/workflows"] {
            internals::check_output_dir(&root.join(folder))?;
        }
        let mut changes = vec![Change::write(
            &self.caller,
            caller(&self.prefix, &self.path, uses),
        )];
        // These names aren't reserved. Only remove files with the old templates' signatures.
        let workflows = [
            ("release.yml", "Release native mod"),
            ("rebuild.yml", "Rebuild reviewed native mod source"),
            (
                "sunrise-release-jobs.yml",
                "Build and publish a planned mod release",
            ),
        ];
        for (file, name) in workflows {
            let path = Path::new(".github/workflows").join(file);
            let title = format!("name: {name}");
            if legacy_file(
                &root.join(&path),
                &[&title, "./.github/actions/sunrise-release-tool"],
            )? {
                changes.push(Change::remove(path, false));
            }
        }
        let actions = Path::new(".github/actions");
        let action = actions.join("sunrise-release-tool");
        let script = action.join("action.yml");
        if metadata(&root.join(actions))?.is_some_and(|meta| meta.is_dir())
            && metadata(&root.join(action))?.is_some_and(|meta| meta.is_dir())
            && legacy_file(
                &root.join(&script),
                &[
                    "Install the verified Sunrise release tool",
                    "SUNRISE_RELEASE_SCRIPTS",
                ],
            )?
        {
            changes.push(Change::remove(script, true));
        }
        for (file, signature) in [
            (
                "sunrise_release.py",
                "Build, sign and stage a native mod release",
            ),
            (
                "sunrise_release_github.py",
                "from sunrise_release import Batch",
            ),
        ] {
            let repo_path = PathBuf::from(file);
            let nested_path =
                (!self.package.as_os_str().is_empty()).then(|| self.package.join(file));
            for path in std::iter::once(repo_path).chain(nested_path) {
                if legacy_file(&root.join(&path), &[signature])? {
                    changes.push(Change::remove(path, false));
                }
            }
        }
        Ok(changes)
    }

    fn version_changes(&self, git: &Git, version: &Version) -> Result<Vec<Change>, String> {
        let version = version.to_string();
        let path = self.package.join(MANIFEST);
        let mut mod_doc = read_tracked_toml(git, &self.root, &path)?;
        internals::set_string(
            mod_doc["version"]
                .as_value_mut()
                .expect("parsed manifest version"),
            &version,
        );
        let mut changes = vec![Change::write(path, mod_doc.to_string())];
        if self.manifest.native.is_none() {
            return Ok(changes);
        }
        let cargo_path = self.package.join("Cargo.toml");
        let lock_path = self.package.join("Cargo.lock");
        let mut cargo = read_tracked_toml(git, &self.root, &cargo_path)?;
        let mut lock = read_tracked_toml(git, &self.root, &lock_path)?;
        let package = cargo
            .get_mut("package")
            .and_then(|p| p.as_table_like_mut())
            .ok_or("Cargo.toml: no [package] table")?;
        let name = package
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("Cargo.toml: no package name")?
            .to_owned();
        let value = package
            .get_mut("version")
            .and_then(|v| v.as_value_mut())
            .ok_or("Cargo.toml: [package] must specify a version")?;
        let old = value
            .as_str()
            .ok_or("Cargo.toml: [package] must specify a version string")?;
        let packages = lock
            .get_mut("package")
            .and_then(|p| p.as_array_of_tables_mut())
            .ok_or("Cargo.lock: no package entries")?;
        // A root package entry has no source. Update it without running Cargo's resolver or
        // rewriting the checksums/versions of the SDK and crates.io dependencies.
        {
            let mut entries = packages.iter_mut().filter(|p| {
                p.get("name").and_then(|v| v.as_str()) == Some(&name)
                    && p.get("version").and_then(|v| v.as_str()) == Some(old)
                    && !p.contains_key("source")
            });
            let own = entries.next().ok_or_else(|| format!("Cargo.lock: no local entry for {name} {old}; refresh Cargo.lock before releasing"))?;
            if entries.next().is_some() {
                return Err(format!(
                    "Cargo.lock: more than one local entry for {name} {old}"
                ));
            }
            internals::set_string(
                own["version"]
                    .as_value_mut()
                    .expect("matched version string"),
                &version,
            );
        }
        // Cargo qualifies a dependency by version when several locked packages share a name.
        let old_id = format!("{name} {old}");
        let new_id = format!("{name} {version}");
        for p in packages.iter_mut() {
            if let Some(deps) = p.get_mut("dependencies").and_then(|v| v.as_array_mut()) {
                for dep in deps.iter_mut().filter(|v| v.as_str() == Some(&old_id)) {
                    internals::set_string(dep, &new_id);
                }
            }
        }
        internals::set_string(value, &version);
        changes.push(Change::write(cargo_path, cargo.to_string()));
        changes.push(Change::write(lock_path, lock.to_string()));
        Ok(changes)
    }
}

fn metadata(path: &Path) -> Result<Option<fs::Metadata>, String> {
    match fs::symlink_metadata(path) {
        Ok(meta) => Ok(Some(meta)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn legacy_file(path: &Path, signatures: &[&str]) -> Result<bool, String> {
    if !metadata(path)?.is_some_and(|m| m.is_file()) {
        return Ok(false);
    }
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(std::str::from_utf8(&bytes).is_ok_and(|text| signatures.iter().all(|&s| text.contains(s))))
}

fn read_tracked_toml(git: &Git, root: &Path, relative: &Path) -> Result<DocumentMut, String> {
    let path = root.join(relative);
    if !metadata(&path)?.is_some_and(|m| m.is_file()) || !git.tracked(root, relative)? {
        return Err(format!(
            "{} must be a committed regular file before releasing",
            relative.display()
        ));
    }
    fs::read_to_string(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .parse()
        .map_err(|e| format!("{}: {e}", path.display()))
}

struct Change {
    path: PathBuf,
    kind: Kind,
}

enum Kind {
    Write(String),
    Remove { prune_parent: bool },
}

impl Change {
    fn write(path: impl Into<PathBuf>, text: String) -> Self {
        Self {
            path: path.into(),
            kind: Kind::Write(text),
        }
    }

    fn remove(path: PathBuf, prune_parent: bool) -> Self {
        Self {
            path,
            kind: Kind::Remove { prune_parent },
        }
    }

    fn apply(&self, root: &Path) -> Result<(), String> {
        let path = root.join(&self.path);
        let parent = path.parent().expect("path in checkout");
        match &self.kind {
            Kind::Write(text) => {
                fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
                internals::atomic_write(&path, text)
            }
            Kind::Remove { prune_parent } => {
                if let Err(e) = fs::remove_file(&path)
                    && e.kind() != ErrorKind::NotFound
                {
                    return Err(format!("{}: {e}", path.display()));
                }
                if *prune_parent
                    && let Err(e) = fs::remove_dir(parent)
                    && !matches!(e.kind(), ErrorKind::NotFound | ErrorKind::DirectoryNotEmpty)
                {
                    return Err(format!("{}: {e}", parent.display()));
                }
                Ok(())
            }
        }
    }

    fn describe(&self) {
        let verb = match self.kind {
            Kind::Write(_) => "Write",
            Kind::Remove { .. } => "Remove",
        };
        println!("  {verb} {}", self.path.display());
    }
}

#[derive(Default)]
struct Git {
    #[cfg(test)]
    env: Vec<(&'static str, std::ffi::OsString)>,
}

impl Git {
    fn command(&self, root: &Path) -> Command {
        let mut cmd = Command::new("git");
        cmd.current_dir(root);
        #[cfg(test)]
        cmd.envs(self.env.iter().map(|(key, value)| (*key, value)));
        cmd
    }

    fn text(
        &self,
        root: &Path,
        args: impl IntoIterator<Item: AsRef<OsStr>>,
    ) -> Result<String, String> {
        let mut cmd = self.command(root);
        cmd.args(args);
        let Output {
            status,
            stdout,
            stderr,
        } = cmd.output().map_err(|e| format!("git: {e}"))?;
        if !status.success() {
            let operation = cmd.get_args().next().unwrap_or_default().to_string_lossy();
            return Err(format!(
                "git {operation}: {}",
                String::from_utf8_lossy(&stderr).trim_end()
            ));
        }
        let stdout = String::from_utf8(stdout).map_err(|e| format!("git: {e}"))?;
        Ok(stdout
            .strip_suffix("\r\n")
            .or_else(|| stdout.strip_suffix('\n'))
            .unwrap_or(&stdout)
            .to_owned())
    }

    fn tracked(&self, root: &Path, path: &Path) -> Result<bool, String> {
        Ok(!self
            .text(
                root,
                ["--literal-pathspecs", "ls-files", "--"]
                    .map(OsStr::new)
                    .into_iter()
                    .chain(std::iter::once(path.as_os_str())),
            )?
            .is_empty())
    }

    fn clean(&self, root: &Path) -> Result<(), String> {
        if !self
            .text(
                root,
                [
                    "--no-optional-locks",
                    "status",
                    "--porcelain",
                    "--untracked-files=all",
                    "--ignore-submodules=none",
                ],
            )?
            .is_empty()
        {
            return Err(
                "the Git work tree has uncommitted changes; commit or stash them before releasing"
                    .into(),
            );
        }
        Ok(())
    }
}

fn next_version(current: &Version, requested: &str) -> Result<Version, String> {
    let bump = |part: u64| {
        part.checked_add(1)
            .ok_or("the version component cannot be incremented")
    };
    let next = match requested {
        "major" => Version::new(bump(current.major)?, 0, 0),
        "minor" => Version::new(current.major, bump(current.minor)?, 0),
        "patch" => Version::new(current.major, current.minor, bump(current.patch)?),
        text => Version::parse(text).map_err(|e| format!("release version {text:?}: {e}"))?,
    };
    if !next.cmp_precedence(current).is_gt() {
        return Err(format!(
            "release version {next} must be higher than {current}"
        ));
    }
    Ok(next)
}

pub(crate) fn run(entry: ChildEntry, root: &Path, args: &[String]) -> Result<(), String> {
    release(root, args, &Git::default(), &fetch, |root, tag| {
        crate::sdk_cmds::check_mod(entry, root, false, &[], Some(tag))
    })
}

fn release(
    package: &Path,
    args: &[String],
    git: &Git,
    fetch: Fetch<'_>,
    check: impl FnOnce(&Path, &str) -> Result<(), String>,
) -> Result<(), String> {
    if let Some((command, rest)) = args.split_first()
        && command == "setup"
    {
        Options::new("release setup", rest, &[]).finish()?;
        let project = Project::read(package, git)?;
        for change in project.workflow_changes(&workflow(fetch)?)? {
            change.apply(&project.root)?;
            change.describe();
        }
        return Ok(());
    }
    let mut options = Options::new("release", args, &[]);
    let preview = options.switch("--check");
    let requested = options.at_most_one()?.unwrap_or("patch");
    options.finish()?;
    let project = Project::read(package, git)?;
    let root = &project.root;
    git.clean(root)?;
    let branch = git
        .text(root, ["symbolic-ref", "--quiet", "HEAD"])
        .map_err(|_| "check out a branch before releasing")?;
    let head = git.text(root, ["rev-parse", "HEAD"])?;
    let origin = git.text(root, ["remote", "get-url", "origin"])?;
    registered(fetch, &origin, &project.path)?;
    let version = next_version(&project.manifest.version, requested)?;
    let tag = format!("{}v{version}", project.prefix);
    let tag_ref = format!("refs/tags/{tag}");
    if !git.text(root, ["tag", "--list", &tag])?.is_empty() {
        return Err(format!("tag {tag} already exists locally"));
    }
    let remote = git.text(
        root,
        ["ls-remote", "--refs", "--", "origin", &branch, &tag_ref],
    )?;
    let find_ref = |name: &str| {
        remote
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .find_map(|(oid, r)| (r == name).then_some(oid))
    };
    if find_ref(&tag_ref).is_some() {
        return Err(format!("tag {tag} already exists on origin"));
    }
    let upstream = git.text(
        root,
        [
            "for-each-ref",
            "--format=%(upstream:remotename)%00%(upstream:remoteref)",
            &branch,
        ],
    )?;
    let upstream_refs = match upstream.split_once('\0') {
        Some((name, reference))
            if !name.is_empty()
                && !reference.is_empty()
                && (name != "origin" || reference != branch) =>
        {
            git.text(root, ["ls-remote", "--refs", "--", name, reference])?
        }
        _ => String::new(),
    };
    let upstream_head = upstream_refs.split_once('\t').map(|(oid, _)| oid);
    for oid in [find_ref(&branch), upstream_head].into_iter().flatten() {
        let status = git
            .command(root)
            .args(["merge-base", "--is-ancestor", oid, &head])
            .output()
            .map_err(|e| format!("git merge-base: {e}"))?
            .status;
        if !status.success() {
            return Err("the branch is not up to date with its upstream or origin; fetch and integrate the remote branch before releasing".into());
        }
    }

    let mut changes = project.workflow_changes(&workflow(fetch)?)?;
    changes.extend(project.version_changes(git, &version)?);

    // The same check as CI sees the proposed version and tag, not the previous version.
    // Clone to a separate Git repository: the check can fetch an SDK and build native code,
    // without writing version files, build output or worktree metadata in the author's repo.
    let scratch = tempfile::tempdir().map_err(|e| format!("release check: {e}"))?;
    let snapshot = scratch.path().join("checkout");
    git.text(
        root,
        [
            OsStr::new("clone"),
            OsStr::new("--shared"),
            OsStr::new("--no-checkout"),
            OsStr::new("--"),
            root.as_os_str(),
            snapshot.as_os_str(),
        ],
    )?;
    git.text(&snapshot, ["checkout", "--detach", &head])?;
    for change in &changes {
        change.apply(&snapshot)?;
    }
    check(&snapshot.join(&project.package), &tag)?;
    if preview {
        println!("Would release {version}:");
        for change in changes {
            change.describe();
        }
        println!(
            "  Commit as Release {version}, tag {tag}, and push {branch} and {tag_ref} to origin"
        );
        return Ok(());
    }

    git.clean(root)?;
    if git.text(root, ["rev-parse", "HEAD"])? != head
        || git.text(root, ["symbolic-ref", "--quiet", "HEAD"])? != branch
    {
        return Err(
            "the checked-out branch changed during the release check; run release again".into(),
        );
    }
    // Stage only explicit changes. Ignore paths whose only change was removing an ignored legacy file.
    let mut tracked = Vec::new();
    for change in &changes {
        let path = change.path.as_os_str();
        if matches!(change.kind, Kind::Write(_)) || git.tracked(root, &change.path)? {
            tracked.push(path);
        }
        change.apply(root)?;
    }
    git.text(
        root,
        ["--literal-pathspecs", "add", "-f", "-A", "--"]
            .map(OsStr::new)
            .into_iter()
            .chain(tracked),
    )?;
    git.text(root, ["commit", "-m", &format!("Release {version}")])?;
    git.text(root, ["tag", &tag])?;
    // Publish both refs together, or neither. In particular, a rejected branch must not release a tag.
    git.text(root, ["push", "--atomic", "--no-follow-tags", "origin", &branch, &tag_ref])
        .map_err(|e| format!("{e}\nThe release commit and {tag} remain local. Resolve the push error, then push both refs to origin."))?;
    println!(
        "Released {version}: pushed {tag} to origin. GitHub Actions will build and attest the release."
    );
    Ok(())
}

#[cfg(test)]
mod tests;
