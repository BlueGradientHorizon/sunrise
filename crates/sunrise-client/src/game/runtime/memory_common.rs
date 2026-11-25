//! Shared memory census report types, formatting, and common `Game` entry points.
#[allow(unused_imports)]
use super::Game;
#[allow(unused_imports)]
use serde::Serialize;
#[allow(unused_imports)]
use serde_json::Value;
#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, Sender, channel, sync_channel},
};
#[allow(unused_imports)]
use std::task::Poll;
#[allow(unused_imports)]
use std::thread::{Builder, JoinHandle};
#[allow(unused_imports)]
use sunrise_client::chunk::{Chunk, ColumnCensus, Section};
#[allow(unused_imports)]
use sunrise_client::fast_hash::FxHashSet;
#[allow(unused_imports)]
use sunrise_client::server_perf::MemoryCost;
#[allow(unused_imports)]
use sunrise_client_render::gfx::RendererMemorySnapshot;

pub(in crate::game) const MB: f64 = 1048576.0;
/// Entity types and players listed apart; the rest only count in the totals.
pub(in crate::game) const TOP: usize = 30;
pub(in crate::game) const TOP_PLAYERS: usize = 50;

pub(in crate::game) fn mb(bytes: impl TryInto<u64>) -> f64 {
    let bytes: u64 = bytes.try_into().unwrap_or(0);
    (bytes as f64 / MB * 10.0).round() / 10.0
}

pub(in crate::game) fn mb_signed(bytes: i64) -> f64 {
    (bytes as f64 / MB * 10.0).round() / 10.0
}

/// Counts each section once however many columns (or worlds) share it.
pub(in crate::game) fn census(chunks: &[Arc<Chunk>]) -> (ColumnCensus, FxHashSet<*const Section>) {
    let mut census = ColumnCensus::default();
    let mut seen = FxHashSet::default();
    for chunk in chunks {
        chunk.census(&mut census, &mut seen);
    }
    (census, seen)
}

#[derive(Serialize)]
pub(in crate::game) struct Columns {
    pub(in crate::game) total_mb: f64,
    pub(in crate::game) columns: usize,
    pub(in crate::game) sections: usize,
    pub(in crate::game) unique_sections: usize,
    pub(in crate::game) uniform_block_sections: usize,
    pub(in crate::game) light_arrays: usize,
    pub(in crate::game) blocks_mb: f64,
    pub(in crate::game) biomes_mb: f64,
    pub(in crate::game) light_mb: f64,
    pub(in crate::game) heightmaps_mb: f64,
    pub(in crate::game) section_records_mb: f64,
    pub(in crate::game) column_records_mb: f64,
    #[serde(skip)]
    pub(in crate::game) bytes: usize,
}

impl Columns {
    pub(in crate::game) fn new(c: &ColumnCensus) -> Self {
        Self {
            total_mb: mb(c.total_bytes()),
            columns: c.columns,
            sections: c.sections,
            unique_sections: c.unique_sections,
            uniform_block_sections: c.uniform_block_sections,
            light_arrays: c.light_arrays,
            blocks_mb: mb(c.block_bytes),
            biomes_mb: mb(c.biome_bytes),
            light_mb: mb(c.light_bytes),
            heightmaps_mb: mb(c.heightmap_bytes),
            section_records_mb: mb(c.section_record_bytes),
            column_records_mb: mb(c.column_bytes),
            bytes: c.total_bytes(),
        }
    }
}

/// Sections shared with `others` by address, and the heap bytes of one copy.
pub(in crate::game) fn shared<'a>(
    entries: impl IntoIterator<Item = &'a Arc<Section>>,
    mut others: FxHashSet<*const Section>,
) -> SharedSections {
    let (mut sections, mut bytes) = (0, 0);
    for section in entries {
        if others.remove(&Arc::as_ptr(section)) {
            sections += 1;
            bytes += section.heap_bytes();
        }
    }
    SharedSections {
        sections,
        mb: mb(bytes),
        bytes,
    }
}

#[derive(Serialize)]
pub(in crate::game) struct SharedSections {
    pub(in crate::game) sections: usize,
    pub(in crate::game) mb: f64,
    #[serde(skip)]
    pub(in crate::game) bytes: usize,
}

#[derive(Serialize)]
pub(in crate::game) struct Generation {
    pub(in crate::game) protos: usize,
    pub(in crate::game) protos_mb: f64,
    pub(in crate::game) proto_blocks_mb: f64,
    pub(in crate::game) proto_section_lists_mb: f64,
    pub(in crate::game) proto_heightmaps_mb: f64,
    pub(in crate::game) proto_other_terrain_mb: f64,
    pub(in crate::game) proto_biomes_mb: f64,
    pub(in crate::game) proto_records_mb: f64,
    pub(in crate::game) proto_sections_by_bits: [usize; 16],
    pub(in crate::game) light_views: usize,
    pub(in crate::game) light_views_mb: f64,
    /// Name, entries and megabytes where the cache can say.
    pub(in crate::game) caches: Vec<(&'static str, usize, Option<f64>)>,
    pub(in crate::game) stages: Vec<(&'static str, usize)>,
    #[serde(skip)]
    pub(in crate::game) bytes: usize,
}

/// Entities of every level by registry id: how many, and the bytes of their records.
#[derive(Serialize)]
pub(in crate::game) struct Entities {
    pub(in crate::game) count: usize,
    pub(in crate::game) total_mb: f64,
    /// The `TOP` types holding the most bytes: id, count, megabytes.
    pub(in crate::game) by_type: Vec<(String, usize, f64)>,
    #[serde(skip)]
    pub(in crate::game) bytes: usize,
}

/// The connected players: queues and what each is owed or holds, the biggest first.
#[derive(Default, Serialize)]
pub(in crate::game) struct Players {
    pub(in crate::game) count: usize,
    pub(in crate::game) total_mb: f64,
    /// Name, packets and megabytes waiting to be sent.
    pub(in crate::game) send_queue: Vec<(String, usize, f64)>,
    pub(in crate::game) chunks_waiting: Vec<(String, usize)>,
    pub(in crate::game) chunks_sent: Vec<(String, usize)>,
    pub(in crate::game) tracked_entities: Vec<(String, usize)>,
    #[serde(skip)]
    pub(in crate::game) bytes: usize,
}

#[derive(Default, Serialize)]
pub(in crate::game) struct Cached {
    pub(in crate::game) entries: usize,
    pub(in crate::game) size_mb: f64,
}

/// What the hub keeps to send chunks: encoded sections, heightmaps and compressed frames.
#[derive(Default, Serialize)]
pub(in crate::game) struct ChunkSend {
    pub(in crate::game) total_mb: f64,
    pub(in crate::game) sections: Cached,
    pub(in crate::game) heightmaps: Cached,
    pub(in crate::game) frames: Cached,
    #[serde(skip)]
    pub(in crate::game) bytes: usize,
}

#[derive(Serialize)]
pub(in crate::game) struct Queue {
    pub(in crate::game) len: usize,
    pub(in crate::game) size_mb: f64,
}

#[derive(Serialize)]
pub(in crate::game) struct Process {
    pub(in crate::game) resident_mb: Option<f64>,
    pub(in crate::game) allocator_commit_mb: f64,
    pub(in crate::game) allocator_peak_commit_mb: f64,
}

#[derive(Serialize)]
pub(in crate::game) struct Allocator {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::game) live_heap_mb: Option<f64>,
    pub(in crate::game) commit_mb: f64,
    pub(in crate::game) peak_commit_mb: f64,
    /// Committed minus the pages in use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::game) overhead_mb: Option<f64>,
}

#[derive(Serialize)]
pub(in crate::game) struct FarTiles {
    pub(in crate::game) active: usize,
    pub(in crate::game) cached: usize,
    pub(in crate::game) kept: usize,
    pub(in crate::game) kept_mb: f64,
}

#[derive(Serialize)]
pub(in crate::game) struct Gpu {
    pub(in crate::game) allocated_mb: f64,
    pub(in crate::game) reserved_mb: f64,
    pub(in crate::game) by_label_mb: Vec<(String, f64)>,
}

#[derive(Serialize)]
pub(in crate::game) struct Renderer {
    pub(in crate::game) section_arenas_mb: f64,
    pub(in crate::game) section_vertices_mb: f64,
    pub(in crate::game) cpu: BTreeMap<String, f64>,
    pub(in crate::game) gpu: Option<Gpu>,
}

/// A serving game's own parts: columns, generation, entities, players, chunk send caches, queues.
pub(in crate::game) struct ServerSide {
    pub(in crate::game) chunks: Vec<Arc<Chunk>>,
    pub(in crate::game) generation: Generation,
    pub(in crate::game) entities: Entities,
    pub(in crate::game) players: Players,
    pub(in crate::game) chunk_send: ChunkSend,
    pub(in crate::game) light_queue: Queue,
    pub(in crate::game) generation_queue: Queue,
    pub(in crate::game) queue_bytes: usize,
}

impl ServerSide {
    /// The bytes of every part this side names.
    pub(in crate::game) fn bytes(&self) -> usize {
        self.generation.bytes
            + self.entities.bytes
            + self.players.bytes
            + self.chunk_send.bytes
            + self.queue_bytes
    }
}

/// How often a running capture records what holds the memory.
pub(in crate::game) const MEMORY_EVERY_S: f64 = 10.0;

/// `sunrise-ctl memory`'s report; `*_mb` values round to tenths while bytes sum unrounded.
#[derive(Serialize)]
pub(in crate::game) struct MemoryReport {
    pub(in crate::game) process: Process,
    pub(in crate::game) allocator: Allocator,
    pub(in crate::game) render_distance: i32,
    /// The window's columns; a dedicated server has no window and names them `columns`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::game) client_columns: Option<Columns>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::game) columns: Option<Columns>,
    pub(in crate::game) server_columns: Option<Columns>,
    pub(in crate::game) shared_with_server: Option<SharedSections>,
    pub(in crate::game) server_generation: Option<Generation>,
    pub(in crate::game) entities: Option<Entities>,
    pub(in crate::game) players: Option<Players>,
    pub(in crate::game) chunk_send: Option<ChunkSend>,
    pub(in crate::game) light_queue: Option<Queue>,
    pub(in crate::game) generation_queue: Option<Queue>,
    pub(in crate::game) far_tiles: Option<FarTiles>,
    pub(in crate::game) renderer: Option<Renderer>,
    /// Live heap by startup step, span, pool job, or thread when `alloc-census` is enabled.
    pub(in crate::game) heap_by_tag_mb: Option<Vec<(String, f64)>>,
    /// Columns, generation, entities, players, chunk send caches, queues, renderer CPU, far tiles.
    pub(in crate::game) named_mb: f64,
    /// Resident memory minus `named_mb` (may be negative where parts overlap).
    pub(in crate::game) unattributed_mb: Option<f64>,
}

/// Values owned by the game or renderer while copy-on-write handles keep section identities alive.
pub(in crate::game) struct MemoryInputs {
    pub(in crate::game) render_distance: i32,
    pub(in crate::game) client: Option<Vec<Arc<Chunk>>>,
    pub(in crate::game) renderer: Option<RendererMemorySnapshot>,
    pub(in crate::game) far: Option<(FarTiles, usize)>,
}

impl MemoryInputs {
    pub(in crate::game) fn report(
        self,
        server: Option<ServerSide>,
        active: impl Fn() -> bool,
    ) -> Option<MemoryReport> {
        let (commit, peak_commit) = sunrise_threads::allocator_commit();
        let live = sunrise_threads::allocator_stats::live_bytes();
        let resident = sunrise_threads::process_memory::resident_bytes();
        if !active() {
            return None;
        }
        let renderer = self.renderer.map(|r| {
            let [arenas, vertices] = r.section_vertex_bytes();
            let cpu = r.cpu_census();
            let cpu_bytes: usize = cpu.iter().map(|(_, v)| *v).sum();
            let gpu = r
                .gpu_memory_by_label()
                .map(|(allocated, reserved, labels)| Gpu {
                    allocated_mb: mb(allocated),
                    reserved_mb: mb(reserved),
                    by_label_mb: labels
                        .into_iter()
                        .take(40)
                        .map(|(label, bytes)| (label, mb(bytes)))
                        .collect(),
                });
            (
                Renderer {
                    section_arenas_mb: mb(arenas),
                    section_vertices_mb: mb(vertices),
                    cpu: cpu
                        .into_iter()
                        .map(|(k, v)| (format!("{k}_mb"), mb(v)))
                        .collect(),
                    gpu,
                },
                cpu_bytes,
            )
        });
        if !active() {
            return None;
        }
        let window = self
            .client
            .as_ref()
            .map(|chunks| Columns::new(&census(chunks).0));
        if !active() {
            return None;
        }
        let (server_columns, shared_with_server) = server.as_ref().map_or((None, None), |s| {
            let (columns, sections) = census(&s.chunks);
            let shared = self
                .client
                .as_ref()
                .map(|chunks| shared(chunks.iter().flat_map(|c| &c.sections), sections));
            (Some(Columns::new(&columns)), shared)
        });
        if !active() {
            return None;
        }
        let named = window.as_ref().map_or(0, |c| c.bytes)
            + server_columns.as_ref().map_or(0, |c| c.bytes)
            + server.as_ref().map_or(0, ServerSide::bytes)
            + renderer.as_ref().map_or(0, |r| r.1)
            + self.far.as_ref().map_or(0, |f| f.1);
        let named = named.saturating_sub(shared_with_server.as_ref().map_or(0, |s| s.bytes));
        let (columns, server_columns) = if window.is_some() {
            (None, server_columns)
        } else {
            (server_columns, None)
        };
        let mut report = MemoryReport {
            process: Process {
                resident_mb: resident.map(mb),
                allocator_commit_mb: mb(commit),
                allocator_peak_commit_mb: mb(peak_commit),
            },
            allocator: Allocator {
                live_heap_mb: live.map(mb),
                commit_mb: mb(commit),
                peak_commit_mb: mb(peak_commit),
                overhead_mb: live.map(|live| mb(commit.saturating_sub(live))),
            },
            render_distance: self.render_distance,
            client_columns: window,
            columns,
            server_columns,
            shared_with_server,
            server_generation: None,
            entities: None,
            players: None,
            chunk_send: None,
            light_queue: None,
            generation_queue: None,
            far_tiles: self.far.map(|f| f.0),
            renderer: renderer.map(|r| r.0),
            heap_by_tag_mb: sunrise_threads::HEAP_CENSUS.then(|| {
                sunrise_threads::heap_census::census()
                    .into_iter()
                    .take(60)
                    .map(|(tag, bytes)| (tag.to_string(), mb(bytes.max(0) as u64)))
                    .collect()
            }),
            named_mb: mb(named),
            unattributed_mb: unattributed(resident, named),
        };
        if let Some(s) = server {
            report.server_generation = Some(s.generation);
            report.entities = Some(s.entities);
            report.players = Some(s.players);
            report.chunk_send = Some(s.chunk_send);
            report.light_queue = Some(s.light_queue);
            report.generation_queue = Some(s.generation_queue);
        }
        Some(report)
    }
}

/// `resident - named` in megabytes, None without a resident size.
pub(in crate::game) fn unattributed(resident: Option<u64>, named: usize) -> Option<f64> {
    resident.map(|r| mb_signed(r as i64 - named as i64))
}

/// `sunrise-ctl memory`'s report as capture rows: `a.b` paths and `key[name]` list entries.
pub(in crate::game) fn flatten_report(report: &Value) -> Vec<(String, f64)> {
    let mut rows = Vec::new();
    flatten(String::new(), report, &mut rows);
    rows
}

pub(in crate::game) fn flatten(path: String, value: &Value, rows: &mut Vec<(String, f64)>) {
    match value {
        Value::Number(n) => rows.extend(n.as_f64().map(|n| (path, n))),
        Value::Object(fields) => {
            for (key, inner) in fields {
                let next = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                flatten(next, inner, rows);
            }
        }
        Value::Array(items) => {
            for item in items {
                let Some([Value::String(name), count, rest @ ..]) =
                    item.as_array().map(Vec::as_slice)
                else {
                    continue;
                };
                rows.extend(count.as_f64().map(|n| (format!("{path}[{name}]"), n)));
                if let Some(mb) = rest.first().and_then(Value::as_f64) {
                    rows.push((format!("{path}_mb[{name}]"), mb));
                }
            }
        }
        _ => {}
    }
}

pub(in crate::game) type MemoryRows = Vec<(String, f64)>;

/// Integrated owner answers directly to the worker and skips collection once cancelled.
pub(in crate::game) struct ServerMemoryRequest {
    pub(in crate::game) reply: Option<Sender<Option<ServerSide>>>,
    pub(in crate::game) cancelled: Arc<AtomicBool>,
    pub(in crate::game) cost: MemoryCost,
}

pub(in crate::game) struct MemoryCancellation {
    pub(in crate::game) cancelled: Arc<AtomicBool>,
    pub(in crate::game) wake_worker: Option<Sender<Option<ServerSide>>>,
}

impl Drop for MemoryCancellation {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// One census slot across captures; stop cancels the sample and waits for the worker to exit.
pub(in crate::game) struct PendingMemory {
    pub(in crate::game) cancel: MemoryCancellation,
    pub(in crate::game) worker: JoinHandle<()>,
    pub(in crate::game) rows: Receiver<MemoryRows>,
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    pub(in crate::game) fn memory_report(&self) -> MemoryReport {
        self.side_memory_report()
    }

    /// The report as JSON, for a manual control-socket request or a dedicated capture.
    pub(in crate::game) fn memory_value(&self) -> Value {
        serde_json::to_value(self.memory_report()).unwrap_or(Value::Null)
    }
}
