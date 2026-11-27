//! Per-connection outgoing play packet capture behind `net-trace` and `SUNRISE_PACKET_CAPTURE`.
use super::Body;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, Once, PoisonError,
        atomic::{AtomicI64, Ordering},
        mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
    },
    thread::JoinHandle,
};

const CHANNEL_CAPACITY: usize = 8192;

fn decode_packet_id(bytes: &[u8]) -> i32 {
    let mut value: u32 = 0;
    let mut shift: u32 = 0;
    for &b in bytes.iter().take(5) {
        value |= u32::from(b & 0x7F) << shift;
        if b & 0x80 == 0 {
            return value as i32;
        }
        shift += 7;
    }
    -1
}

fn sanitize_filename(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if clean.is_empty() {
        "player".to_owned()
    } else {
        clean
    }
}

fn sanitize_field(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '\t' | '\r' | '\n' => ' ',
            other => other,
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PacketMeta {
    pub(crate) id: i32,
    pub(crate) len: usize,
    pub(crate) hash: u128,
}

impl PacketMeta {
    pub(crate) fn from_body(body: &[u8]) -> Self {
        Self {
            id: decode_packet_id(body),
            len: body.len(),
            hash: xxhash_rust::xxh3::xxh3_128(body),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct QueuedTrace {
    pub(super) tick: i64,
    pub(super) meta: Option<PacketMeta>,
}

enum Payload {
    Meta(PacketMeta),
    Body(Body),
}

enum Cmd {
    Open {
        session_id: usize,
        file: BufWriter<File>,
    },
    Packet {
        session_id: usize,
        seq: u64,
        tick: i64,
        payload: Payload,
    },
    Close {
        session_id: usize,
        expected_count: u64,
        incomplete_reason: Option<&'static str>,
    },
    Shutdown,
}

struct SessionEntry {
    conn: u32,
    ordinal: u32,
    player: String,
    filename: String,
    status: String,
    count: u64,
    fallback_close: Option<(u64, &'static str)>,
}

#[derive(Default)]
struct RunState {
    finished: bool,
    player_ordinals: HashMap<String, u32>,
    sessions: Vec<SessionEntry>,
}

pub(super) struct CaptureRun {
    dir: PathBuf,
    run_id: String,
    clock: Arc<AtomicI64>,
    state: Arc<Mutex<RunState>>,
    tx: SyncSender<Cmd>,
    writer: Mutex<Option<JoinHandle<()>>>,
}

impl CaptureRun {
    pub(super) fn from_env(lan_incarnation: u64) -> Option<Arc<Self>> {
        let path = std::env::var_os("SUNRISE_PACKET_CAPTURE")?;
        if path.is_empty() {
            return None;
        }
        let dir = PathBuf::from(path);
        match Self::open_in(&dir, lan_incarnation) {
            Ok(run) => Some(run),
            Err(e) => {
                log::error!("[lan] packet capture disabled for {}: {e}", dir.display());
                None
            }
        }
    }

    pub(super) fn open_in(dir: &Path, lan_incarnation: u64) -> io::Result<Arc<Self>> {
        std::fs::create_dir_all(dir)?;
        if std::fs::read_dir(dir)?.next().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "packet capture directory must be empty",
            ));
        }
        let run_id = format!("{}-{lan_incarnation}", std::process::id());
        let state = Arc::new(Mutex::new(RunState::default()));
        let (tx, rx) = sync_channel(CHANNEL_CAPACITY);
        let thread_dir = dir.to_path_buf();
        let thread_run_id = run_id.clone();
        let thread_state = state.clone();
        let handle = std::thread::Builder::new()
            .name("sunrise-pkt-cap".into())
            .spawn(move || writer_loop(thread_dir, thread_run_id, thread_state, rx))?;
        Ok(Arc::new(Self {
            dir: dir.to_path_buf(),
            run_id,
            clock: Arc::new(AtomicI64::new(0)),
            state,
            tx,
            writer: Mutex::new(Some(handle)),
        }))
    }

    pub(super) fn set_tick(&self, tick: i64) {
        self.clock.store(tick, Ordering::Relaxed);
    }

    pub(super) fn clock(&self) -> Arc<AtomicI64> {
        self.clock.clone()
    }

    pub(super) fn open_session(self: &Arc<Self>, conn: u32, player: &str) -> Option<SessionWriter> {
        let clean_player = sanitize_field(player);
        let filename = format!("{}-{conn}.pkt", sanitize_filename(player));
        let path = self.dir.join(&filename);
        let (session_id, ordinal) = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if state.finished {
                return None;
            }
            let ordinal = state
                .player_ordinals
                .entry(clean_player.clone())
                .and_modify(|n| *n += 1)
                .or_insert(1);
            let ordinal = *ordinal;
            let session_id = state.sessions.len();
            state.sessions.push(SessionEntry {
                conn,
                ordinal,
                player: clean_player.clone(),
                filename: filename.clone(),
                status: "incomplete:unclosed".to_owned(),
                count: 0,
                fallback_close: None,
            });
            (session_id, ordinal)
        };
        let opened = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|file| {
                let mut out = BufWriter::new(file);
                writeln!(
                    out,
                    "# v2\t{}\t{conn}\t{ordinal}\t{clean_player}",
                    self.run_id
                )?;
                out.flush()?;
                Ok(out)
            });
        let out = match opened {
            Ok(out) => out,
            Err(e) => {
                log::warn!("[lan] packet capture open {}: {e}", path.display());
                let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
                state.sessions[session_id].filename = "-".to_owned();
                state.sessions[session_id].status = "incomplete:open_failed".to_owned();
                return None;
            }
        };
        if self
            .tx
            .send(Cmd::Open {
                session_id,
                file: out,
            })
            .is_err()
        {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.sessions[session_id].status = "incomplete:writer_closed".to_owned();
            return None;
        }
        Some(SessionWriter {
            session_id,
            seq: 0,
            overflowed: false,
            closed: false,
            clock: self.clock.clone(),
            tx: self.tx.clone(),
            state: self.state.clone(),
        })
    }

    pub(super) fn finish(&self) {
        let handle = {
            let mut guard = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
            guard.take()
        };
        let Some(handle) = handle else {
            return;
        };
        {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.finished = true;
        }
        let _ = self.tx.send(Cmd::Shutdown);
        let _ = handle.join();
    }
}

impl Drop for CaptureRun {
    fn drop(&mut self) {
        self.finish();
    }
}

pub(super) struct SessionWriter {
    session_id: usize,
    seq: u64,
    overflowed: bool,
    closed: bool,
    clock: Arc<AtomicI64>,
    tx: SyncSender<Cmd>,
    state: Arc<Mutex<RunState>>,
}

impl SessionWriter {
    pub(super) fn current_tick(&self) -> i64 {
        self.clock.load(Ordering::Relaxed)
    }

    pub(super) fn record(&mut self, tick: i64, body: Body, meta: Option<PacketMeta>) {
        let seq = self.seq;
        self.seq += 1;
        if self.overflowed {
            return;
        }
        let payload = match meta {
            Some(meta) => Payload::Meta(meta),
            None => Payload::Body(body),
        };
        match self.tx.try_send(Cmd::Packet {
            session_id: self.session_id,
            seq,
            tick,
            payload,
        }) {
            Ok(()) => {}
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.overflowed = true;
                static LOGGED: Once = Once::new();
                LOGGED.call_once(|| {
                    log::warn!("[lan] packet capture channel overflowed; marking run incomplete");
                });
            }
        }
    }

    pub(super) fn close(mut self, reason: Option<&'static str>) {
        self.finish_close(reason);
    }

    fn finish_close(&mut self, reason: Option<&'static str>) {
        if self.closed {
            return;
        }
        self.closed = true;
        let incomplete_reason = reason.or(self.overflowed.then_some("overflow"));
        let cmd = Cmd::Close {
            session_id: self.session_id,
            expected_count: self.seq,
            incomplete_reason,
        };
        if self.tx.try_send(cmd).is_err() {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.sessions[self.session_id].fallback_close =
                Some((self.seq, incomplete_reason.unwrap_or("overflow")));
        }
    }
}

impl Drop for SessionWriter {
    fn drop(&mut self) {
        if !self.closed {
            self.finish_close(Some("dropped"));
        }
    }
}

struct OpenFile {
    out: BufWriter<File>,
    written: u64,
    io_error: bool,
}

fn finalize_file(
    mut file: OpenFile,
    expected_count: u64,
    incomplete_reason: Option<&'static str>,
) -> (String, u64) {
    let reason = if file.io_error {
        Some("write_error")
    } else if let Some(reason) = incomplete_reason {
        Some(reason)
    } else if file.written != expected_count {
        Some("count_mismatch")
    } else {
        None
    };
    if let Some(reason) = reason {
        if !file.io_error {
            let _ = writeln!(file.out, "# end\t{}\tincomplete\t{reason}", file.written);
            let _ = file.out.flush();
        }
        return (format!("incomplete:{reason}"), file.written);
    }
    let ok = writeln!(file.out, "# end\t{expected_count}\tok")
        .and_then(|()| file.out.flush())
        .is_ok();
    if ok {
        ("ok".to_owned(), expected_count)
    } else {
        ("incomplete:write_error".to_owned(), file.written)
    }
}

fn writer_loop(dir: PathBuf, run_id: String, state: Arc<Mutex<RunState>>, rx: Receiver<Cmd>) {
    let mut files: HashMap<usize, OpenFile> = HashMap::new();
    let mut logged_write_err = false;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Open { session_id, file } => {
                files.insert(
                    session_id,
                    OpenFile {
                        out: file,
                        written: 0,
                        io_error: false,
                    },
                );
            }
            Cmd::Packet {
                session_id,
                seq,
                tick,
                payload,
            } => {
                let Some(file) = files.get_mut(&session_id) else {
                    continue;
                };
                if file.io_error {
                    continue;
                }
                let meta = match payload {
                    Payload::Meta(meta) => meta,
                    Payload::Body(body) => PacketMeta::from_body(&body),
                };
                if let Err(e) = writeln!(
                    file.out,
                    "{seq}\t{tick}\t{}\t{}\t{:032x}",
                    meta.id, meta.len, meta.hash
                ) {
                    file.io_error = true;
                    if !logged_write_err {
                        logged_write_err = true;
                        log::warn!("[lan] packet capture write failed: {e}");
                    }
                } else {
                    file.written += 1;
                }
            }
            Cmd::Close {
                session_id,
                expected_count,
                incomplete_reason,
            } => {
                if let Some(file) = files.remove(&session_id) {
                    let (status, count) = finalize_file(file, expected_count, incomplete_reason);
                    let mut st = state.lock().unwrap_or_else(PoisonError::into_inner);
                    st.sessions[session_id].status = status;
                    st.sessions[session_id].count = count;
                }
            }
            Cmd::Shutdown => break,
        }
    }
    let mut st = state.lock().unwrap_or_else(PoisonError::into_inner);
    for (session_id, file) in files {
        let (expected, reason) = st.sessions[session_id]
            .fallback_close
            .unwrap_or((file.written, "unclosed"));
        let (status, count) = finalize_file(file, expected, Some(reason));
        st.sessions[session_id].status = status;
        st.sessions[session_id].count = count;
    }
    let run_status = if st.sessions.iter().all(|s| s.status == "ok") {
        "ok"
    } else {
        "incomplete"
    };
    let manifest_path = dir.join("manifest.tsv");
    if let Err(e) = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&manifest_path)
        .and_then(|file| {
            let mut out = BufWriter::new(file);
            writeln!(out, "# manifest v1\t{run_id}\t{run_status}")?;
            for entry in &st.sessions {
                writeln!(
                    out,
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    entry.conn,
                    entry.ordinal,
                    entry.player,
                    entry.filename,
                    entry.status,
                    entry.count
                )?;
            }
            out.flush()
        })
    {
        log::warn!(
            "[lan] packet capture manifest write {}: {e}",
            manifest_path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_packet_id_handles_single_multibyte_and_empty() {
        assert_eq!(decode_packet_id(&[46, 0, 1]), 46);
        assert_eq!(decode_packet_id(&[0x80, 0x01, 9]), 128);
        assert_eq!(decode_packet_id(&[]), -1);
    }

    #[test]
    fn rejects_nonempty_capture_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("stale.pkt"), b"old\n").unwrap();
        let err = match CaptureRun::open_in(dir.path(), 1) {
            Ok(_) => panic!("expected nonempty directory refusal"),
            Err(err) => err,
        };
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    }

    #[test]
    fn writes_complete_footer_and_manifest_with_ordinals() {
        let dir = tempfile::tempdir().unwrap();
        let run = CaptureRun::open_in(dir.path(), 3).unwrap();
        run.set_tick(12);
        let mut first = run.open_session(1, "Alice").unwrap();
        first.record(12, Body::from(vec![8u8, 1, 2]), None);
        first.close(None);
        let mut second = run.open_session(4, "Alice").unwrap();
        second.record(15, Body::from(vec![50u8, 9]), None);
        second.close(None);
        run.finish();

        let manifest = std::fs::read_to_string(dir.path().join("manifest.tsv")).unwrap();
        let mlines: Vec<&str> = manifest.lines().collect();
        assert_eq!(mlines.len(), 3);
        assert!(mlines[0].starts_with("# manifest v1\t"));
        assert!(mlines[0].ends_with("\tok"));
        assert_eq!(mlines[1], "1\t1\tAlice\tAlice-1.pkt\tok\t1");
        assert_eq!(mlines[2], "4\t2\tAlice\tAlice-4.pkt\tok\t1");

        let pkt1 = std::fs::read_to_string(dir.path().join("Alice-1.pkt")).unwrap();
        assert!(pkt1.ends_with("# end\t1\tok\n"));
        let pkt2 = std::fs::read_to_string(dir.path().join("Alice-4.pkt")).unwrap();
        assert!(pkt2.ends_with("# end\t1\tok\n"));
    }
}
