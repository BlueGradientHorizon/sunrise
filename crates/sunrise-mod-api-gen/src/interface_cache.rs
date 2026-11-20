//! Build-local cache for walks that inspect signatures and module declarations, never bodies.
use quote::ToTokens;
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use syn::visit_mut::VisitMut;

thread_local! {
    static DIRECTORY: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    /// This scope's parses by path, with the source each was made from.
    static PARSED: RefCell<HashMap<PathBuf, (String, syn::File)>> = RefCell::new(HashMap::new());
}

pub(crate) fn with_cache<T>(
    out: &Path,
    run: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    // A new generator executable invalidates all summaries, including after parser upgrades.
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let identity = Sha256::digest(fs::read(executable).map_err(|e| e.to_string())?);
    let directory = out.join(format!("interfaces-{identity:x}"));
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    struct Reset(Option<PathBuf>);
    impl Drop for Reset {
        fn drop(&mut self) {
            DIRECTORY.with(|slot| *slot.borrow_mut() = self.0.take());
            PARSED.with(|parsed| parsed.borrow_mut().clear());
        }
    }
    let _reset = Reset(DIRECTORY.with(|slot| slot.replace(Some(directory))));
    run()
}

/// The scope's entry `name`: its body if it reads back intact (a digest line, then the body).
/// Entries are disposable, never authoritative: a missing, truncated or corrupt one is `None`.
pub(crate) fn load(name: &str) -> Result<Option<String>, String> {
    let Some(directory) = DIRECTORY.with(|slot| slot.borrow().clone()) else {
        return Ok(None);
    };
    match fs::read(directory.join(name)) {
        Ok(bytes) => Ok(String::from_utf8(bytes).ok().and_then(|text| {
            let (digest, body) = text.split_once('\n')?;
            (digest == format!("{:x}", Sha256::digest(body.as_bytes()))).then(|| body.to_owned())
        })),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// Writes the scope's entry `name` through a temporary file and a rename, so a reader (the other
/// build script, a crash) never sees a partial entry.
pub(crate) fn store(name: &str, body: &str) -> Result<(), String> {
    let Some(directory) = DIRECTORY.with(|slot| slot.borrow().clone()) else {
        return Ok(());
    };
    let bytes = format!("{:x}\n{body}", Sha256::digest(body.as_bytes()));
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temporary = directory.join(format!(
        "{name}.tmp-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    output
        .write_all(bytes.as_bytes())
        .map_err(|e| e.to_string())?;
    drop(output);
    fs::rename(temporary, directory.join(name)).map_err(|e| e.to_string())
}

/// `file`'s signatures: its items with every function body emptied. Inside a scope the result is
/// cached on disk by the file's path and content, and remembered for the rest of the scope, so
/// the module walk and the point scan share one parse of each file.
pub(crate) fn parse(path: &Path, source: &str) -> Result<syn::File, String> {
    if DIRECTORY.with(|slot| slot.borrow().is_none()) {
        return syn::parse_file(source).map_err(|e| format!("{}: {e}", path.display()));
    }
    if let Some(file) = PARSED.with(|parsed| {
        parsed
            .borrow()
            .get(path)
            .filter(|(known, _)| known == source)
            .map(|(_, file)| file.clone())
    }) {
        return Ok(file);
    }
    let mut key = Sha256::new();
    key.update(path.as_os_str().as_encoded_bytes());
    key.update([0]);
    key.update(source.as_bytes());
    let name = format!("{:x}.cache", key.finalize());
    // A truncated entry (even an empty but syntactically valid Rust file) must never remove
    // points silently: `load` checks its digest.
    let file = match load(&name)?.and_then(|text| syn::parse_file(&text).ok()) {
        Some(file) => file,
        None => {
            let mut file =
                syn::parse_file(source).map_err(|e| format!("{}: {e}", path.display()))?;
            struct Signatures;
            impl VisitMut for Signatures {
                fn visit_item_fn_mut(&mut self, function: &mut syn::ItemFn) {
                    function.block.stmts.clear();
                }
                fn visit_impl_item_fn_mut(&mut self, function: &mut syn::ImplItemFn) {
                    function.block.stmts.clear();
                }
                fn visit_trait_item_fn_mut(&mut self, function: &mut syn::TraitItemFn) {
                    if let Some(body) = &mut function.default {
                        body.stmts.clear();
                    }
                }
            }
            Signatures.visit_file_mut(&mut file);
            store(&name, &file.to_token_stream().to_string())?;
            file
        }
    };
    PARSED.with(|parsed| {
        parsed
            .borrow_mut()
            .insert(path.to_owned(), (source.to_owned(), file.clone()))
    });
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_entries_regenerate_and_scope_resets_after_error() {
        let out = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("interface-cache-unit-{}", std::process::id()));
        let path = Path::new("example.rs");
        let source = "pub fn example(value: i32) -> i32 { value + 1 }";
        with_cache(&out, || {
            let cold = parse(path, source)?.to_token_stream().to_string();
            assert_eq!(cold, parse(path, source)?.to_token_stream().to_string());
            let directory = DIRECTORY.with(|slot| slot.borrow().clone().unwrap());
            let cached = fs::read_dir(directory)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            for corrupt in ["", "truncated", "wrong-digest\nfn another() {}"] {
                fs::write(&cached, corrupt).unwrap();
                PARSED.with(|parsed| parsed.borrow_mut().clear());
                assert_eq!(cold, parse(path, source)?.to_token_stream().to_string());
            }
            // An edit to the file is seen at once, and an entry that fails its digest is not read.
            let edited = parse(
                path,
                "pub fn example(value: i32) -> i32 { value + 2 }\npub fn added() {}",
            )?;
            assert_eq!(edited.items.len(), 2);
            store("entry", "types")?;
            assert_eq!(load("entry")?.as_deref(), Some("types"));
            let entry = DIRECTORY
                .with(|slot| slot.borrow().clone().unwrap())
                .join("entry");
            fs::write(&entry, "0\ntypes").unwrap();
            assert_eq!(load("entry")?, None);
            assert_eq!(load("absent")?, None);
            Ok(())
        })
        .unwrap();
        let failed: Result<(), String> = with_cache(&out, || Err("expected failure".into()));
        assert!(failed.is_err());
        assert!(DIRECTORY.with(|slot| slot.borrow().is_none()));
        let original = parse(path, source).unwrap();
        let syn::Item::Fn(function) = &original.items[0] else {
            panic!("missing function")
        };
        assert_eq!(function.block.stmts.len(), 1);
        fs::remove_dir_all(out).unwrap();
    }
}
