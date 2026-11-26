use std::collections::HashMap;

#[derive(Default)]
pub(in crate::game) struct NativeNet {
    pub(in crate::game) placement: super::native_book_placement::State,
    pub(in crate::game) book_connections:
        HashMap<u32, sunrise_sim::recipe_book::settings_connection::Connection>,
    pub(in crate::game) book_view: sunrise_sim::recipe_book::settings_sync::ClientView,
    pub(in crate::game) book_request: u64,
    pub(in crate::game) book_ready: bool,
    pub(in crate::game) book_retry_at: Option<std::time::Instant>,
}
