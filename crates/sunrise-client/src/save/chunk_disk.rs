//! Server-owned chunk-disk callbacks and the captured envelopes handed to region writers.
use super::*;
use mod_api::{ChunkDiskContext, ChunkDiskJob, ChunkDiskKind, ChunkDiskPhase, WorldEpoch};
use sunrise_client_render::api_context::WorldgenReader;
use sunrise_mod_runtime::attachments::Attachments;

pub(super) struct DiskRuntime {
    world: WorldEpoch,
    dimension: mod_api::Identifier,
    resources: Arc<sunrise_world::native_load_context::WorldgenContext>,
    load: bool,
    save: bool,
}

impl DiskRuntime {
    fn capture(
        &self,
        chunk: &Chunk,
        kind: ChunkDiskKind,
        phase: ChunkDiskPhase,
    ) -> Option<ChunkDiskJob> {
        let context = ChunkDiskContext::new(
            self.world,
            self.dimension.clone(),
            [chunk.x, chunk.z],
            kind,
            phase,
        );
        let empty = Attachments::default();
        ChunkDiskJob::capture(
            &context,
            chunk
                .saved
                .as_ref()
                .map_or(&empty, |saved| &saved.attachments),
        )
    }

    fn run(&self, job: &ChunkDiskJob) -> Compound {
        let reader = WorldgenReader(&self.resources);
        // A disk job has no block-state index (that is the game's, published for worldgen), so
        // `state_with_property` answers None, as worldgen does before the index is published.
        job.run(
            &mod_api::OwningWorldgen::new(&reader, &|_, _, _| None),
            sunrise_world::datafix::upgrade_compound,
        )
        .clone()
    }
}

pub(super) struct DiskRecord {
    pub(super) original: Record,
    entries: Compound,
    encoded: OnceLock<std::result::Result<Option<Arc<NamedTag>>, String>>,
}
impl DiskRecord {
    pub(super) fn cancel(&self) {
        let _ = self
            .encoded
            .set(Err("chunk save cancelled before encoding".into()));
    }

    fn encode(&self) -> std::io::Result<Option<Arc<NamedTag>>> {
        self.encoded
            .get_or_init(|| {
                // Cache the encoded record so pending readers share the writer's outcome.
                super::compression_pool::catch_encoding_panic(|| {
                    let mut tag = self.original.tag_for_write().ok_or_else(|| {
                        std::io::Error::other("captured chunk has no disk record")
                    })?;
                    replace_entries(Arc::make_mut(&mut tag), self.entries.clone())?;
                    Ok(Some(tag))
                })
                .map_err(|e| e.to_string())
            })
            .clone()
            .map_err(std::io::Error::other)
    }

    pub(super) fn without_block_entity_edits(&self) -> Option<Arc<NamedTag>> {
        let mut tag = self.original.without_block_entity_edits()?;
        replace_entries(Arc::make_mut(&mut tag), self.entries.clone()).ok()?;
        Some(tag)
    }

    // Readers may wait for the owning writer, but never execute save callbacks.
    pub(super) fn read(&self) -> Option<Arc<NamedTag>> {
        match self.encoded.wait() {
            Ok(tag) => tag.clone(),
            // A reader that borrowed this snapshot before completion still owns that read.
            Err(_) => self.original.tag(),
        }
    }
}

pub(super) fn encode_record(record: &Record) -> std::io::Result<Option<Arc<NamedTag>>> {
    match record.disk_job() {
        Some(record) => record.encode(),
        None => Ok(record.tag_for_write()),
    }
}

fn replace_entries(tag: &mut NamedTag, entries: Compound) -> std::io::Result<()> {
    let Tag::Compound(root) = &mut tag.tag else {
        return Err(std::io::Error::other("invalid chunk root"));
    };
    if entries.is_empty() {
        root.remove(&ATTACHMENTS_KEY.into());
    } else {
        root.insert(ATTACHMENTS_KEY.into(), Tag::Compound(entries));
    }
    Ok(())
}

impl SaveDir {
    /// Bind disk callbacks before this server level starts any chunk workers.
    /// The world epoch belongs to the existing lifecycle owner. Remote caches
    /// never call this method. With no subscribers no context is retained.
    pub fn set_chunk_disk_context(
        &self,
        world: WorldEpoch,
        dimension: mod_api::Identifier,
        resources: Arc<sunrise_world::native_load_context::WorldgenContext>,
    ) {
        let load = mod_api::__private::listened(mod_api::HookId::ChunkDataLoad);
        let save = mod_api::__private::listened(mod_api::HookId::ChunkDataSave);
        if !load && !save {
            return;
        }
        let _ = self.disk.set(Arc::new(DiskRuntime {
            world,
            dimension,
            resources,
            load,
            save,
        }));
    }

    pub(super) fn capture_disk_record(&self, record: Record) -> Record {
        let (chunk, kind) = match &record {
            Record::Chunk(chunk, ..) => (&**chunk, ChunkDiskKind::Full),
            Record::Proto(proto, ..) => (&proto.column, ChunkDiskKind::Proto),
            _ => return record,
        };
        if !mod_api::__private::listened(mod_api::HookId::ChunkDataSave) {
            return record;
        }
        let Some(runtime) = self.disk.get() else {
            return record;
        };
        let Some(job) = runtime.capture(chunk, kind, ChunkDiskPhase::Save) else {
            return record;
        };
        let entries = runtime.run(&job);
        Record::Disk(Arc::new(DiskRecord {
            original: record,
            entries,
            encoded: OnceLock::new(),
        }))
    }

    pub(super) fn has_chunk_disk_hook(&self, phase: ChunkDiskPhase) -> bool {
        self.disk.get().is_some_and(|runtime| match phase {
            ChunkDiskPhase::Load => runtime.load,
            ChunkDiskPhase::Save => runtime.save,
            _ => false,
        })
    }

    /// Dispatch on the owner after disk decode and before publishing or restoring the chunk.
    pub(super) fn load_disk_attachments(&self, chunk: &mut Chunk, kind: ChunkDiskKind) {
        let Some(runtime) = self.disk.get() else {
            // Non-authoritative remote caches have no disk context.
            return;
        };
        if let Some(job) = runtime.capture(chunk, kind, ChunkDiskPhase::Load) {
            Arc::make_mut(chunk.saved.get_or_insert_with(Default::default)).attachments =
                Attachments::from_nbt(runtime.run(&job));
        }
    }

    pub(super) fn save_disk_sync(
        &self,
        chunks: &[Arc<Chunk>],
        registry: &BlockRegistry,
        biomes: &Biomes,
        items: &crate::items::Items,
    ) -> Option<Result<usize>> {
        if !mod_api::__private::listened(mod_api::HookId::ChunkDataSave) {
            return None;
        }
        let runtime = self.disk.get()?;
        Some((|| {
            let edits: Vec<_> = chunks
                .iter()
                .map(|chunk| {
                    runtime
                        .capture(chunk, ChunkDiskKind::Full, ChunkDiskPhase::Save)
                        .map(|job| runtime.run(&job))
                })
                .collect();
            let work = || {
                chunks
                    .iter()
                    .zip(edits)
                    .filter_map(|(chunk, entries)| {
                        let key = (chunk.x, chunk.z);
                        let tag = compression_pool::catch_encoding_panic(|| {
                            let blending = self.calculated_blending(chunk.x, chunk.z);
                            let mut tag = encode_saved(
                                chunk,
                                registry,
                                biomes,
                                self.tables.get().map(|t| &**t),
                                self.shape,
                                blending.as_deref(),
                                items,
                            );
                            if let Some(entries) = entries {
                                replace_entries(&mut tag, entries)?;
                            }
                            Ok(tag)
                        });
                        match tag {
                            Ok(tag) => Some((key, Some(tag))),
                            Err(error) => {
                                self.report_record_failure(Folder::Region, key, &error);
                                None
                            }
                        }
                    })
                    .collect()
            };
            let records = std::thread::scope(|scope| scope.spawn(work).join())
                .map_err(|_| "chunk disk worker unwound")?;
            self.write_records("region", records, true)?;
            self.save_poi(chunks, true)?;
            Ok(chunks.len())
        })())
    }
}
