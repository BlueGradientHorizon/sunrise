//! The advisory lookup honors Cargo's `net.offline` without spawning Cargo.
use serde::Deserialize;
use std::{
    collections::HashSet,
    env,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

pub(super) fn online() -> bool {
    let Ok(cwd) = env::current_dir() else {
        return false;
    };
    let home = env::var_os("CARGO_HOME")
        .filter(|home| !home.is_empty())
        .map(|home| cwd.join(home))
        .or_else(|| env::home_dir().map(|home| home.join(".cargo")));
    // A failed config read must not turn an offline tool into an online one.
    offline(
        &cwd,
        home.as_deref(),
        env::var_os("CARGO_NET_OFFLINE").as_deref(),
    )
    .is_ok_and(|offline| !offline)
}

fn offline(cwd: &Path, cargo_home: Option<&Path>, env: Option<&OsStr>) -> io::Result<bool> {
    if let Some(value) = env {
        return match value.to_str() {
            Some("true") => Ok(true),
            Some("false") => Ok(false),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid CARGO_NET_OFFLINE",
            )),
        };
    }
    // Nearest directories first, Cargo home last. Cargo prefers the legacy name if both exist.
    let dirs = cwd
        .ancestors()
        .map(|dir| dir.join(".cargo"))
        .chain(cargo_home.map(Path::to_path_buf));
    for dir in dirs {
        let legacy = dir.join("config");
        let file = if legacy.is_file() {
            legacy
        } else {
            dir.join("config.toml")
        };
        if let Some(value) = read_offline(&file, true, &mut HashSet::new())? {
            return Ok(value);
        }
    }
    Ok(false)
}

#[derive(Deserialize)]
struct Config {
    #[serde(default)]
    net: Net,
    #[serde(default)]
    include: Vec<Include>,
}

#[derive(Default, Deserialize)]
struct Net {
    offline: Option<bool>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Include {
    Path(PathBuf),
    Table {
        path: PathBuf,
        #[serde(default)]
        optional: bool,
    },
}

fn read_offline(
    path: &Path,
    optional: bool,
    active: &mut HashSet<PathBuf>,
) -> io::Result<Option<bool>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if optional && error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let config: Config = toml::from_str(&text).map_err(io::Error::other)?;
    if config.net.offline.is_some() || config.include.is_empty() {
        return Ok(config.net.offline);
    }
    let identity = path.canonicalize()?;
    if !active.insert(identity.clone()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cyclic Cargo config includes",
        ));
    }
    // Top-level values win; later includes win over earlier ones, independently of hierarchy.
    for include in config.include.into_iter().rev() {
        let (include, optional) = match include {
            Include::Path(path) => (path, false),
            Include::Table { path, optional } => (path, optional),
        };
        if include.extension() != Some(OsStr::new("toml")) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Cargo config includes must end in .toml",
            ));
        }
        if let Some(value) = read_offline(&path.with_file_name(include), optional, active)? {
            return Ok(Some(value));
        }
    }
    active.remove(&identity);
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_and_nearest_config_win_over_parent_and_cargo_home() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("project/.cargo");
        let local = root.path().join("project/child/.cargo");
        let home = root.path().join("cargo-home");
        for dir in [&parent, &local, &home] {
            fs::create_dir_all(dir).unwrap();
        }
        let cwd = local.parent().unwrap();
        let offline = |value| super::offline(cwd, Some(&home), value).unwrap();
        fs::write(home.join("config.toml"), "[net]\noffline = true").unwrap();
        assert!(offline(None));
        fs::write(parent.join("config.toml"), "net.offline = false").unwrap();
        assert!(!offline(None));
        fs::write(local.join("config.toml"), "net.offline = true").unwrap();
        assert!(offline(None));
        fs::write(local.join("config"), "net.offline = false").unwrap();
        assert!(!offline(None));
        assert!(offline(Some(OsStr::new("true"))));
        fs::write(local.join("config"), "net.offline = true").unwrap();
        assert!(!offline(Some(OsStr::new("false"))));
    }

    #[test]
    fn includes_are_read_from_last_to_first_and_yield_to_the_including_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let includes = r#"include = ["first.toml", "nested.toml", { path = "missing.toml", optional = true }]"#;
        fs::write(&path, includes).unwrap();
        fs::write(dir.path().join("first.toml"), "net.offline = false").unwrap();
        fs::write(
            dir.path().join("nested.toml"),
            r#"include = ["offline.toml"]"#,
        )
        .unwrap();
        fs::write(dir.path().join("offline.toml"), "net.offline = true").unwrap();
        assert!(offline(dir.path(), Some(dir.path()), None).unwrap());
        fs::write(&path, format!("{includes}\nnet.offline = false")).unwrap();
        assert!(!offline(dir.path(), Some(dir.path()), None).unwrap());
    }
}
