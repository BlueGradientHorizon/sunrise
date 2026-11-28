//! `RetiredPackBuilds` (#6721): a pack build dropped before it ended is kept until it ends, and
//! the renderer's drop waits for it.
use super::{PackBuild, RetiredPackBuilds, pack_post};
use std::sync::mpsc;

/// A build that ends when `release` sends (or is dropped), refused with `reason`.
fn build(reason: &'static str) -> (PackBuild, mpsc::Sender<()>) {
    let (release, wait) = mpsc::channel::<()>();
    let thread = std::thread::spawn(move || {
        let _ = wait.recv();
        Ok((
            Err(pack_post::Refused {
                reason: reason.into(),
                notes: Vec::new(),
            }),
            Vec::new(),
        ))
    });
    (thread, release)
}

fn count(retired: &RetiredPackBuilds) -> usize {
    retired.lock().len()
}

#[test]
fn reap_keeps_running_builds_and_joins_ended_ones() {
    let retired = RetiredPackBuilds::default();
    let (running, keep) = build("running");
    let (ended, release) = build("ended");
    retired.retire(running);
    retired.retire(ended);
    retired.reap();
    assert_eq!(count(&retired), 2, "neither build has ended");
    release.send(()).unwrap();
    while !retired.lock().iter().any(|t| t.is_finished()) {
        std::thread::yield_now();
    }
    retired.reap();
    assert_eq!(count(&retired), 1, "only the ended build is joined");
    drop(keep);
    retired.join_all();
    assert_eq!(count(&retired), 0);
}

#[test]
fn join_all_waits_for_every_build() {
    let retired = RetiredPackBuilds::default();
    let (first, first_release) = build("first");
    let (second, second_release) = build("second");
    retired.retire(first);
    retired.retire(second);
    let ended = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let waiter = {
        let (retired, ended) = (retired.clone(), ended.clone());
        std::thread::spawn(move || {
            retired.join_all();
            ended.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
    };
    first_release.send(()).unwrap();
    // `join_all` is still waiting for the second build.
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(ended.load(std::sync::atomic::Ordering::SeqCst), 0);
    second_release.send(()).unwrap();
    waiter.join().unwrap();
    assert_eq!(ended.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(count(&retired), 0);
}
