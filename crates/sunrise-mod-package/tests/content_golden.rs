//! Content declarations admitted (or refused) within a stable release line.
//! To add an entry, create its corpus folder and run this test with
//! `SUNRISE_BLESS_CONTENT_GOLDEN=1`. Do not re-bless existing entries to fix a mismatch.

use std::{collections::BTreeSet, fs, path::Path};
use sunrise_mod_package::{Content, Package};

const CORPUS: &str = "tests/content-corpus";
const GOLDEN: &str = "tests/content-golden";

#[test]
fn content_golden() {
    // Cargo runs integration tests from the crate root, even when invoked with -p
    // from the workspace. Keep the literal forward slashes in Content's Source::Dir.
    let entries = entry_names(CORPUS, true);
    let goldens = entry_names(GOLDEN, false);
    let bless = std::env::var("SUNRISE_BLESS_CONTENT_GOLDEN").as_deref() == Ok("1");
    assert!(!entries.is_empty(), "{CORPUS} is empty");
    let orphans: Vec<_> = goldens.difference(&entries).collect();
    assert!(
        orphans.is_empty(),
        "goldens without corpus entries: {orphans:?}"
    );
    if !bless {
        let missing: Vec<_> = entries.difference(&goldens).collect();
        assert!(
            missing.is_empty(),
            "corpus entries without goldens: {missing:?}"
        );
    }

    for entry in entries {
        let path = format!("{CORPUS}/{entry}");
        let (dump, refusal) = dump(Path::new(&path));
        assert_eq!(
            refusal,
            entry.starts_with("refused_"),
            "unexpected admission result for {entry}:\n{dump}"
        );
        let golden = format!("{GOLDEN}/{entry}.txt");
        if bless {
            fs::write(&golden, dump).unwrap_or_else(|e| panic!("{golden}: {e}"));
        } else {
            let expected = fs::read_to_string(&golden).unwrap_or_else(|e| panic!("{golden}: {e}"));
            if dump != expected {
                panic!("{entry} changed\n--- golden ---\n{expected}--- actual ---\n{dump}");
            }
        }
    }
}

fn entry_names(dir: &str, corpus: bool) -> BTreeSet<String> {
    fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
        .map(|entry| {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            let filename = entry.file_name().into_string().expect("UTF-8 fixture name");
            let name = if corpus {
                assert!(kind.is_dir(), "{dir}/{filename} is not a directory");
                &filename
            } else {
                assert!(kind.is_file(), "{dir}/{filename} is not a regular file");
                filename
                    .strip_suffix(".txt")
                    .expect("golden file ends in .txt")
            };
            assert!(
                !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                "invalid fixture name: {dir}/{filename}"
            );
            name.to_owned()
        })
        .collect()
}

fn dump(path: &Path) -> (String, bool) {
    let Package { manifest, .. } = match Package::read(path) {
        Ok(package) => package,
        Err(error) => return (format!("manifest:\nrefused: {error}\n"), true),
    };
    let content = Content::read(path, &manifest).and_then(|content| {
        match content.server_only_refusal(&manifest) {
            Some(error) => Err(error),
            None => Ok(content),
        }
    });
    let (content, refusal) = match content {
        Ok(content) => (format!("{content:#?}"), false),
        Err(error) => (format!("refused: {error}"), true),
    };
    (
        format!("manifest:\n{manifest:#?}\n\ncontent:\n{content}\n"),
        refusal,
    )
}
