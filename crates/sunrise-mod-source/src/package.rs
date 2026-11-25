//! Which files of a mod folder are its package's source: the one selection that packing
//! (`sunrise_mod_package::Package::directory_files`) and the uses scan ([`crate::mod_uses`]) share.
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use mod_surface::uses::USES_FILE;
use std::{io, path::Path};

/// The manifest's file name in a `.smod` or a mod folder.
pub const MANIFEST: &str = "sunrise-mod.toml";

/// The folder of a package's runtime files, which `[runtime]` lists.
pub const RUNTIME_DIR: &str = "runtime";

/// Folders of a mod's project that are never part of its package: Git's, Cargo's build output
/// and the pinned SDK the mod tool fetches into `.sunrise/`.
pub const NOT_PACKAGED: [&str; 3] = [".git", "target", ".sunrise"];

/// Whether `name` (`/`-separated, relative to the package) is one of the package's own names:
/// the manifest, the uses table, the icon, and what is under `runtime/`, `assets/` and `data/`.
/// Any other file of a native package is source.
pub fn reserved(name: &str) -> bool {
    name == MANIFEST
        || name == USES_FILE
        || name == "icon.png"
        || [RUNTIME_DIR, "assets", "data"].iter().any(|folder| {
            name.strip_prefix(folder)
                .is_some_and(|rest| rest.starts_with('/'))
        })
}

/// The source files a package of the folder at `root` carries, `/`-separated and sorted: the files
/// `cargo package` would put in the crate. The whole folder is walked, but not [`NOT_PACKAGED`]
/// folders, folders whose name starts with `.`, nested packages (a folder holding its own
/// `Cargo.toml`), the package's [`reserved`] names, or what the root `.gitignore` ignores. Then
/// `[package] include` of `Cargo.toml` keeps only the files it matches, or else `[package]
/// exclude` drops the files it matches, both as gitignore-style patterns. `Cargo.toml` and
/// `Cargo.lock` at the root are always kept. No entry whose name a package path cannot hold (not
/// UTF-8, or with a `\` in it) is read. The walk keeps its own stack, so a deep tree needs no
/// recursion.
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
                // `reserved` reads a trailing `/` as a file below the folder.
                if file_name.starts_with('.')
                    || NOT_PACKAGED.contains(&file_name)
                    || reserved(&format!("{name}/"))
                    || entry.path().join("Cargo.toml").exists()
                    || !packaged(&name, true)
                {
                    continue;
                }
                pending.push((entry.path(), name));
            } else if !reserved(&name) && packaged(&name, false) {
                found.push(name);
            }
        }
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
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
