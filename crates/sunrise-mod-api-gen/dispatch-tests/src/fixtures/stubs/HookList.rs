mod hook_list {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../sunrise-mod-vocab/src/hook_list.rs"
    ));
}
pub(super) use hook_list::{HookList, HookListJournal, HookListStorage};
