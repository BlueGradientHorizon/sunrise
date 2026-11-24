//! The save screen freezes simulation, but keeps pumping window events while
//! snapshots are prepared in small batches and the region workers drain.
#[allow(unused_imports)]
use super::{Game, loading_frame};
#[allow(unused_imports)]
use std::{
    collections::{HashSet, VecDeque},
    sync::mpsc::{Receiver, TryRecvError},
    time::{Duration, Instant},
};
#[allow(unused_imports)]
use sunrise_client::{Result, hud::HudBatch};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::game) enum ClientQuit {
    Running,
    Requested,
    Stopping,
}

pub(in crate::game) struct ClosingSave {
    pub quit: bool,
    pub(in crate::game) close: bool,
    pub(in crate::game) reply: Option<std::sync::mpsc::Sender<String>>,
    pub(in crate::game) started: Instant,
    pub(in crate::game) stage: Stage,
}
pub(in crate::game) enum Stage {
    Generation,
    Chunks(VecDeque<(i32, i32)>),
    Entities(VecDeque<(i32, i32)>),
    Writes(Option<Receiver<std::io::Result<()>>>),
    Server,
    Quit,
}

// Bound the work in both sparse and block-entity-heavy worlds. A single chunk
// is indivisible; check the time budget after each snapshot, not each batch.
pub(in crate::game) const SNAPSHOTS_PER_FRAME: usize = 16;
pub(in crate::game) const SNAPSHOT_BUDGET: Duration = Duration::from_millis(2);
pub(in crate::game) const CLOSING_SNAPSHOTS_PER_FRAME: usize = 256;
pub(in crate::game) const CLOSING_SNAPSHOT_BUDGET: Duration = Duration::from_millis(10);

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::game::native_entity_refs::tests::with_game;
    #[allow(unused_imports)]
    use std::sync::{Arc, mpsc};
    #[allow(unused_imports)]
    use sunrise_client::save::{LevelShape, SaveDir};
    #[allow(unused_imports)]
    use sunrise_core::nbt::{DISK_WRITE_NESTING_GUARD, Tag};

    #[allow(unused_imports)]
    pub(in crate::game) use crate::game::client_side::saving_client::tests::finish_staged_save;

    #[test]
    #[ignore = "daily"]
    fn both_successful_save_paths_resolve_the_banner_in_the_active_language() {
        use sunrise_client::lang::{Language, scoped};

        mod_platform::testing::with_mods(Vec::new().into(), || {
            with_game(|game| {
                let dir = tempfile::tempdir().unwrap();
                game.server.world_root = dir.path().to_path_buf();
                game.save.0 = Arc::new(SaveDir::new(
                    game.server.world_root.clone(),
                    LevelShape::OVERWORLD,
                    Some(Default::default()),
                ));
                let translated = "[test locale]";
                let language = Language::of(
                    "en_test",
                    [("commands.save.success".into(), translated.into())],
                );
                scoped(language, || {
                    game.save_game().unwrap();
                    let explicit = game.client.message.take().unwrap().0;
                    game.pending_save.0 = Some(ClosingSave {
                        quit: false,
                        close: false,
                        reply: None,
                        started: Instant::now(),
                        stage: Stage::Writes(Some(game.save.0.flush_later().unwrap())),
                    });
                    assert!(!finish_staged_save(game));
                    assert!(game.pending_save.0.is_none());
                    let incremental = game.client.message.take().unwrap().0;
                    assert_eq!(
                        (explicit.as_str(), incremental.as_str()),
                        (translated, translated)
                    );
                });
            });
        });
    }

    #[test]
    #[ignore = "daily"]
    fn closing_and_control_saves_complete_after_reporting_a_record_failure() {
        mod_platform::testing::with_mods(Vec::new().into(), || {
            with_game(|game| {
                for quit in [false, true] {
                    let dir = tempfile::tempdir().unwrap();
                    game.save.0 = Arc::new(SaveDir::new(
                        dir.path().to_path_buf(),
                        LevelShape::OVERWORLD,
                        None,
                    ));
                    let invalid = (0..=DISK_WRITE_NESTING_GUARD)
                        .fold(Tag::Int(0), |nested, _| Tag::List(vec![nested]));
                    game.save
                        .0
                        .save_entities(vec![((2, 0), vec![invalid])])
                        .unwrap();
                    assert_eq!(game.save.0.record_save_failures(), 1);
                    let (send, receive) = mpsc::channel();
                    game.pending_save.0 = Some(ClosingSave {
                        quit,
                        close: quit,
                        reply: (!quit).then_some(send),
                        started: Instant::now(),
                        stage: Stage::Writes(Some(game.save.0.flush_later().unwrap())),
                    });
                    game.client.saved_for_exit = false;
                    game.client.message = None;
                    assert_eq!(finish_staged_save(game), quit);
                    assert!(game.pending_save.0.is_none());
                    if !quit {
                        let reply: serde_json::Value =
                            serde_json::from_str(&receive.try_recv().unwrap()).unwrap();
                        assert_eq!(reply, serde_json::json!({"ok": true}));
                    }
                    assert_eq!(game.client.saved_for_exit, quit);
                    assert_eq!(game.client.message.is_some(), !quit);
                }
            });
        });
    }
}
