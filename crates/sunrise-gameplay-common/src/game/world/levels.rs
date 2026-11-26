

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

pub fn queue_unencodable_entity_record(save: &SaveDir) {
        use sunrise_core::nbt::{DISK_WRITE_NESTING_GUARD, Tag};
        let invalid =
            (0..=DISK_WRITE_NESTING_GUARD).fold(Tag::Int(0), |nested, _| Tag::List(vec![nested]));
        save.save_entities_later(vec![((2, 0), vec![invalid])])
            .unwrap();
    }
#[cfg(test)]
struct LogCapture;
pub static LOGGED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
#[cfg(test)]
impl log::Log for LogCapture {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            LOGGED.lock().unwrap().push(record.args().to_string());
        }
        fn flush(&self) {}
    }
}
