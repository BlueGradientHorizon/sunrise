use std::path::{Path, PathBuf};

/// The folder `SUNRISE_PROFILE` names; captures go there instead of the game home's `reports/`.
pub(in crate::game) const ENV_DIR: &str = "SUNRISE_PROFILE";

/// The server side's capture state: the integrated server thread's history/capture and guests' remote requests.
#[derive(Default)]
pub(in crate::game) struct ServerProfile {
    /// The server thread's health history and its side of a capture; set on its own Game only.
    pub(in crate::game) server: Option<super::integrated::capture::ServerSide>,
    /// The guests' requests to this server and the capture they ask of it (perf_remote.rs).
    pub(in crate::game) served: super::perf_remote::Served,
}

pub(in crate::game) fn reports_dir() -> PathBuf {
    sunrise_sim::perf_types::reports_dir(
        std::env::var_os(ENV_DIR).as_deref(),
        &sunrise_level::logging::game_home(),
    )
}

/// The path as the player sees it: under the game home's folder when it is there.
pub(in crate::game) fn shown(path: &Path) -> String {
    shown_in(path, &sunrise_level::logging::game_home())
}

/// `path` under `home` as `reports/profile-….hzperf`, with `/` on every platform so replies and chat read the same
/// everywhere; a path outside `home` as the platform writes it, ready to paste into its file manager.
pub(in crate::game) fn shown_in(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(inside) => inside
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        Err(_) => path.display().to_string(),
    }
}

pub(in crate::game) fn reply_text<T: serde::Serialize>(reply: &T) -> String {
    serde_json::to_string(reply).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capture_under_the_home_is_shown_with_slashes() {
        let home = std::env::temp_dir().join("sunrise-home");
        let inside = home.join("reports").join("profile-20261113-190003Z.hzperf");
        assert_eq!(
            shown_in(&inside, &home),
            "reports/profile-20261113-190003Z.hzperf"
        );
        let elsewhere = std::env::temp_dir().join("captures").join("a.hzperf");
        assert_eq!(shown_in(&elsewhere, &home), elsewhere.display().to_string());
    }
}
