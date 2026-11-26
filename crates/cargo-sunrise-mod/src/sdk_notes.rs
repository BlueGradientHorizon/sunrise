//! The change notes of the SDK builds between two builds, and the ones that explain a failed
//! build of a mod. Each build's `sdk.json` carries the notes of the changes since the build
//! before it (`surface.previous`), so a mod that moves across several builds follows that chain.
//! Only each build's small files are fetched ([`crate::sdk_fetch::fetch_build`]).
use crate::sdk_fetch::PinnedSdk;
use mod_surface::scan::{Token, tokens};
use mod_surface::{Surface, leaf};
use std::collections::BTreeSet;
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

/// A note that explains a failed build, with the ids of it the mod's source names.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Explained<'a> {
    pub names: Vec<&'a str>,
    pub shipped: &'a Shipped,
}

/// The notes of `notes` that name an id `surface` no longer holds whose final name occurs as an
/// identifier in `sources` (file and text), each once, with those names.
pub(crate) fn explaining<'a>(
    notes: &'a [Shipped],
    surface: &Surface,
    sources: &[(String, String)],
) -> Vec<Explained<'a>> {
    let present: BTreeSet<&str> = surface.entries().map(|(id, _)| id).collect();
    let written: BTreeSet<&str> = sources
        .iter()
        .flat_map(|(_, text)| tokens(text))
        .filter_map(|(token, _)| match token {
            Token::Ident(name) => Some(name),
            _ => None,
        })
        .collect();
    notes
        .iter()
        .filter_map(|shipped| {
            let mut names: Vec<&str> = shipped
                .note
                .ids
                .iter()
                .filter(|id| !present.contains(id.as_str()))
                .map(|id| leaf(id))
                .filter(|name| written.contains(name))
                .collect();
            names.sort_unstable();
            names.dedup();
            (!names.is_empty()).then_some(Explained { names, shipped })
        })
        .collect()
}

/// `error` followed by the notes that explain it and, after them, why the chain stopped early.
/// With no note, `error` as it is.
pub(crate) fn with_notes(error: String, explained: &[Explained<'_>], chain: &Chain) -> String {
    if explained.is_empty() {
        return error;
    }
    let mut text = format!("{error}\n\nChange notes that explain it:\n");
    for item in explained {
        let names: Vec<String> = item.names.iter().map(|name| format!("`{name}`")).collect();
        text.push_str(&format!(
            "  {}: changed in {}\n      why: {}\n",
            names.join(", "),
            item.shipped.build,
            item.shipped.note.why
        ));
        if let Some(migrate) = &item.shipped.note.migrate {
            text.push_str(&format!("      migrate: {migrate}\n"));
        }
    }
    if let Some(line) = chain.stopped_line() {
        text.push_str(&line);
        text.push('\n');
    }
    text
}
