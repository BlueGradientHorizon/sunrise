//! Resolve and download exact original Java jars from Modrinth.
//!
//! Callers supply the Modrinth endpoints ([`Modrinth`]) and their own HTTP transport ([`Http`]),
//! keeping this crate independent of any HTTP stack or Sunrise workspace crate so both
//! `cargo-sunrise-mod` and `sunrise-launcher` can carry it unchanged.

use serde::Deserialize;
use sha2::{Digest, Sha512};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

/// The largest jar downloaded or written (512 MiB).
pub const MAX_JAR: u64 = 512 << 20;

/// Maximum size of a Modrinth API JSON response (16 MiB).
const MAX_API_RESPONSE: u64 = 16 << 20;

/// Modrinth project types that may appear in a version page URL.
const MODRINTH_PROJECT_TYPES: [&str; 7] = [
    "mod",
    "plugin",
    "datapack",
    "shader",
    "resourcepack",
    "modpack",
    "project",
];

/// Modrinth endpoints supplied by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modrinth<'a> {
    /// Base URL of Modrinth's v2 API (for example `https://api.modrinth.com/v2`).
    pub api: &'a str,
    /// Exact CDN host name from which jars may be downloaded (for example `cdn.modrinth.com`).
    pub cdn_host: &'a str,
}

/// Minimal HTTP transport supplied by the caller.
pub trait Http {
    /// Fetches `url` up to `limit` bytes. Returns `Ok(None)` on HTTP 404.
    fn get(&self, url: &str, limit: u64) -> Result<Option<Vec<u8>>, String>;

    /// Sends a JSON `POST` request to `url` with `body` and reads up to `limit` bytes of UTF-8 text.
    fn post_json(&self, url: &str, body: &str, limit: u64) -> Result<String, String>;
}

/// One file of a Modrinth version, as returned by [`version_file`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionFile {
    /// Verified CDN download URL.
    pub url: String,
    /// Lowercase hexadecimal SHA-512 digest.
    pub sha512: String,
    /// Original file name reported by Modrinth.
    pub filename: String,
}

/// How a caller names a Modrinth version for [`version_file`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionRef<'a> {
    /// A Modrinth version page URL:
    /// `https://modrinth.com/<type>/<slug-or-id>/version/<version-number-or-id>`.
    Url(&'a str),
    /// A Modrinth project slug or ID paired with a version number or ID.
    ProjectVersion { project: &'a str, version: &'a str },
}

/// Returns whether `url` is an `https://` URL on exactly `cdn_host` with a non-empty path.
fn is_cdn_url(url: &str, cdn_host: &str) -> bool {
    if cdn_host.is_empty() || url.bytes().any(|b| b <= b' ' || b == 0x7f) {
        return false;
    }
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let Some((authority, path)) = rest.split_once('/') else {
        return false;
    };
    !path.is_empty() && authority.eq_ignore_ascii_case(cdn_host)
}

/// Returns whether `segment` is a valid Modrinth project or version path segment.
fn valid_segment(segment: &str) -> bool {
    !matches!(segment, "" | "." | "..")
        && !segment
            .bytes()
            .any(|b| b <= b' ' || b == 0x7f || matches!(b, b'/' | b'\\' | b'?' | b'#'))
}

/// Percent-encodes one URL path segment.
fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Parses a Modrinth version reference into `(project, version)`.
fn parse_reference<'a>(reference: VersionRef<'a>) -> Result<(&'a str, &'a str), String> {
    match reference {
        VersionRef::ProjectVersion { project, version } => {
            if !valid_segment(project) || !valid_segment(version) {
                return Err(format!(
                    "expected a Modrinth project and version, got {project:?} {version:?}"
                ));
            }
            Ok((project, version))
        }
        VersionRef::Url(url) => {
            let expected = || {
                format!(
                    "expected a Modrinth version URL like https://modrinth.com/mod/<project>/version/<version>, got {url:?}"
                )
            };
            if url.bytes().any(|b| b <= b' ' || b == 0x7f) || url.contains('?') || url.contains('#')
            {
                return Err(expected());
            }
            let rest = url
                .strip_prefix("https://modrinth.com/")
                .ok_or_else(expected)?;
            let rest = rest.strip_suffix('/').unwrap_or(rest);
            let mut parts = rest.split('/');
            let (Some(kind), Some(project), Some("version"), Some(version), None) = (
                parts.next(),
                parts.next(),
                parts.next(),
                parts.next(),
                parts.next(),
            ) else {
                return Err(expected());
            };
            if !MODRINTH_PROJECT_TYPES.contains(&kind)
                || !valid_segment(project)
                || !valid_segment(version)
            {
                return Err(expected());
            }
            Ok((project, version))
        }
    }
}

/// Looks up the CDN download URL for each SHA-512 hash in one `POST {api}/version_files` request.
///
/// For each hash, keeps only a file whose own `hashes.sha512` equals that hash (case-insensitive)
/// and whose URL is `https` on exactly `modrinth.cdn_host`.
pub fn file_urls(
    http: &dyn Http,
    modrinth: Modrinth<'_>,
    sha512s: &[&str],
) -> Result<BTreeMap<String, String>, String> {
    if sha512s.is_empty() {
        return Ok(BTreeMap::new());
    }
    #[derive(Deserialize)]
    struct Version {
        files: Vec<File>,
    }
    #[derive(Deserialize)]
    struct File {
        hashes: BTreeMap<String, String>,
        url: String,
    }
    let url = format!("{}/version_files", modrinth.api.trim_end_matches('/'));
    let body = serde_json::json!({"hashes": sha512s, "algorithm": "sha512"}).to_string();
    let text = http.post_json(&url, &body, MAX_API_RESPONSE)?;
    let versions: BTreeMap<String, Version> =
        serde_json::from_str(&text).map_err(|e| format!("Modrinth version files: {e}"))?;
    Ok(versions
        .into_iter()
        .filter_map(|(hash, version)| {
            version
                .files
                .into_iter()
                .find(|file| {
                    file.hashes
                        .get("sha512")
                        .is_some_and(|actual| actual.eq_ignore_ascii_case(&hash))
                        && is_cdn_url(&file.url, modrinth.cdn_host)
                })
                .map(|file| (hash, file.url))
        })
        .collect())
}

/// Downloads `url` from `modrinth.cdn_host` into `dest`, publishing it only once its SHA-512
/// matches `sha512`.
///
/// Refuses any URL that is not `https` on `modrinth.cdn_host`, reads at most [`MAX_JAR`] bytes,
/// writes to a temporary file beside `dest`, verifies SHA-512, and renames it into place. On any
/// error or hash mismatch, the temporary file is removed and `dest` is never left partial.
pub fn download(
    http: &dyn Http,
    modrinth: Modrinth<'_>,
    url: &str,
    sha512: &str,
    dest: &Path,
) -> Result<(), String> {
    if !is_cdn_url(url, modrinth.cdn_host) {
        return Err(format!(
            "{url}: refused non-CDN URL (expected https://{}/...)",
            modrinth.cdn_host
        ));
    }
    let bytes = http
        .get(url, MAX_JAR)?
        .ok_or_else(|| format!("{url}: Modrinth jar not found"))?;
    if bytes.len() as u64 > MAX_JAR {
        return Err(format!("{url}: jar exceeds 512 MiB"));
    }
    if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let suffix = NEXT.fetch_add(1, Ordering::Relaxed);
    let part = dest.with_file_name(format!(
        ".sunrise-download-{}-{suffix}.part",
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part)
        .map_err(|e| format!("{url}: {e}"))?;
    let result = (|| {
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        let got = format!("{:x}", Sha512::digest(&bytes));
        if !sha512.eq_ignore_ascii_case(&got) {
            return Err(format!("SHA-512 {got}, expected {sha512}"));
        }
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&part, dest).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result.map_err(|e| format!("{url}: {e}"))
}

/// Resolves `sha512` on Modrinth via [`file_urls`] and downloads it to `dest` via [`download`].
///
/// When Modrinth does not list the hash, returns an error stating that the original file is not
/// (or no longer) on Modrinth.
pub fn fetch(
    http: &dyn Http,
    modrinth: Modrinth<'_>,
    sha512: &str,
    dest: &Path,
) -> Result<(), String> {
    let urls = file_urls(http, modrinth, &[sha512])?;
    let url = urls
        .iter()
        .find(|(hash, _)| hash.eq_ignore_ascii_case(sha512))
        .map(|(_, url)| url.as_str())
        .ok_or_else(|| {
            format!("original file (SHA-512 {sha512}) is not (or no longer) on Modrinth")
        })?;
    download(http, modrinth, url, sha512, dest)
}

/// Resolves a Modrinth version page URL or `(project, version)` pair to its downloadable file.
///
/// Calls `GET {api}/project/{project}/version/{version}`, selects the file marked `primary` (or
/// the only file when there is one), and refuses a version with multiple non-primary files by
/// listing their names.
pub fn version_file(
    http: &dyn Http,
    modrinth: Modrinth<'_>,
    reference: VersionRef<'_>,
) -> Result<VersionFile, String> {
    #[derive(Deserialize)]
    struct VersionResponse {
        files: Vec<VersionResponseFile>,
    }
    #[derive(Deserialize)]
    struct VersionResponseFile {
        url: String,
        filename: String,
        #[serde(default)]
        primary: bool,
        hashes: BTreeMap<String, String>,
    }
    let (project, version) = parse_reference(reference)?;
    let url = format!(
        "{}/project/{}/version/{}",
        modrinth.api.trim_end_matches('/'),
        encode_segment(project),
        encode_segment(version)
    );
    let bytes = http
        .get(&url, MAX_API_RESPONSE)?
        .ok_or_else(|| format!("Modrinth version {project} {version} was not found"))?;
    let parsed: VersionResponse =
        serde_json::from_slice(&bytes).map_err(|e| format!("{url}: {e}"))?;
    let chosen = if let Some(primary) = parsed.files.iter().find(|file| file.primary) {
        primary
    } else if parsed.files.len() == 1 {
        &parsed.files[0]
    } else if parsed.files.is_empty() {
        return Err(format!("Modrinth version {project} {version} has no files"));
    } else {
        let names: Vec<&str> = parsed
            .files
            .iter()
            .map(|file| file.filename.as_str())
            .collect();
        return Err(format!(
            "Modrinth version {project} {version} has multiple files and none is marked primary: {}",
            names.join(", ")
        ));
    };
    let sha512 = chosen
        .hashes
        .get("sha512")
        .filter(|h| h.len() == 128 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| {
            format!(
                "Modrinth version {project} {version} file {} is missing a SHA-512 hash",
                chosen.filename
            )
        })?
        .to_ascii_lowercase();
    if !is_cdn_url(&chosen.url, modrinth.cdn_host) {
        return Err(format!(
            "{}: refused non-CDN URL (expected https://{}/...)",
            chosen.url, modrinth.cdn_host
        ));
    }
    Ok(VersionFile {
        url: chosen.url.clone(),
        sha512,
        filename: chosen.filename.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const MODRINTH: Modrinth<'static> = Modrinth {
        api: "https://api.modrinth.com/v2",
        cdn_host: "cdn.modrinth.com",
    };

    struct FakeHttp {
        gets: BTreeMap<String, Option<Vec<u8>>>,
        posts: BTreeMap<String, String>,
        seen_posts: RefCell<Vec<(String, String)>>,
    }

    impl FakeHttp {
        fn new() -> Self {
            Self {
                gets: BTreeMap::new(),
                posts: BTreeMap::new(),
                seen_posts: RefCell::new(Vec::new()),
            }
        }
    }

    impl Http for FakeHttp {
        fn get(&self, url: &str, limit: u64) -> Result<Option<Vec<u8>>, String> {
            let Some(entry) = self.gets.get(url) else {
                return Ok(None);
            };
            if let Some(bytes) = entry
                && bytes.len() as u64 > limit
            {
                return Err(format!("{url}: exceeds limit {limit}"));
            }
            Ok(entry.clone())
        }

        fn post_json(&self, url: &str, body: &str, _limit: u64) -> Result<String, String> {
            self.seen_posts
                .borrow_mut()
                .push((url.to_owned(), body.to_owned()));
            self.posts
                .get(url)
                .cloned()
                .ok_or_else(|| format!("unexpected POST {url}"))
        }
    }

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "sunrise-jar-fetch-{name}-{}-{n}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn file_urls_keeps_only_exact_hash_cdn_files() {
        let hash_a = "a".repeat(128);
        let hash_b = "b".repeat(128);
        let hash_c = "c".repeat(128);
        let mut http = FakeHttp::new();
        http.posts.insert(
            "https://api.modrinth.com/v2/version_files".into(),
            serde_json::json!({
                hash_a.clone(): {"files": [
                    {"primary": true, "url": "https://cdn.modrinth.com/data/wrong.jar", "hashes": {"sha512": hash_b}},
                    {"url": "https://evil.example/data/exact.jar", "hashes": {"sha512": hash_a}},
                    {"url": "https://cdn.modrinth.com/data/exact.jar", "hashes": {"sha512": hash_a.to_uppercase()}}
                ]},
                hash_b.clone(): {"files": [
                    {"url": "http://cdn.modrinth.com/data/insecure.jar", "hashes": {"sha512": hash_b}}
                ]},
                hash_c.clone(): {"files": [
                    {"url": "https://cdn.modrinth.com@evil.example/data/tricky.jar", "hashes": {"sha512": hash_c}}
                ]}
            })
            .to_string(),
        );

        let urls = file_urls(&http, MODRINTH, &[&hash_a, &hash_b, &hash_c]).unwrap();
        assert_eq!(
            urls,
            BTreeMap::from([(hash_a, "https://cdn.modrinth.com/data/exact.jar".into())])
        );
    }

    #[test]
    fn download_refuses_other_hosts() {
        let dir = TempDir::new("bad-host");
        let dest = dir.0.join("mod.jar");
        let http = FakeHttp::new();
        for bad in [
            "http://cdn.modrinth.com/data/mod.jar",
            "https://evil.example/data/mod.jar",
            "https://cdn.modrinth.com.evil.example/data/mod.jar",
            "https://cdn.modrinth.com@evil.example/data/mod.jar",
            "https://cdn.modrinth.com/",
        ] {
            let err = download(&http, MODRINTH, bad, &"0".repeat(128), &dest).unwrap_err();
            assert!(err.contains("non-CDN"), "{bad}: {err}");
            assert!(!dest.exists());
        }
    }

    #[test]
    fn download_refuses_wrong_hash_and_leaves_no_file() {
        let dir = TempDir::new("bad-hash");
        let dest = dir.0.join("mod.jar");
        let url = "https://cdn.modrinth.com/data/mod.jar";
        let expected = "0".repeat(128);
        let actual = format!("{:x}", Sha512::digest(b"actual jar bytes"));
        let mut http = FakeHttp::new();
        http.gets
            .insert(url.into(), Some(b"actual jar bytes".to_vec()));

        let err = download(&http, MODRINTH, url, &expected, &dest).unwrap_err();
        assert!(err.contains(&actual), "{err}");
        assert!(err.contains(&expected), "{err}");
        assert!(!dest.exists());
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
    }

    #[test]
    fn download_publishes_verified_bytes() {
        let dir = TempDir::new("ok");
        let dest = dir.0.join("sub/mod.jar");
        let url = "https://cdn.modrinth.com/data/mod.jar";
        let payload = b"verified jar bytes";
        let sha512 = format!("{:x}", Sha512::digest(payload));
        let mut http = FakeHttp::new();
        http.gets.insert(url.into(), Some(payload.to_vec()));

        download(&http, MODRINTH, url, &sha512.to_uppercase(), &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), payload);
        assert_eq!(fs::read_dir(dest.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn fetch_reports_when_modrinth_does_not_list_file() {
        let dir = TempDir::new("missing");
        let dest = dir.0.join("mod.jar");
        let mut http = FakeHttp::new();
        http.posts.insert(
            "https://api.modrinth.com/v2/version_files".into(),
            "{}".into(),
        );
        let err = fetch(&http, MODRINTH, &"f".repeat(128), &dest).unwrap_err();
        assert!(err.contains("Modrinth"), "{err}");
        assert!(!dest.exists());
    }

    #[test]
    fn version_file_reads_page_url_and_picks_primary() {
        let primary_hash = "1".repeat(128);
        let other_hash = "2".repeat(128);
        let mut http = FakeHttp::new();
        http.gets.insert(
            "https://api.modrinth.com/v2/project/macaws-bridges/version/3.0.0%2B1.21".into(),
            Some(
                serde_json::json!({
                    "files": [
                        {
                            "primary": false,
                            "filename": "mcw-bridges-sources.jar",
                            "url": "https://cdn.modrinth.com/data/sources.jar",
                            "hashes": {"sha512": other_hash}
                        },
                        {
                            "primary": true,
                            "filename": "mcw-bridges.jar",
                            "url": "https://cdn.modrinth.com/data/mcw-bridges.jar",
                            "hashes": {"sha512": primary_hash.to_uppercase()}
                        }
                    ]
                })
                .to_string()
                .into_bytes(),
            ),
        );

        let file = version_file(
            &http,
            MODRINTH,
            VersionRef::Url("https://modrinth.com/mod/macaws-bridges/version/3.0.0+1.21"),
        )
        .unwrap();
        assert_eq!(
            file,
            VersionFile {
                url: "https://cdn.modrinth.com/data/mcw-bridges.jar".into(),
                sha512: primary_hash,
                filename: "mcw-bridges.jar".into(),
            }
        );
    }

    #[test]
    fn version_file_reads_project_version_and_refuses_ambiguous_or_bad_urls() {
        let only_hash = "3".repeat(128);
        let mut http = FakeHttp::new();
        http.gets.insert(
            "https://api.modrinth.com/v2/project/sodium/version/mc1.21-0.6.0".into(),
            Some(
                serde_json::json!({
                    "files": [
                        {
                            "primary": false,
                            "filename": "sodium.jar",
                            "url": "https://cdn.modrinth.com/data/sodium.jar",
                            "hashes": {"sha512": only_hash}
                        }
                    ]
                })
                .to_string()
                .into_bytes(),
            ),
        );
        let file = version_file(
            &http,
            MODRINTH,
            VersionRef::ProjectVersion {
                project: "sodium",
                version: "mc1.21-0.6.0",
            },
        )
        .unwrap();
        assert_eq!(file.filename, "sodium.jar");

        http.gets.insert(
            "https://api.modrinth.com/v2/project/multi/version/1.0".into(),
            Some(
                serde_json::json!({
                    "files": [
                        {
                            "primary": false,
                            "filename": "alpha.jar",
                            "url": "https://cdn.modrinth.com/data/alpha.jar",
                            "hashes": {"sha512": "4".repeat(128)}
                        },
                        {
                            "primary": false,
                            "filename": "beta.jar",
                            "url": "https://cdn.modrinth.com/data/beta.jar",
                            "hashes": {"sha512": "5".repeat(128)}
                        }
                    ]
                })
                .to_string()
                .into_bytes(),
            ),
        );
        let err = version_file(
            &http,
            MODRINTH,
            VersionRef::ProjectVersion {
                project: "multi",
                version: "1.0",
            },
        )
        .unwrap_err();
        assert!(
            err.contains("alpha.jar") && err.contains("beta.jar"),
            "{err}"
        );

        for bad in [
            "https://modrinth.com/mod/sodium",
            "https://modrinth.com/unknown/sodium/version/1.0",
            "https://example.com/mod/sodium/version/1.0",
            "https://modrinth.com/mod/sodium/version/1.0?foo=bar",
        ] {
            let err = version_file(&http, MODRINTH, VersionRef::Url(bad)).unwrap_err();
            assert!(
                err.contains("https://modrinth.com/mod/<project>/version/<version>"),
                "{bad}: {err}"
            );
        }
    }
}
