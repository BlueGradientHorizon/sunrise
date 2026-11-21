//! Resolve published stable GitHub releases to the exact commits their tags name.
use crate::internals::atomic_write;
use serde::Deserialize;
use std::{collections::BTreeSet, env, io::Read, path::Path, time::Duration};
use sunrise_addresses::{GAME_GIT_API, SDK_RELEASE_LIST};
use sunrise_mod_package::{
    index::{
        registry::{ReleaseInput, parse_releases},
        stable_tag,
    },
    manifest::revision,
};

const PAGE_SIZE: usize = 100; // SDK_RELEASE_LIST asks GitHub for this many rows.
const MAX_PAGES: usize = 10_000;
const MAX_TAG_DEPTH: usize = 128;
const MAX_RESPONSE: u64 = 32 << 20;

struct Response {
    body: Vec<u8>,
    links: Vec<String>,
}

pub(super) fn fetch(out: &Path) -> Result<(), String> {
    let mut auth = None;
    for name in ["GH_TOKEN", "GITHUB_TOKEN"] {
        match env::var(name) {
            Ok(token) if token.is_empty() => continue,
            Ok(token) if token.bytes().all(|b| b.is_ascii_graphic()) => {
                auth = Some(format!("Bearer {token}"));
                break;
            }
            Err(env::VarError::NotPresent) => continue,
            _ => return Err(format!("{name} is not a valid GitHub token")),
        }
    }
    // Request only the fixed Sunrise repository. Do not forward credentials to redirects or
    // to URLs supplied by pagination headers or Git objects.
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(30))
        .build();
    write_list(out, |url| {
        let mut request = agent
            .get(url)
            .set("Accept", "application/vnd.github+json")
            .set("User-Agent", "cargo-sunrise-mod");
        if let Some(auth) = &auth {
            request = request.set("Authorization", auth);
        }
        let response = request.call().map_err(|error| match error {
            ureq::Error::Status(status, _) => format!("GitHub returned HTTP {status}"),
            ureq::Error::Transport(error) => format!("GitHub request failed: {:?}", error.kind()),
        })?;
        if response.status() != 200 {
            return Err(format!(
                "GitHub returned HTTP {}, expected 200",
                response.status()
            ));
        }
        let links = response
            .all("Link")
            .into_iter()
            .map(str::to_owned)
            .collect();
        let mut body = Vec::new();
        response
            .into_reader()
            .take(MAX_RESPONSE + 1)
            .read_to_end(&mut body)
            .map_err(|e| format!("could not read the whole GitHub response: {e}"))?;
        if body.len() as u64 > MAX_RESPONSE {
            return Err("GitHub response exceeds the release discovery size limit".into());
        }
        Ok(Response { body, links })
    })
}

fn write_list(
    out: &Path,
    fetch: impl FnMut(&str) -> Result<Response, String>,
) -> Result<(), String> {
    let releases = discover(fetch)?;
    let mut text = serde_json::to_string(&releases).map_err(|e| e.to_string())?;
    parse_releases(&text)?;
    text.push('\n');
    atomic_write(out, text)?;
    println!("Wrote {}", out.display());
    Ok(())
}

#[derive(Deserialize)]
struct Release {
    id: u64,
    tag_name: String,
    draft: bool,
    prerelease: bool,
    // Null means unpublished; unlike Option, this requires the field to be present.
    published_at: PublishedAt,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PublishedAt {
    At(String),
    Unpublished(()),
}

impl PublishedAt {
    fn is_published(&self) -> bool {
        let Self::At(timestamp) = self else {
            return false;
        };
        let pattern = b"0000-00-00T00:00:00Z";
        timestamp.len() == pattern.len()
            && timestamp.bytes().zip(pattern).all(|(byte, expected)| {
                if *expected == b'0' {
                    byte.is_ascii_digit()
                } else {
                    byte == *expected
                }
            })
    }
}

#[derive(Default)]
struct PageLinks {
    next: Option<usize>,
    last: Option<usize>,
}

impl PageLinks {
    fn read(headers: &[String]) -> Result<Self, String> {
        let mut pages = Self::default();
        let invalid = || "malformed GitHub releases pagination header".to_owned();
        for link in headers.iter().flat_map(|header| header.split(',')) {
            let (url, attrs) = link
                .trim()
                .strip_prefix('<')
                .and_then(|link| link.split_once('>'))
                .ok_or_else(invalid)?;
            let relation = attrs
                .split(';')
                .find_map(|attr| {
                    attr.trim()
                        .strip_prefix("rel=\"")
                        .and_then(|rel| rel.strip_suffix('"'))
                })
                .ok_or_else(invalid)?;
            let slot = match relation {
                "next" => &mut pages.next,
                "last" => &mut pages.last,
                _ => continue,
            };
            let page = url
                .split_once('?')
                .and_then(|(_, query)| {
                    query
                        .split('&')
                        .find_map(|param| param.strip_prefix("page="))
                })
                .and_then(|page| page.parse::<usize>().ok())
                .filter(|page| *page != 0)
                .ok_or_else(invalid)?;
            if slot.replace(page).is_some() {
                return Err(invalid());
            }
        }
        Ok(pages)
    }
}

fn discover(
    mut fetch: impl FnMut(&str) -> Result<Response, String>,
) -> Result<Vec<ReleaseInput>, String> {
    let mut tags = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut page = 1;
    let mut last = None;
    let mut promised = false;
    loop {
        let at = |e| format!("Sunrise releases page {page}: {e}");
        let response = fetch(&format!("{SDK_RELEASE_LIST}&page={page}")).map_err(at)?;
        let links = PageLinks::read(&response.links).map_err(at)?;
        let rows: Vec<Release> =
            serde_json::from_slice(&response.body).map_err(|e| at(e.to_string()))?;
        if let Some(end) = links.last {
            if end < page || last.is_some_and(|old| old != end) {
                return Err(at("the last page changed during listing".into()));
            }
            last = Some(end);
        }
        if rows.len() > PAGE_SIZE || (rows.is_empty() && (promised || links.next.is_some())) {
            return Err(at("incomplete or inconsistent page".into()));
        }
        let full = rows.len() == PAGE_SIZE;
        for release in rows {
            if !ids.insert(release.id) {
                return Err(at(format!(
                    "release {} appeared twice during listing",
                    release.id
                )));
            }
            if !release.draft
                && !release.prerelease
                && release.published_at.is_published()
                && stable_tag(&release.tag_name).is_some()
            {
                tags.insert(release.tag_name);
            }
        }
        if let Some(next) = links.next {
            if next != page + 1 || last.is_some_and(|end| next > end) {
                return Err(at("pagination did not point to the next page".into()));
            }
            promised = true;
        } else {
            if last.is_some_and(|end| page < end) {
                return Err(at(
                    "missing next page before the advertised last page".into()
                ));
            }
            // In the absence of pagination headers a full page may not be the last.
            if !full || last == Some(page) {
                break;
            }
            promised = false;
        }
        page += 1;
        if page > MAX_PAGES {
            return Err("Sunrise release listing exceeded its page limit".into());
        }
    }
    tags.into_iter()
        .map(|tag| {
            let commit =
                peel(&tag, &mut fetch).map_err(|e| format!("Sunrise release {tag}: {e}"))?;
            Ok(ReleaseInput { tag, commit })
        })
        .collect()
}

#[derive(Deserialize)]
struct GitObject {
    #[serde(rename = "type")]
    kind: String,
    sha: String,
}

fn peel(
    tag: &str,
    fetch: &mut impl FnMut(&str) -> Result<Response, String>,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Ref {
        object: GitObject,
    }
    let mut url = format!("{GAME_GIT_API}/ref/tags/{tag}");
    let mut seen = BTreeSet::new();
    for _ in 0..MAX_TAG_DEPTH {
        let response = fetch(&url)?;
        let Ref { object } = serde_json::from_slice(&response.body).map_err(|e| e.to_string())?;
        if !revision(&object.sha) {
            return Err("GitHub did not return a full lowercase Git object ID".into());
        }
        match object.kind.as_str() {
            "commit" => return Ok(object.sha),
            "tag" => {
                url = format!("{GAME_GIT_API}/tags/{}", object.sha);
                if !seen.insert(object.sha) {
                    return Err("annotated tag cycle".into());
                }
            }
            _ => return Err("tag does not point to a commit".into()),
        }
    }
    Err("annotated tag chain is too deep".into())
}

#[cfg(test)]
mod tests;
