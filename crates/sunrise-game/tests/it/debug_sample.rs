//! `DedicatedServer.isTickTimeLoggingEnabled`: the real dedicated server sends a `DebugSample` tick
//! sample only to an operator who asked for `dedicated_server_tick_time`. Needs the prepared
//! resources; skipped without them.
use super::dedicated_walk::{boot, join, resources};
use std::{
    io::Write,
    time::{Duration, Instant},
};
use sunrise_protocol::{
    connection::Connection,
    packets::{
        Empty,
        play::{self, Clientbound as C, Serverbound as S},
    },
};

/// A joined player that answers what keeps it in the world: teleports, chunk batches, keep-alives.
struct Bot {
    c: Connection,
    loaded: bool,
}
impl Bot {
    fn send(&mut self, p: S) {
        self.c.send(&p.encode()).unwrap();
    }
    /// The next packet within 50 ms, after the replies it needs.
    fn step(&mut self) -> Option<C> {
        let (id, payload) = match self.c.recv() {
            Ok(packet) => packet,
            Err(e) if e.is_timed_out() => return None,
            Err(e) => panic!("connection lost: {e}"),
        };
        let packet = C::decode(id, &payload).ok()?;
        match &packet {
            C::PlayerPosition(p) => {
                self.send(S::AcceptTeleportation(play::AcceptTeleportation {
                    id: p.teleport,
                    x: p.position[0],
                    y: p.position[1],
                    z: p.position[2],
                    yaw: 0.0,
                    pitch: 0.0,
                }));
                if !self.loaded {
                    self.loaded = true;
                    self.send(S::PlayerLoaded(Empty));
                }
            }
            C::KeepAlive(k) => self.send(S::KeepAlive(k.clone())),
            C::ChunkBatchFinished(_) => {
                self.send(S::ChunkBatchReceived(play::ChunkBatchReceived {
                    chunks_per_tick: 20.0,
                }))
            }
            C::Disconnect(d) => panic!("disconnected: {d:?}"),
            _ => {}
        }
        Some(packet)
    }
    fn request(&mut self, subscriptions: Vec<i32>) {
        self.send(S::DebugSubscriptionRequest(
            play::DebugSubscriptionRequest { subscriptions },
        ));
    }
    /// The tick samples seen until `done` says stop, or `limit` passes.
    fn samples_until(
        &mut self,
        limit: Duration,
        mut done: impl FnMut(&C, &[Vec<i64>]) -> bool,
    ) -> Vec<Vec<i64>> {
        let deadline = Instant::now() + limit;
        let mut samples = Vec::new();
        while Instant::now() < deadline {
            let Some(packet) = self.step() else { continue };
            if let C::DebugSample(s) = &packet {
                assert_eq!(s.kind, play::DEBUG_SAMPLE_TICK_TIME);
                samples.push(s.sample.clone());
            }
            if done(&packet, &samples) {
                break;
            }
        }
        samples
    }
    fn until_chat(&mut self, marker: &str) -> Vec<Vec<i64>> {
        self.samples_until(
            Duration::from_secs(30),
            |p, _| matches!(p, C::SystemChat(c) if format!("{:?}", c.content).contains(marker)),
        )
    }
    fn first_sample(&mut self) -> Vec<Vec<i64>> {
        let samples = self.samples_until(Duration::from_secs(60), |_, s| !s.is_empty());
        assert!(!samples.is_empty(), "no sample arrived");
        samples
    }
    fn silence(&mut self) {
        let seen = self.samples_until(Duration::from_millis(1500), |_, _| false);
        assert!(seen.is_empty(), "samples after delivery stopped: {seen:?}");
    }
}

#[test]
#[ignore = "daily"]
fn a_dedicated_server_sends_tick_samples_only_to_an_operator_who_asked() {
    let Some(resources) = resources() else {
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let (_server, port, _read, _log, mut console) = boot(scratch.path(), &resources);
    let mut say = |line: &str| writeln!(console, "{line}").unwrap();
    let mut bot = Bot {
        c: join(port, "Sampler").unwrap(),
        loaded: false,
    };
    bot.c
        .reader
        .set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();

    // Not an operator: the request is stored and nothing is sent.
    bot.request(vec![0]);
    bot.silence();

    // An operator's request is honoured; teleporting far makes the chunks load in the waits.
    say("op Sampler");
    let mut samples = bot.first_sample();
    say("tp Sampler 3000 100 3000");
    samples.extend(bot.samples_until(Duration::from_secs(60), |_, s| s.iter().any(|v| v[2] > 0)));
    for v in &samples {
        assert_eq!(v.len(), 4, "{v:?}");
        assert!(v.iter().all(|n| *n >= 0), "{v:?}");
        assert!(
            v[0] >= v[1] + v[2] + v[3],
            "the tick holds method, tasks and idle: {v:?}"
        );
    }
    assert!(
        samples.iter().any(|v| v[2] > 0),
        "chunk work in the wait is scheduled work: {samples:?}"
    );

    // Deop stops it; re-op resumes it.
    say("deop Sampler");
    say("tellraw Sampler \"deopped\"");
    bot.until_chat("deopped");
    bot.silence();
    say("op Sampler");
    bot.first_sample();

    // Clearing the request stops it; asking again resumes it.
    bot.request(vec![]);
    bot.send(S::ChatCommand(play::ChatCommand {
        command: "tellraw @s \"cleared\"".into(),
    }));
    bot.until_chat("cleared");
    bot.silence();
    bot.request(vec![0]);
    bot.first_sample();
}
