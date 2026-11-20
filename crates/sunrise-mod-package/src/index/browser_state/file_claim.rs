use std::ffi::OsStr;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Component, Path};

/// A claim anchored to the real directory that contains its file or missing suffix.
pub(super) struct FileClaim {
    parent: Identity,
    // An uncheckable spelling is ambiguous only inside its resolved parent.
    tail: Option<Vec<String>>,
    file: Option<Identity>,
}

impl FileClaim {
    pub(super) fn target(home: &Path, claim: &str) -> Result<Self, String> {
        let mut depth = 0;
        let mut parts = claim
            .split(|c| c == '/' || (cfg!(windows) && c == '\\'))
            .peekable();
        while let Some(part) = parts.next() {
            match part {
                "." if parts.peek().is_some() => {}
                ".." if depth > 0 && parts.peek().is_some() => depth -= 1,
                _ if portable_name(part) => depth += 1,
                _ => return Err(format!("invalid destination component {part:?}")),
            }
        }
        Self::read(home, claim, true)
    }

    pub(super) fn other(home: &Path, claim: &str) -> Result<Self, String> {
        Self::read(home, claim, false)
    }

    fn read(home: &Path, claim: &str, target: bool) -> Result<Self, String> {
        let path = home.join(claim);
        // Another row may use an absolute path. Use the filesystem's root in that case,
        // not this home; a real alias can be reached from either starting point.
        let base = if path.starts_with(home) {
            home
        } else if path.is_absolute() {
            path.ancestors().last().unwrap_or(&path)
        } else {
            return Err("cannot locate the file claim's starting directory".into());
        };
        let mut parent = base.to_path_buf();
        let mut parent_info = None;
        let mut parts = path
            .strip_prefix(base)
            .map_err(|e| e.to_string())?
            .components()
            .peekable();
        let mut missing = Vec::<(&OsStr, bool)>::new();
        let mut tail = None;
        let mut file = None;
        while let Some(part) = parts.next() {
            let leaf = parts.peek().is_none();
            if part == Component::ParentDir && !leaf && missing.pop().is_some() {
                continue;
            }
            let name = part.as_os_str();
            if !missing.is_empty() {
                missing.push((name, true));
                continue;
            }
            let current = parent.join(name);
            let metadata = if leaf && target {
                fs::symlink_metadata(&current)
            } else {
                fs::metadata(&current)
            };
            match metadata {
                Ok(info) if leaf => {
                    if target && (!info.is_file() || is_link(&info)) {
                        return Err(format!("{} is not a plain file", current.display()));
                    }
                    if info.is_file() {
                        file = Some(
                            identity(&current, Some(&info))
                                .map_err(|e| format!("{}: {e}", current.display()))?,
                        );
                    }
                    tail = name
                        .to_str()
                        .filter(|name| portable_name(name))
                        .map(|name| vec![name.to_ascii_lowercase()]);
                }
                Ok(info) if info.is_dir() => {
                    parent = current;
                    parent_info = Some(info);
                }
                Ok(_) if !target => missing.push((name, false)),
                Ok(_) => return Err(format!("{} is not a directory", current.display())),
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    // Do not mistake a dangling link for a new filename in this directory.
                    // We cannot associate a dangling link with a checked destination.
                    match fs::symlink_metadata(&current) {
                        Err(e) if e.kind() == ErrorKind::NotFound => missing.push((name, true)),
                        Err(e) if !target && invalid_lookup(&e) => missing.push((name, false)),
                        Err(e) => return Err(format!("{}: {e}", current.display())),
                        Ok(_) => return Err(format!("cannot resolve {}", current.display())),
                    }
                }
                // No file can be reached through a rejected name or a non-directory.
                // This does not make rows under other directories ambiguous.
                Err(e) if !target && invalid_lookup(&e) => missing.push((name, false)),
                // For example, permission denied may hide a link; no identity to bound it.
                Err(e) => return Err(format!("{}: {e}", current.display())),
            }
        }
        if !missing.is_empty() {
            tail = missing
                .into_iter()
                .map(|(name, foldable)| {
                    let name = name.to_str()?;
                    (foldable && portable_name(name) && name.is_ascii() && !name.contains('~'))
                        .then(|| name.to_ascii_lowercase())
                })
                .collect();
        }
        let parent = identity(&parent, parent_info.as_ref())
            .map_err(|e| format!("{}: {e}", parent.display()))?;
        Ok(Self { parent, tail, file })
    }

    pub(super) fn conflicts_with(&self, other: &Self) -> bool {
        (self.file.is_some() && self.file == other.file)
            || (self.parent == other.parent
                && match (&self.tail, &other.tail) {
                    (Some(a), Some(b)) => a.starts_with(b) || b.starts_with(a),
                    _ => true,
                })
    }
}

#[cfg(unix)]
type Identity = (u64, u64);
#[cfg(not(unix))]
type Identity = same_file::Handle;

#[cfg(unix)]
fn identity(path: &Path, info: Option<&fs::Metadata>) -> io::Result<Identity> {
    use std::os::unix::fs::MetadataExt;
    // A stat gives the same identity without requiring permission to read the file.
    let read;
    let info = match info {
        Some(info) => info,
        None => {
            read = fs::metadata(path)?;
            &read
        }
    };
    Ok((info.dev(), info.ino()))
}

#[cfg(not(unix))]
fn identity(path: &Path, _info: Option<&fs::Metadata>) -> io::Result<Identity> {
    same_file::Handle::from_path(path)
}

fn is_link(info: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        info.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        info.is_symlink()
    }
}

fn invalid_lookup(error: &std::io::Error) -> bool {
    // ERROR_INVALID_NAME is not consistently mapped to InvalidInput in std.
    #[cfg(windows)]
    if error.raw_os_error() == Some(windows_sys::Win32::Foundation::ERROR_INVALID_NAME as i32) {
        return true;
    }
    matches!(
        error.kind(),
        ErrorKind::InvalidInput | ErrorKind::NotADirectory
    )
}

// Apply Win32's filename restrictions to destinations even on Unix, since homes can be moved.
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
