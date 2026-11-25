//! Which files of a package are its source, and which of them are refused: the one file list and
//! the one binary rule, shared by the packer (`sunrise_mod_package::Package::write_files`), the
//! uses scan ([`crate::mod_uses`]) and the registry bot. The game's loader never calls them. Walking
//! a mod folder for them (`source_files`) is the `folder` feature, which only the tools enable.
#[cfg(feature = "folder")]
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use mod_surface::uses::USES_FILE;
#[cfg(feature = "folder")]
use std::{io, path::Path};

/// The manifest's file name in a `.smod` or a mod folder.
pub const MANIFEST: &str = "sunrise-mod.toml";

/// The folder of a package's runtime files, which `[runtime]` lists.
pub const RUNTIME_DIR: &str = "runtime";

/// Folders of a mod's project that are never part of its package: Git's, Cargo's build output
/// and the pinned SDK the mod tool fetches into `.sunrise/`.
pub const NOT_PACKAGED: [&str; 3] = [".git", "target", ".sunrise"];

/// Whether `name` (`/`-separated, relative to the package) is a source file: any path outside
/// [`NOT_PACKAGED`] folders that is not one of the package's own names, which are the manifest,
/// the uses table, the icon, and what is under `runtime/`, `assets/` and `data/`. A name is never
/// judged by its extension; a trailing `/` reads as a file below the folder.
pub fn source_file(name: &str) -> bool {
    !name.split('/').any(|part| NOT_PACKAGED.contains(&part))
        && name != MANIFEST
        && name != USES_FILE
        && name != "icon.png"
        && ![RUNTIME_DIR, "assets", "data"].iter().any(|folder| {
            name.strip_prefix(folder)
                .is_some_and(|rest| rest.starts_with('/'))
        })
}

/// The kind of prebuilt binary a file is, from its first bytes alone, whatever its name: ELF,
/// Mach-O (32- and 64-bit in both byte orders, and the universal header), PE, or an `ar` archive
/// (which covers `.a` and `.lib`). `head` is the file's first bytes, at least 8 of them when the
/// file has that many. The packer refuses a source file this names.
pub fn binary_kind(head: &[u8]) -> Option<&'static str> {
    match head {
        [0x7f, b'E', b'L', b'F', ..] => Some("ELF"),
        [0xfe, 0xed, 0xfa, 0xce | 0xcf, ..] | [0xce | 0xcf, 0xfa, 0xed, 0xfe, ..] => Some("Mach-O"),
        [0xca, 0xfe, 0xba, 0xbe, ..] | [0xbe, 0xba, 0xfe, 0xca, ..] => {
            Some("Mach-O universal binary or Java class file")
        }
        [b'M', b'Z', ..] => Some("PE"),
        _ if head.starts_with(b"!<arch>\n") => Some("ar archive"),
        _ => None,
    }
}

/// The source files a package of the folder at `root` carries, `/`-separated and sorted: the files
/// `cargo package` would put in the crate. The whole folder is walked, but not [`NOT_PACKAGED`]
/// folders, folders whose name starts with `.`, nested packages (a folder holding its own
/// `Cargo.toml`), the names [`source_file`] refuses, or what the root `.gitignore` ignores. Then
/// `[package] include` of `Cargo.toml` keeps only the files it matches, or else `[package]
/// exclude` drops the files it matches, both as gitignore-style patterns. `Cargo.toml` and
/// `Cargo.lock` at the root are always kept. No entry whose name a package path cannot hold (not
/// UTF-8, or with a `\` in it) is read. The walk keeps its own stack, so a deep tree needs no
/// recursion.
#[cfg(feature = "folder")]
pub fn source_files(root: &Path) -> io::Result<Vec<String>> {
    let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidData, message);
    let manifest = match std::fs::read_to_string(root.join("Cargo.toml")) {
        Ok(text) => text
            .parse::<toml::Table>()
            .map_err(|e| invalid(format!("Cargo.toml: {e}")))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => toml::Table::new(),
        Err(e) => return Err(e),
    };
    let patterns = |key: &str| -> io::Result<Option<Gitignore>> {
        let Some(value) = manifest.get("package").and_then(|package| package.get(key)) else {
            return Ok(None);
        };
        let list = value
            .as_array()
            .and_then(|list| {
                list.iter()
                    .map(toml::Value::as_str)
                    .collect::<Option<Vec<_>>>()
            })
            .ok_or_else(|| {
                invalid(format!(
                    "Cargo.toml: package.{key} must be a list of strings"
                ))
            })?;
        // Cargo reads an empty `include` as none.
        if list.is_empty() {
            return Ok(None);
        }
        // Every name matched here is relative to the folder: with an empty root the matcher
        // strips nothing from it, as a root that is itself a relative path would.
        let mut builder = GitignoreBuilder::new("");
        for pattern in list {
            builder.add_line(None, pattern).map_err(|e| {
                invalid(format!(
                    "Cargo.toml: package.{key}: pattern `{pattern}`: {e}"
                ))
            })?;
        }
        builder
            .build()
            .map(Some)
            .map_err(|e| invalid(format!("Cargo.toml: package.{key}: {e}")))
    };
    let include = patterns("include")?;
    let exclude = patterns("exclude")?;
    let mut ignored = GitignoreBuilder::new("");
    if root.join(".gitignore").is_file()
        && let Some(error) = ignored.add(root.join(".gitignore"))
    {
        return Err(invalid(format!(".gitignore: {error}")));
    }
    let ignored = ignored
        .build()
        .map_err(|e| invalid(format!(".gitignore: {e}")))?;
    let packaged = |name: &str, is_dir: bool| {
        if !is_dir && (name == "Cargo.toml" || name == "Cargo.lock") {
            return true;
        }
        if ignored
            .matched_path_or_any_parents(name, is_dir)
            .is_ignore()
        {
            return false;
        }
        match (&include, &exclude) {
            // Cargo checks no folder against `include`, which names files.
            (Some(include), _) => {
                is_dir || include.matched_path_or_any_parents(name, false).is_ignore()
            }
            (None, Some(exclude)) => !exclude
                .matched_path_or_any_parents(name, is_dir)
                .is_ignore(),
            (None, None) => true,
        }
    };

    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str().filter(|name| !name.contains('\\')) else {
                continue;
            };
            let name = if prefix.is_empty() {
                file_name.to_owned()
            } else {
                format!("{prefix}/{file_name}")
            };
            if kind.is_dir() {
                if file_name.starts_with('.')
                    || !source_file(&format!("{name}/"))
                    || entry.path().join("Cargo.toml").exists()
                    || !packaged(&name, true)
                {
                    continue;
                }
                pending.push((entry.path(), name));
            } else if source_file(&name) && packaged(&name, false) {
                found.push(name);
            }
        }
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::{binary_kind, source_file};

    #[test]
    fn a_source_file_is_any_path_but_the_packages_own_and_not_packaged_folders() {
        for name in [
            "Cargo.toml",
            "build.rs",
            "src/ffi/add.c",
            "vendor/zlib/inflate.h",
            "configure",
            "Makefile",
            "tools/gen.sh",
            "assetsx/a.c",
        ] {
            assert!(source_file(name), "{name}");
        }
        for name in [
            "sunrise-mod.toml",
            "uses.json",
            "icon.png",
            "runtime/table.bin",
            "assets/ruby/lang.json",
            "data/ruby/tags.json",
            "assets/",
            ".git/config",
            "target/debug/x.c",
            "src/target/x.c",
            ".sunrise/sdk/api/lib.rs",
        ] {
            assert!(!source_file(name), "{name}");
        }
    }

    #[test]
    fn binary_kind_reads_the_first_bytes_whatever_the_name() {
        for (head, kind) in [
            (&b"\x7fELF\x02\x01\x01\0"[..], Some("ELF")),
            (b"\xcf\xfa\xed\xfe\x07\0\0\x01", Some("Mach-O")),
            (b"\xfe\xed\xfa\xce", Some("Mach-O")),
            (
                b"\xca\xfe\xba\xbe\0\0\0\x02",
                Some("Mach-O universal binary or Java class file"),
            ),
            (b"MZ\x90\0", Some("PE")),
            (b"!<arch>\nfoo.o/", Some("ar archive")),
            (b"#include <stdio.h>", None),
            (b"!<arch>", None),
            (b"", None),
        ] {
            assert_eq!(binary_kind(head), kind, "{head:?}");
        }
    }
}

#[cfg(all(test, feature = "folder"))]
mod folder_tests {
    use super::source_files;
    use std::path::Path;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        for (name, text) in files {
            let path = root.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        root
    }

    fn names(root: &Path) -> Vec<String> {
        source_files(root).unwrap()
    }

    #[test]
    fn c_sources_and_headers_under_src_and_other_project_files_are_packaged() {
        let root = project(&[
            ("Cargo.toml", "[package]\nname = 'ruby'\n"),
            ("src/lib.rs", ""),
            ("src/ffi/add.c", "int add(int a, int b) { return a + b; }"),
            ("src/ffi/add.h", "int add(int, int);"),
            ("build.rs", ""),
            ("vendor/zlib/inflate.c", ""),
            // The package's own names are never source.
            ("sunrise-mod.toml", ""),
            ("assets/ruby/lang.json", ""),
            ("data/ruby/tags.json", ""),
            ("runtime/table.bin", ""),
            ("icon.png", ""),
        ]);
        assert_eq!(
            names(root.path()),
            [
                "Cargo.toml",
                "build.rs",
                "src/ffi/add.c",
                "src/ffi/add.h",
                "src/lib.rs",
                "vendor/zlib/inflate.c"
            ]
        );
    }

    #[test]
    fn include_names_the_only_files_a_package_carries() {
        let root = project(&[
            (
                "Cargo.toml",
                "[package]\nname = 'ruby'\ninclude = ['src/**', 'shim.c']\n",
            ),
            ("Cargo.lock", ""),
            ("src/lib.rs", ""),
            ("shim.c", ""),
            ("notes.txt", ""),
            ("vendor/zlib/inflate.c", ""),
        ]);
        assert_eq!(
            names(root.path()),
            ["Cargo.lock", "Cargo.toml", "shim.c", "src/lib.rs"]
        );
    }

    #[test]
    fn exclude_keeps_a_file_out() {
        let root = project(&[
            (
                "Cargo.toml",
                "[package]\nname = 'ruby'\nexclude = ['src/bench.c', 'docs/']\n",
            ),
            ("src/lib.rs", ""),
            ("src/bench.c", ""),
            ("src/add.c", ""),
            ("docs/guide.md", ""),
        ]);
        assert_eq!(
            names(root.path()),
            ["Cargo.toml", "src/add.c", "src/lib.rs"]
        );
    }

    #[test]
    fn a_nested_package_and_a_gitignored_file_are_left_out() {
        let root = project(&[
            ("Cargo.toml", "[package]\nname = 'ruby'\n"),
            (".gitignore", "out/\n*.o\n"),
            ("src/lib.rs", ""),
            ("src/cache.o", ""),
            ("out/generated.c", ""),
            ("vendor/dep/Cargo.toml", "[package]\nname = 'dep'\n"),
            ("vendor/dep/src/lib.rs", ""),
            ("vendor/plain.c", ""),
            (".cargo/config.toml", ""),
        ]);
        assert_eq!(
            names(root.path()),
            [".gitignore", "Cargo.toml", "src/lib.rs", "vendor/plain.c"]
        );
    }
}
