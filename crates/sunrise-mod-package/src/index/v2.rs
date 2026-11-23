//! The signed v2 registry index (`v2/index.json`, sunrise-mods `docs/registry-v2.md` "Published
//! index"): only what the mod tool reads to take a library. Under schema 2 a reader ignores
//! fields it doesn't know, so the published page, build and fit fields are left out here.
use super::Kind;
use serde::Deserialize;

/// The only schema served at the v2 address; a breaking change raises it and moves the index.
pub const SCHEMA: u64 = 2;

#[derive(Debug, Deserialize)]
pub struct Index {
    pub schema: u64,
    /// Rises with every publish: a reader refuses one lower than it last accepted.
    pub serial: u64,
    /// Projects with at least one release.
    pub projects: Vec<Project>,
    pub revoked: Vec<Revoked>,
}

#[derive(Debug, Deserialize)]
pub struct Project {
    /// The permanent project ID, not the in-game ID.
    pub id: String,
    /// The in-game ID: the dependency slot the project provides.
    #[serde(rename = "mod")]
    pub mod_id: String,
    pub kind: Kind,
    pub repository: Repository,
    pub releases: Vec<Release>,
}

#[derive(Debug, Deserialize)]
pub struct Repository {
    /// GitHub's numeric repository ID, which a release's attestation must name.
    pub id: u64,
    /// `owner/name` when published; a rename changes it, never the ID.
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Release {
    pub version: String,
    /// The exact signer workflow ref the release's attestation records.
    pub workflow: String,
    pub file: File,
}

#[derive(Debug, Deserialize)]
pub struct File {
    pub url: String,
    pub size: u64,
    pub sha256: String,
    /// The `.sigstore.json` bundle attesting `sha256`.
    pub attestation: String,
}

#[derive(Debug, Deserialize)]
pub struct Revoked {
    pub project: String,
    /// One file's digest, or none for every file of the project.
    pub sha256: Option<String>,
}

impl Index {
    /// Check `signature` (`.minisig` text) over the exact bytes served, then read them.
    pub fn verify(bytes: &[u8], signature: &str, key: &str) -> Result<Self, String> {
        super::verify_signature_bytes(key, signature, bytes)
            .map_err(|_| "the signature doesn't match the registry's key".to_string())?;
        let index: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if index.schema != SCHEMA {
            return Err(format!(
                "schema {} where {SCHEMA} is expected",
                index.schema
            ));
        }
        Ok(index)
    }

    /// The takedown covering `project`'s file with this digest, or every file of it.
    pub fn revoked(&self, project: &str, sha256: &str) -> Option<&Revoked> {
        self.revoked.iter().find(|r| {
            r.project == project && r.sha256.as_deref().is_none_or(|digest| digest == sha256)
        })
    }
}
