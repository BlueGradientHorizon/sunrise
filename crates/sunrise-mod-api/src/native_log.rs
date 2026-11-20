//! Sink for the separately compiled `log` facade in prebuilt native mods.
use std::fmt;

// The `log` 0.4 discriminants; no type from either copy crosses the dylib boundary.
fn level(raw: u8) -> Option<log::Level> {
    Some(match raw {
        1 => log::Level::Error,
        2 => log::Level::Warn,
        3 => log::Level::Info,
        4 => log::Level::Debug,
        5 => log::Level::Trace,
        _ => return None,
    })
}

/// Whether the game's logger keeps a record of this level and target.
pub fn enabled(raw: u8, target: &str) -> bool {
    level(raw).is_some_and(|level| {
        level <= log::max_level()
            && log::logger().enabled(&log::Metadata::builder().level(level).target(target).build())
    })
}

/// Submit a mod record to the game's logger, preserving the source of the diagnostic.
pub fn record(
    raw: u8,
    target: &str,
    args: fmt::Arguments<'_>,
    file: Option<&str>,
    line: Option<u32>,
) {
    if let Some(level) = level(raw)
        && level <= log::max_level()
    {
        log::logger().log(
            &log::Record::builder()
                .level(level)
                .target(target)
                .args(args)
                .file(file)
                .line(line)
                .build(),
        );
    }
}

/// Flush the destination of the forwarded records.
pub fn flush() {
    log::logger().flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn sends_the_source_record_at_the_games_level() {
        struct Capture(Mutex<Vec<(log::Level, String, String, String, u32)>>);
        impl log::Log for Capture {
            fn enabled(&self, meta: &log::Metadata<'_>) -> bool {
                meta.target() == "greeter::registration"
            }
            fn log(&self, record: &log::Record<'_>) {
                if self.enabled(record.metadata()) {
                    self.0.lock().unwrap().push((
                        record.level(),
                        record.target().to_owned(),
                        record.args().to_string(),
                        record.file().unwrap().to_owned(),
                        record.line().unwrap(),
                    ));
                }
            }
            fn flush(&self) {}
        }
        static CAPTURE: Capture = Capture(Mutex::new(Vec::new()));
        log::set_logger(&CAPTURE).unwrap();
        log::set_max_level(log::LevelFilter::Info);
        let target = "greeter::registration";
        let file = "src/greeter.rs";
        assert!(enabled(3, target));
        assert!(!enabled(3, "muted"));
        assert!(!enabled(4, target));
        record(
            3,
            target,
            format_args!("registered {}", "Gizmo"),
            Some(file),
            Some(23),
        );
        record(4, target, format_args!("disabled"), Some(file), Some(24));
        assert_eq!(
            *CAPTURE.0.lock().unwrap(),
            [(
                log::Level::Info,
                target.into(),
                "registered Gizmo".into(),
                file.into(),
                23
            )],
        );
    }
}
