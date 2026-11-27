//! Per-connection outgoing play packet recorder gated on `net-trace` and
//! `SUNRISE_PACKET_CAPTURE=<dir>`.
use super::Body;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, PoisonError,
        atomic::{AtomicI64, Ordering},
    },
};
use sunrise_protocol::{connection::Framing, frame};

static CURRENT_TICK: AtomicI64 = AtomicI64::new(0);

pub fn set_tick(tick: i64) {
    CURRENT_TICK.store(tick, Ordering::Relaxed);
}

pub fn current_tick() -> i64 {
    CURRENT_TICK.load(Ordering::Relaxed)
}

fn capture_dir() -> Option<&'static PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let path = std::env::var_os("SUNRISE_PACKET_CAPTURE")?;
        if path.is_empty() {
            return None;
        }
        let dir = PathBuf::from(path);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("[lan] packet capture mkdir {}: {e}", dir.display());
            return None;
        }
        Some(dir)
    })
    .as_ref()
}

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

fn sanitize_header(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '\t' | '\r' | '\n' => ' ',
            other => other,
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PacketMeta {
    pub(super) id: i32,
    pub(super) len: usize,
    pub(super) hash: u128,
}

impl PacketMeta {
    pub(super) fn from_body(body: &[u8]) -> Self {
        Self {
            id: decode_packet_id(body),
            len: body.len(),
            hash: xxhash_rust::xxh3::xxh3_128(body),
        }
    }

    pub(super) fn from_prepared(bytes: &[u8], framing: Framing) -> Self {
        match framing {
            Framing::Memory | Framing::Network(None) => Self::from_body(bytes),
            Framing::Network(Some(threshold)) => {
                let mut inflater = frame::Inflater::default();
                match frame::body(bytes, Some(threshold), &mut inflater) {
                    Ok(frame::Body::Plain(start)) => Self::from_body(&bytes[start..]),
                    Ok(frame::Body::Inflated(raw)) => Self::from_body(&raw),
                    Err(_) => Self::from_body(bytes),
                }
            }
        }
    }

    pub(super) fn from_lan_body(body: &Body, framing: Framing) -> Self {
        if body.is_prepared() {
            Self::from_prepared(body, framing)
        } else {
            Self::from_body(body)
        }
    }
}

struct Sink {
    out: BufWriter<File>,
    seq: u64,
}

pub(super) struct Recorder {
    sink: Mutex<Sink>,
}

impl Recorder {
    pub(super) fn for_connection(conn: u32, player: &str) -> Option<Arc<Self>> {
        let dir = capture_dir()?;
        match Self::open_in(dir, conn, player) {
            Ok(recorder) => Some(recorder),
            Err(e) => {
                log::warn!("[lan] packet capture open for {player} ({conn}): {e}");
                None
            }
        }
    }

    pub(super) fn open_in(dir: &Path, conn: u32, player: &str) -> std::io::Result<Arc<Self>> {
        let path = dir.join(format!("{}-{conn}.pkt", sanitize_filename(player)));
        let file = File::create(path)?;
        let mut out = BufWriter::new(file);
        writeln!(out, "# v1\t{conn}\t{}", sanitize_header(player))?;
        out.flush()?;
        Ok(Arc::new(Self {
            sink: Mutex::new(Sink { out, seq: 0 }),
        }))
    }

    pub(super) fn record_one(&self, meta: PacketMeta) {
        self.record_slice(std::slice::from_ref(&meta));
    }

    pub(super) fn record_slice(&self, metas: &[PacketMeta]) {
        if metas.is_empty() {
            return;
        }
        let tick = current_tick();
        let mut sink = self.sink.lock().unwrap_or_else(PoisonError::into_inner);
        for meta in metas {
            let seq = sink.seq;
            sink.seq += 1;
            let _ = writeln!(
                sink.out,
                "{seq}\t{tick}\t{}\t{}\t{:032x}",
                meta.id, meta.len, meta.hash
            );
        }
        let _ = sink.out.flush();
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        let _ = self
            .sink
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .out
            .flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lan::{Holding, PlayerSender, conn::transaction_packets, flush_all};
    use std::sync::mpsc::channel;

    #[test]
    fn decode_packet_id_handles_single_multibyte_and_empty() {
        assert_eq!(decode_packet_id(&[46, 0, 1]), 46);
        assert_eq!(decode_packet_id(&[0x80, 0x01, 9]), 128);
        assert_eq!(decode_packet_id(&[]), -1);
    }

    #[test]
    fn prepared_compressed_body_records_uncompressed_metadata() {
        let mut raw = vec![24u8];
        raw.extend(std::iter::repeat_n(b'a', 128));
        let framing = Framing::Network(Some(32));
        let prepared = framing.contents(&raw).unwrap();
        assert_ne!(prepared, raw);
        assert_eq!(
            PacketMeta::from_prepared(&prepared, framing),
            PacketMeta::from_body(&raw)
        );
    }

    #[test]
    fn records_in_send_order_after_holding_and_transactions() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = Recorder::open_in(dir.path(), 7, "Alice").unwrap();
        let (tx, _rx) = channel();
        let sender = PlayerSender::new(tx, Arc::default()).with_capture(recorder);

        let first = vec![50u8, 1, 2];
        let second = vec![8u8, 3];
        let rolled_back = vec![9u8, 4];
        let committed = vec![10u8, 5, 6, 7];

        set_tick(10);
        sender.send(first.clone());

        let holding = Holding::start();
        sender.send(second.clone());
        let aborted = transaction_packets::Scope::begin();
        sender.send(rolled_back);
        drop(aborted);
        let kept = transaction_packets::Scope::begin();
        sender.send(committed.clone());
        kept.finish().unwrap().publish();
        drop(holding);

        // Held packets are recorded only once flushed to the connection.
        let path = dir.path().join("Alice-7.pkt");
        let before_flush = std::fs::read_to_string(&path).unwrap();
        assert_eq!(before_flush.lines().count(), 2);

        set_tick(11);
        flush_all([&sender]);

        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        assert_eq!(
            lines,
            vec![
                "# v1\t7\tAlice".to_owned(),
                format!("0\t10\t50\t3\t{:032x}", xxhash_rust::xxh3::xxh3_128(&first)),
                format!("1\t11\t8\t2\t{:032x}", xxhash_rust::xxh3::xxh3_128(&second)),
                format!(
                    "2\t11\t10\t4\t{:032x}",
                    xxhash_rust::xxh3::xxh3_128(&committed)
                ),
            ]
        );
    }
}
