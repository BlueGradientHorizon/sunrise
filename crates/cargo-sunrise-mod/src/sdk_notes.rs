//! The change notes of the SDK builds between two builds. Each build's `sdk.json` carries the notes of the changes since the build
//! before it (`surface.previous`), so a mod that moves across several builds follows that chain.
//! Only each build's small files are fetched ([`crate::sdk_fetch::fetch_build`]).
use crate::sdk_fetch::PinnedSdk;
use sunrise_mod_package::sdk::{self, ChangeNote, Index};

/// A change note and the build whose `sdk.json` carried it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Shipped {
    /// The build's name, as people read it.
    pub build: String,
    pub note: ChangeNote,
}

/// The notes found walking back through the builds, newest build first.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Chain {
    pub notes: Vec<Shipped>,
    /// Why the walk ended before the older build or the chain's end: the build it could not
    /// read, and why.
    pub stopped: Option<String>,
}

impl Chain {
    /// The line that says why the walk ended early, when it did.
    pub(crate) fn stopped_line(&self) -> Option<String> {
        self.stopped
            .as_ref()
            .map(|why| format!("Older change notes were not read: {why}"))
    }
}

/// `sdk`'s verified `sdk.json`.
fn index_of(sdk: &PinnedSdk) -> Result<Index, String> {
    let path = sdk.dir.join(sdk::INDEX);
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Index::read(&bytes, &sdk.build).map_err(|e| format!("{}: {e}", path.display()))
}

/// The notes of `newer` and of every build before it, following `surface.previous`: up to the
/// build `older` names (the walk stops when the chain reaches it, and its own notes are not
/// included), or with no `older` to the chain's end. A build `fetch` cannot read ends the walk
/// with the notes found so far and the reason.
pub(crate) fn chain(
    newer: &PinnedSdk,
    older: Option<&str>,
    mut fetch: impl FnMut(&str) -> Result<PinnedSdk, String>,
) -> Result<Chain, String> {
    let mut notes = Vec::new();
    let mut stopped = None;
    let mut name = newer.name.clone();
    let mut index = index_of(newer)?;
    loop {
        notes.extend(index.surface.notes.drain(..).map(|note| Shipped {
            build: name.clone(),
            note,
        }));
        let Some(previous) = index.surface.previous.take() else {
            break;
        };
        if older == Some(previous.as_str()) {
            break;
        }
        match fetch(&previous).and_then(|build| Ok((index_of(&build)?, build.name))) {
            Ok((next, next_name)) => (index, name) = (next, next_name),
            Err(why) => {
                let short = previous.get(..7).unwrap_or(&previous);
                stopped = Some(format!("build {short}: {why}"));
                break;
            }
        }
    }
    Ok(Chain { notes, stopped })
}
