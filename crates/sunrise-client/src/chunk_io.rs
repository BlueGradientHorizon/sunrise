//! Server reports for the window, as in `IntegratedServer.reportChunkLoadFailure`,
//! `reportChunkSaveFailure` and `sendLowDiskSpaceWarning`. The server and its I/O
//! workers report here; only the receiving client admits their toasts (`Minecraft.execute`).
//! `SystemToast.addOrUpdate` keeps only the newest chunk failure of each kind.
use crate::{save, text::Text, toasts::SystemToast};
use std::{
    path::Path,
    sync::{Arc, LazyLock, Mutex},
};

/// What the receiver has yet to display.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pending {
    /// `SystemToast.onLowDiskSpace`.
    pub low_disk_space: bool,
    /// `SystemToast.onChunkLoadFailure`: the newest chunk.
    pub load_failure: Option<(i32, i32)>,
    /// `SystemToast.onChunkSaveFailure`: the newest chunk.
    pub save_failure: Option<(i32, i32)>,
    /// Mod IDs whose delayed callbacks panicked, in report order.
    pub scheduled_task_failures: Vec<&'static str>,
}

/// The receiver's inbox, shared with the integrated server and its region workers.
#[derive(Default)]
pub struct Reports(Mutex<Pending>);

/// Dedicated-server I/O reports; a client window has its own inbox.
pub static PROCESS_REPORTS: LazyLock<Arc<Reports>> = LazyLock::new(Arc::default);

impl Pending {
    /// `reportChunkLoadFailure` after `warnOnLowDiskSpace` found `low_disk_space`.
    fn load_failed(&mut self, x: i32, z: i32, low_disk_space: bool) {
        self.low_disk_space |= low_disk_space;
        self.load_failure = Some((x, z));
    }

    /// `reportChunkSaveFailure` after `warnOnLowDiskSpace` found `low_disk_space`.
    fn save_failed(&mut self, x: i32, z: i32, low_disk_space: bool) {
        self.low_disk_space |= low_disk_space;
        self.save_failure = Some((x, z));
    }
}

/// `MinecraftServer.warnOnLowDiskSpace` for the world folder `dir`.
fn warn_on_low_disk_space(dir: &Path) -> bool {
    let low = save::low_disk_space(dir);
    if low {
        // `MinecraftServer.sendLowDiskSpaceWarning`.
        log::warn!("Low disk space! Might not be able to save the world.");
    }
    low
}

impl Reports {
    fn pending(&self) -> std::sync::MutexGuard<'_, Pending> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `reportChunkLoadFailure` for the chunk at (`x`, `z`) of the dimension in `dir`.
    pub fn report_load_failure(&self, dir: &Path, x: i32, z: i32, error: &dyn std::fmt::Display) {
        log::error!("Failed to load chunk {x},{z}: {error}");
        let low = warn_on_low_disk_space(dir);
        self.pending().load_failed(x, z, low);
    }

    /// `reportChunkSaveFailure`.
    pub fn report_save_failure(&self, dir: &Path, x: i32, z: i32, error: &dyn std::fmt::Display) {
        log::error!("Failed to save chunk {x},{z}: {error}");
        let low = warn_on_low_disk_space(dir);
        self.pending().save_failed(x, z, low);
    }

    /// `Minecraft.sendLowDiskSpaceWarning`: a server's `ClientboundLowDiskSpaceWarningPacket`
    /// (`ClientPacketListener.handleLowDiskSpaceWarning`) toasts like the local report.
    pub fn report_low_disk_space(&self) {
        self.pending().low_disk_space = true;
    }

    /// Report a mod's failed callback to the integrated server owner's client.
    pub fn report_scheduled_task_failure(&self, mod_id: &'static str) {
        self.pending().scheduled_task_failures.push(mod_id);
    }

    /// The reports since the last call.
    pub fn take(&self) -> Pending {
        std::mem::take(&mut *self.pending())
    }
}

/// `ChatFormatting.RED`.
fn red() -> u32 {
    crate::hud::rgba(255, 85, 85, 255)
}

/// `SystemToast.onLowDiskSpace` (`SystemToastId.LOW_DISK_SPACE`: ten seconds).
pub fn low_disk_space_toast() -> SystemToast {
    SystemToast::new(
        Text::tr("chunk.toast.lowDiskSpace", &[]),
        Some(Text::tr("chunk.toast.lowDiskSpace.description", &[])),
    )
    .with_display_time(SystemToast::LONG_DISPLAY_TIME)
}

fn chunk_toast(key: &str, x: i32, z: i32) -> SystemToast {
    // `Component.translationArg(ChunkPos)` is `ChunkPos.toString`.
    let pos = format!("[{x}, {z}]");
    SystemToast::new(
        Text::tr(key, &[&pos]),
        Some(Text::tr("chunk.toast.checkLog", &[])),
    )
    .with_title_color(red())
}

/// `SystemToast.onChunkLoadFailure`.
pub fn load_failure_toast(x: i32, z: i32) -> SystemToast {
    chunk_toast("chunk.toast.loadFailure", x, z)
}

/// `SystemToast.onChunkSaveFailure`.
pub fn save_failure_toast(x: i32, z: i32) -> SystemToast {
    chunk_toast("chunk.toast.saveFailure", x, z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_keep_the_newest_chunk_of_each_kind() {
        let mut pending = Pending::default();
        pending.load_failed(1, 2, false);
        pending.load_failed(3, 4, false);
        pending.save_failed(5, 6, false);
        assert_eq!(
            pending,
            Pending {
                low_disk_space: false,
                load_failure: Some((3, 4)),
                save_failure: Some((5, 6)),
                ..Pending::default()
            }
        );
        // A later failure on a full disk raises the warning, and one with
        // space left does not lower it.
        pending.save_failed(7, 8, true);
        pending.load_failed(9, 10, false);
        assert_eq!(
            pending,
            Pending {
                low_disk_space: true,
                load_failure: Some((9, 10)),
                save_failure: Some((7, 8)),
                ..Pending::default()
            }
        );
    }

    #[test]
    fn a_servers_warning_packet_toasts_low_disk_space() {
        let reports = Reports::default();
        reports.report_low_disk_space();
        assert!(reports.take().low_disk_space);
    }

    #[test]
    fn low_disk_space_toast_stays_ten_seconds() {
        let toast = low_disk_space_toast();
        assert_eq!(toast.display_time(), 10000.0);
        assert_eq!(load_failure_toast(0, 0).display_time(), 5000.0);
    }
}
