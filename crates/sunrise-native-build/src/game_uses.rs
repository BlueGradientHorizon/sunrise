//! What a mod declares of the game's own code (`[[native.mixin]]` targets, `[[native.loop]]` loops and its `internals`), as entries of the list of what it uses: one function reads them from a build's build data, for the packer to record at the pinned build and for the fit check to compare at the installed one.
use std::path::Path;
use sunrise_mod_package::internals::{self, ListedInternal};
use sunrise_mod_package::mod_surface::uses::{Change, Entry, Misfit};
use sunrise_mod_package::{Manifest, build_data};
use sunrise_platform::mod_build::api_gen::{self, functions::FunctionIndex};

/// What every such entry's id starts with; no surface item's id does.
pub const PREFIX: &str = "game:";

/// The `[native]` table whose rows name game code no build data describes, so a mod with one works on the build it was made for only; `None` for a mod a list can judge.
pub fn unjudged(manifest: &Manifest) -> Option<&'static str> {
    let native = manifest.native.as_ref()?;
    [
        (native.module.is_empty(), "[[native.module]]"),
        (native.bridge.is_empty(), "[[native.bridge]]"),
        (native.state.is_empty(), "[[native.state]]"),
    ]
    .into_iter()
    .find_map(|(empty, table)| (!empty).then_some(table))
}

/// Whether `manifest` declares game code [`entries`] lists.
pub fn declared(manifest: &Manifest) -> bool {
    !manifest.internals.is_empty()
        || manifest
            .native
            .as_ref()
            .is_some_and(|native| !native.mixin.is_empty() || !native.r#loop.is_empty())
}

/// A declared piece of game code in one build.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Now {
    /// Its fingerprint there.
    Is(String),
    /// The build has no such item.
    Gone,
    /// The build has it, and its build data does not hold the code a fingerprint needs (the body of a function it does not index), so no list can show it unchanged.
    Unjudged,
}

/// The misfit of a mod that pins a horizon-mixins revision other than `revision`, the one the build's tool builds with: that build refuses the mod.
pub fn tool(manifest: &Manifest, revision: &str) -> Option<Misfit> {
    let native = manifest.native.as_ref()?;
    let pinned = native.mixins.as_deref()?;
    (!native.mixin.is_empty() && pinned != revision).then(|| Misfit {
        id: format!("{PREFIX}tool:horizon-mixins"),
        change: Change::Changed,
    })
}

/// The build-data file `file` of `dir`, which must describe `build`: a folder of another build's data would compare that build with itself.
fn document<T: serde::de::DeserializeOwned>(
    dir: &Path,
    file: &str,
    build: &str,
) -> Result<T, String> {
    let path = dir.join(file);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (described, body) =
        build_data::read::<T>(file, &text).map_err(|e| format!("{}: {e}", path.display()))?;
    if described.commit != build {
        return Err(format!(
            "{}: it describes game build {}, not {build}",
            path.display(),
            described.commit
        ));
    }
    Ok(body)
}

/// Each piece of game code `manifest` declares, by id, as it is in `build`, whose build data is in `dir`. A function is fingerprinted by signature and body, since a replacement or a call written for an old body could meet a different one; a type by its signature; a loop by what the game publishes of it, its two functions and its slot trait.
pub fn entries(manifest: &Manifest, dir: &Path, build: &str) -> Result<Vec<(String, Now)>, String> {
    let loops = manifest
        .native
        .as_ref()
        .is_some_and(|n| !n.r#loop.is_empty());
    Ok(
        Data::read_for(dir, build, loops || !manifest.internals.is_empty(), loops)?
            .entries(manifest),
    )
}

/// The game-code tables of one build, read once when judging many packages.
pub struct Data {
    index: FunctionIndex,
    internals: Vec<ListedInternal>,
    loops: Vec<api_gen::loops::PublishedLoop>,
}

impl Data {
    /// Read all build-data files, including those not used by any currently listed mod.
    pub fn read(dir: &Path, build: &str) -> Result<Self, String> {
        Self::read_for(dir, build, true, true)
    }

    fn read_for(dir: &Path, build: &str, internals: bool, loops: bool) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Loops {
            loops: Vec<api_gen::loops::PublishedLoop>,
        }
        #[derive(serde::Deserialize)]
        struct Internals {
            internals: Vec<ListedInternal>,
        }
        let index = document(dir, api_gen::functions::FILE, build)?;
        let internals = if internals {
            document::<Internals>(dir, build_data::INTERNALS, build)?.internals
        } else {
            Vec::new()
        };
        let loops = if loops {
            let published = document::<Loops>(dir, api_gen::loops::FILE, build)?.loops;
            if let Some(l) = published.iter().find(|l| !l.entry.contains("::")) {
                return Err(format!(
                    "{}: the loop `{}` has the entry `{}`, which is not a function's path",
                    dir.join(api_gen::loops::FILE).display(),
                    l.name,
                    l.entry
                ));
            }
            published
        } else {
            Vec::new()
        };
        Ok(Self {
            index,
            internals,
            loops,
        })
    }

    /// The game code this mod declares, as this build has it.
    pub fn entries(&self, manifest: &Manifest) -> Vec<(String, Now)> {
        let native = manifest.native.as_ref();
        let mixins = native.map_or(&[][..], |native| &native.mixin);
        let loops = native.map_or(&[][..], |native| &native.r#loop);
        let index = &self.index;
        let code = |target: &str| {
            index
                .functions
                .get(&api_gen::functions::key(target))
                .map(|f| format!("{}:{}", f.signature_hash, f.sha256))
        };
        let listed = &self.internals;
        let mut entries = Vec::new();
        for row in mixins {
            entries.push((
                format!("{PREFIX}fn:{}", row.target),
                code(&row.target).map_or(Now::Gone, Now::Is),
            ));
        }
        for row in loops {
            let now = match self.loops.iter().find(|l| l.name == row.name) {
                None => Now::Gone,
                Some(l) => {
                    let generic = l.generic_target();
                    // The slot trait lies in the module of the loop's functions.
                    let slot = index
                        .functions
                        .get(&api_gen::functions::key(&generic))
                        .and_then(|f| {
                            let path = format!("{}::{}", f.module, l.slot);
                            listed.iter().find(|i| i.path == path && i.kind == "trait")
                        });
                    match (code(&l.entry), code(&generic), slot) {
                        (Some(entry), Some(generic), Some(slot)) => Now::Is(format!(
                            "{}|{}|{}|{}|{entry}|{generic}|{}",
                            l.entry, l.generic, l.slot, l.side, slot.hash
                        )),
                        _ => Now::Unjudged,
                    }
                }
            };
            entries.push((format!("{PREFIX}loop:{}", row.name), now));
        }
        for declared in &manifest.internals {
            let found = internals::resolve(
                listed,
                &manifest.id,
                &declared.path,
                declared.kind.as_deref(),
            );
            // The id is what the mod declares, so a record is found again whatever the item became;
            // the kind it resolved to is part of the fingerprint.
            let id = format!(
                "{PREFIX}internal:{}:{}",
                declared.kind.as_deref().unwrap_or("any"),
                declared.path
            );
            let now = match found {
                Err(_) => Now::Gone,
                Ok(internal) => match internal.kind.as_str() {
                    "fn" | "method" => code(&internal.path).map_or(Now::Unjudged, |code| {
                        Now::Is(format!("{}:{}:{code}", internal.kind, internal.hash))
                    }),
                    kind => Now::Is(format!("{kind}:{}", internal.hash)),
                },
            };
            entries.push((id, now));
        }
        entries.sort();
        entries.dedup();
        entries
    }
}

/// How a build's [`entries`] stand against a list's record of them.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Compared {
    /// What is gone or differs in more than just its body, in item ID order.
    pub breaks: Vec<Misfit>,
    /// Game functions whose bodies changed with their signatures and declarations unchanged.
    pub bodies: Vec<BodyChange>,
    /// Whether the list has no record of something the mod declares that the build can fingerprint: packing the mod again writes it.
    pub unrecorded: bool,
    /// The ids the build cannot fingerprint ([`Now::Unjudged`]).
    pub unjudged: Vec<String>,
}

/// A game-code item whose only differences were function bodies.
#[derive(Debug, PartialEq, Eq)]
pub struct BodyChange {
    pub id: String,
    pub functions: Vec<String>,
}

impl Compared {
    /// The standalone build check still requires even a body change to be reviewed by the author.
    pub fn take_misfits(&mut self) -> impl Iterator<Item = Misfit> + use<> {
        std::mem::take(&mut self.breaks).into_iter().chain(
            std::mem::take(&mut self.bodies)
                .into_iter()
                .map(|change| Misfit {
                    id: change.id,
                    change: Change::Changed,
                }),
        )
    }
}

/// `now`, a build's [`entries`], against the list's `recorded` entries.
pub fn compare(recorded: &[&Entry], now: Vec<(String, Now)>) -> Compared {
    let mut compared = Compared::default();
    for (id, now) in now {
        let made = recorded.iter().find(|entry| entry.id == id);
        let change = match (made, now) {
            (_, Now::Gone) => Change::Removed,
            (_, Now::Unjudged) => {
                compared.unjudged.push(id);
                continue;
            }
            (None, Now::Is(_)) => {
                compared.unrecorded = true;
                continue;
            }
            (Some(made), Now::Is(now)) => {
                if made.hash == now {
                    continue;
                }
                if let Some(functions) = only_bodies(&id, &made.hash, &now) {
                    compared.bodies.push(BodyChange { id, functions });
                    continue;
                }
                Change::Changed
            }
        };
        compared.breaks.push(Misfit { id, change });
    }
    compared
}

/// Interpret the existing uses.json fingerprints. An older or incomplete fingerprint may not
/// record the signature and body separately; a difference then cannot prove a body-only change.
fn only_bodies(id: &str, old: &str, new: &str) -> Option<Vec<String>> {
    // After matching complete signatures, report whether the bodies differ.
    fn body(old: &str, new: &str) -> Option<bool> {
        let (old_sig, old_body) = old.split_once(':')?;
        let (new_sig, new_body) = new.split_once(':')?;
        (old_sig == new_sig
            && !old_sig.is_empty()
            && !old_body.is_empty()
            && !new_body.is_empty()
            && !old_body.contains(':')
            && !new_body.contains(':'))
        .then_some(old_body != new_body)
    }
    if let Some(path) = id.strip_prefix("game:fn:") {
        return body(old, new)?.then(|| vec![path.to_owned()]);
    }
    if let Some(internal) = id.strip_prefix("game:internal:") {
        let (_, path) = internal.split_once(':')?;
        let (old_kind, old) = old.split_once(':')?;
        let (kind, new) = new.split_once(':')?;
        let (old_decl, old) = old.split_once(':')?;
        let (decl, new) = new.split_once(':')?;
        return (matches!(kind, "fn" | "method")
            && old_kind == kind
            && old_decl == decl
            && body(old, new)?)
        .then(|| vec![path.to_owned()]);
    }
    if id.starts_with("game:loop:") {
        let old: Vec<_> = old.split('|').collect();
        let new: Vec<_> = new.split('|').collect();
        let [
            old_entry,
            old_generic,
            old_slot,
            old_side,
            old_outer,
            old_inner,
            old_trait,
        ] = old.as_slice()
        else {
            return None;
        };
        let [entry, generic, slot, side, outer, inner, slot_trait] = new.as_slice() else {
            return None;
        };
        if [old_entry, old_generic, old_slot, old_side, old_trait]
            != [entry, generic, slot, side, slot_trait]
        {
            return None;
        }
        let outer_changed = body(old_outer, outer)?;
        let inner_changed = body(old_inner, inner)?;
        let (owner, _) = entry.rsplit_once("::")?;
        let mut paths = Vec::new();
        if outer_changed {
            paths.push((*entry).to_owned());
        }
        if inner_changed {
            paths.push(format!("{owner}::{generic}"));
        }
        return (!paths.is_empty()).then_some(paths);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_review_requires_every_signature_and_declaration_to_match() {
        let internal = "game:internal:any:client::run";
        let loop_item = "game:loop:run";
        let matched = "client::run|run_with|Slot|client|entry:old|generic:old|trait";
        let changed = "client::run|run_with|Slot|client|entry:new|generic:new|trait";
        for (id, old, new, review) in [
            (
                internal,
                "fn:decl:sig:old",
                "fn:decl:sig:new",
                &["client::run"][..],
            ),
            (
                loop_item,
                matched,
                changed,
                &["client::run", "client::run_with"][..],
            ),
            (
                internal,
                "fn:old-decl:sig:old",
                "fn:new-decl:sig:new",
                &[][..],
            ),
            (
                loop_item,
                matched,
                "client::run|run_with|Slot|client|entry:new|other-sig:new|trait",
                &[][..],
            ),
            // The old recipe had no executable fingerprints for a loop or called internal.
            (internal, "fn:decl", "fn:decl:sig:new", &[][..]),
            (
                loop_item,
                "client::run|run_with|Slot|client",
                changed,
                &[][..],
            ),
        ] {
            let old = Entry::plain(id.into(), old.into());
            let compared = compare(&[&old], vec![(id.into(), Now::Is(new.into()))]);
            if review.is_empty() {
                assert!(compared.bodies.is_empty(), "{compared:?}");
                assert_eq!(
                    compared.breaks,
                    [Misfit {
                        id: id.into(),
                        change: Change::Changed
                    }]
                );
            } else {
                assert!(compared.breaks.is_empty(), "{compared:?}");
                assert_eq!(
                    compared.bodies,
                    [BodyChange {
                        id: id.into(),
                        functions: review.iter().map(|p| (*p).to_owned()).collect()
                    }]
                );
            }
        }
    }
}
