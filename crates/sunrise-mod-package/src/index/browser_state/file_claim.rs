use same_file::Handle;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// A file claim under the locked home, including a file or directory not created yet.
/// Use native lookup for the existing prefix and a conservative spelling for the rest.
pub(super) struct FileClaim {
    folded: Vec<String>,
    parent: Handle,
    parent_depth: usize,
    file: Option<Handle>,
}

impl FileClaim {
    pub(super) fn read(home: &Path, claim: &str) -> Result<Self, String> {
        let mut path = home.to_path_buf();
        let mut folded = Vec::new();
        let mut parent_depth = 0;
        let mut file = None;
        let mut parts = claim
            .split(|c| c == '/' || (cfg!(windows) && c == '\\'))
            .peekable();
        while let Some(part) = parts.next() {
            let leaf = parts.peek().is_none();
            match part {
                "." if !leaf => continue,
                ".." if !leaf && folded.pop().is_some() => {
                    path.pop();
                    parent_depth = parent_depth.min(folded.len());
                    continue;
                }
                _ => {}
            }
            if !portable_name(part) {
                return Err(format!("invalid file-claim component {part:?}"));
            }
            path.push(part);
            let mut exists = false;
            if parent_depth == folded.len() {
                match fs::symlink_metadata(&path) {
                    Ok(info) => {
                        #[cfg(windows)]
                        let link = {
                            use std::os::windows::fs::MetadataExt;
                            use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
                            info.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
                        };
                        #[cfg(not(windows))]
                        let link = info.is_symlink();
                        // Even a directory canceled by `..` was looked up. It may not be a link:
                        // following one can change what `..` means or leave the selected home.
                        let expected_type = if leaf { info.is_file() } else { info.is_dir() };
                        if link || !expected_type {
                            return Err(format!(
                                "{} is not a plain {}",
                                path.display(),
                                if leaf { "file" } else { "directory" }
                            ));
                        }
                        exists = true;
                        if leaf {
                            file = Some(
                                Handle::from_path(&path)
                                    .map_err(|e| format!("{}: {e}", path.display()))?,
                            );
                        } else {
                            parent_depth += 1;
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::NotFound => {}
                    Err(e) => return Err(format!("{}: {e}", path.display())),
                }
            }
            // A nonexistent name doesn't have a native identity yet. Don't try to
            // predict Unicode equivalence or a future Win32 short name.
            if !exists && (!part.is_ascii() || part.contains('~')) {
                return Err(format!(
                    "cannot compare missing file-claim component {part:?}"
                ));
            }
            folded.push(part.to_ascii_lowercase());
        }
        for _ in parent_depth..folded.len() {
            path.pop();
        }
        let parent = Handle::from_path(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self {
            folded,
            parent,
            parent_depth,
            file,
        })
    }

    pub(super) fn conflicts_with(&self, other: &Self) -> bool {
        // One claim cannot use the other's file as a directory, even if neither exists yet.
        let overlap = |a: &[String], b: &[String]| a.starts_with(b) || b.starts_with(a);
        overlap(&self.folded, &other.folded)
            || (self.parent == other.parent
                && overlap(
                    &self.folded[self.parent_depth..],
                    &other.folded[other.parent_depth..],
                ))
            || (self.file.is_some() && self.file == other.file)
    }
}

// Apply Win32's filename restrictions even on Unix, since homes can be shared or moved.
fn portable_name(part: &str) -> bool {
    if part.is_empty()
        || part.ends_with(['.', ' '])
        || part
            .bytes()
            .any(|b| b < b' ' || b"<>:\"/\\|?*".contains(&b))
    {
        return false;
    }
    let (stem, _) = part.split_once('.').unwrap_or((part, ""));
    let stem = stem.trim_end_matches(' ').to_ascii_uppercase();
    !(matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|n| {
            matches!(
                n,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        }))
}
