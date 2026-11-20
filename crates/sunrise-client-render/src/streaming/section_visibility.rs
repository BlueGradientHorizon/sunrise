//! Admission and world-thread preparation for terrain visibility in the player's level.
use super::{Job, MeshSettings, Streaming, WorkerContext};
use crate::fast_hash::{FxHashMap, FxHashSet};
use crate::mesher::{
    ANSWER_BYTES, Lease, MAX_RESIDENT_VISIBILITY_BYTES, PLAN_BYTES, Region, SectionAnswer,
    SectionVisibilityPlan, plan_section,
};
use crate::section_mask::SectionMask;
use crate::world::World;
use mod_api::{BiomeId, CellVisibility, MAX_CELL_RECORDS, SECTION_INPUT_CELLS, SectionCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const SNAPSHOT_BYTES: usize = SECTION_INPUT_CELLS
    * (size_of::<SectionCell>() + size_of::<u8>() + size_of::<Option<BiomeId>>());
const WORKER_BYTES: usize = PLAN_BYTES + size_of::<Region>() + size_of::<FrozenPlan>();
const REQUEST_BYTES: usize = SNAPSHOT_BYTES
    + MAX_CELL_RECORDS * size_of::<CellVisibility>()
    + size_of::<VisibilityRequest>()
    + WORKER_BYTES
    + ANSWER_BYTES;
static RESIDENT_BYTES: AtomicUsize = AtomicUsize::new(0);

/// Section visibility state for the player's current level.
#[derive(Default)]
pub struct SectionPreparation {
    pub(super) listening: bool,
    /// Also validate results left over from when a listener was active, or just became active.
    pub(super) guard_uploads: bool,
    pending: Vec<VisibilityRequest>,
    answers: FxHashMap<(i32, i32, i32), HeldAnswer>,
    /// The mods and refusal codes already logged since the last refresh.
    pub refusals: FxHashSet<(&'static str, &'static str)>,
    logged_capacity: bool,
}

struct HeldAnswer {
    answer: SectionAnswer,
    _lease: Lease,
    resident: &'static AtomicUsize,
}

impl Drop for HeldAnswer {
    fn drop(&mut self) {
        self.resident.fetch_sub(ANSWER_BYTES, Ordering::AcqRel);
    }
}

/// Prepared alongside the hook input; keeps the same resources and budget until the worker ends.
pub struct FrozenPlan {
    pub(crate) plan: Option<SectionVisibilityPlan>,
    pub(super) context: Arc<WorkerContext>,
    _lease: Lease,
}

/// A section chosen to mesh, with admission already held for the input, answer and worker plan.
pub struct VisibilityRequest {
    /// The section: column x, section index, column z.
    pub key: (i32, i32, i32),
    /// The same immutable blocks, light and biomes that the worker will read.
    pub region: Arc<Region>,
    seq: u64,
    first: bool,
    settings: MeshSettings,
    context: Arc<WorkerContext>,
    lease: Lease,
}

impl SectionPreparation {
    /// Admit before copying the snapshot. A job that cannot fit stays dirty, not in a second
    /// queue holding an uncharged snapshot or answers.
    pub(super) fn admit() -> Option<Lease> {
        Lease::charge(REQUEST_BYTES)
    }

    fn replace_answer(
        &mut self,
        key: (i32, i32, i32),
        answer: Option<SectionAnswer>,
        lease: &mut Lease,
        resident: &'static AtomicUsize,
    ) -> bool {
        let before = self.answers.get(&key).map(|held| &held.answer);
        let border_changed = SectionAnswer::border_differs(before, answer.as_ref());
        if let Some(answer) = answer {
            if let Some(old) = self.answers.get_mut(&key) {
                // Reuse the old resident charge. The new answer was staged with the request's
                // charge and replacing it cannot fail, even when the resident pool is full.
                old.answer = answer;
            } else if resident
                .try_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                    (used + ANSWER_BYTES <= MAX_RESIDENT_VISIBILITY_BYTES)
                        .then_some(used + ANSWER_BYTES)
                })
                .is_ok()
            {
                self.answers.insert(
                    key,
                    HeldAnswer {
                        answer,
                        _lease: lease.split(ANSWER_BYTES),
                        resident,
                    },
                );
            } else {
                if !self.logged_capacity {
                    log::warn!(
                        "Sunrise: section_visibility: resident visibility budget exhausted; new masks are refused (E_SECTION_VISIBILITY_CAPACITY)"
                    );
                    self.logged_capacity = true;
                }
                return false;
            }
        } else {
            self.answers.remove(&key);
        }
        border_changed
    }
}

impl Streaming {
    pub(super) fn submit_mesh(
        &mut self,
        key: (i32, i32, i32),
        region: &Arc<Region>,
        first: bool,
        visibility: Option<Lease>,
    ) {
        let settings = self.mesh_settings();
        if let Some(lease) = visibility {
            let context = self.shared.context.read().unwrap().clone();
            self.section_visibility.pending.push(VisibilityRequest {
                key,
                region: region.clone(),
                seq: self.sequence,
                first,
                settings,
                context,
                lease,
            });
            return;
        }
        let (x, sy, z) = key;
        let job = Job::Mesh(x, sy, z, region.clone(), self.sequence, settings);
        self.submit(Some((x, z)), if first { 2 } else { 0 }, job);
    }

    /// The admitted sections whose raw inputs are ready to lend to mods on the world thread.
    /// Empty while no mod listens.
    pub fn take_visibility_requests(&mut self) -> Vec<VisibilityRequest> {
        std::mem::take(&mut self.section_visibility.pending)
    }

    /// Accept a completed decision only while its selection is still current. Freeze its own
    /// answer and the cached halo answers before handing the immutable input to a worker.
    pub fn submit_visibility_answer(
        &mut self,
        world: &mut World,
        request: VisibilityRequest,
        answer: Option<SectionAnswer>,
    ) {
        let VisibilityRequest {
            key,
            region,
            seq,
            first,
            mut settings,
            context,
            mut lease,
        } = request;
        let (x, sy, z) = key;
        if self.meshing.get(&key) != Some(&seq) || !world.chunks.contains_key(&(x, z)) {
            return;
        }
        if self
            .section_visibility
            .replace_answer(key, answer, &mut lease, &RESIDENT_BYTES)
        {
            dirty_neighbours(world, key);
        }
        let answers = &self.section_visibility.answers;
        let plan = plan_section(
            answers.get(&key).map(|held| &held.answer),
            |[dx, dy, dz]| {
                answers
                    .get(&(x + dx, sy + dy, z + dz))
                    .map(|held| &held.answer)
            },
        );
        settings.visibility = Some(Arc::new(FrozenPlan {
            plan,
            context,
            _lease: lease.split(WORKER_BYTES),
        }));
        let job = Job::Mesh(x, sy, z, region, seq, settings);
        self.submit(Some((x, z)), if first { 2 } else { 0 }, job);
    }

    /// A visibility policy change, or the join. Invalidate answers, jobs and queued uploads,
    /// including results accepted earlier in this frame. No work when there was no listener.
    pub fn refresh_section_visibility(&mut self, world: &mut World, listening: bool) {
        let state = &mut self.section_visibility;
        if !listening && !state.listening {
            return;
        }
        state.listening = listening;
        state.guard_uploads = true;
        self.reload_sections(world);
    }

    pub(super) fn invalidate_visibility(&mut self) {
        let state = &mut self.section_visibility;
        state.pending.clear();
        state.answers.clear();
        state.refusals.clear();
        state.logged_capacity = false;
    }

    /// Rebuild surviving halo readers even if the unloaded column never had a mod answer:
    /// the raw input changes from present to missing independently of its visibility mask.
    pub(super) fn forget_visibility(
        &mut self,
        world: &mut World,
        columns: impl IntoIterator<Item = (i32, i32)>,
    ) {
        if !self.section_visibility.listening {
            return;
        }
        for (x, z) in columns {
            for sy in 0..world.layout().sections() as i32 {
                self.section_visibility.answers.remove(&(x, sy, z));
            }
            Self::dirty_visibility_columns(world, x, z);
        }
    }

    /// A column arrived or disappeared; every resident reader's raw halo changed.
    pub(super) fn dirty_visibility_columns(world: &mut World, x: i32, z: i32) {
        let mask = SectionMask::below(world.layout().sections());
        for dz in -1..=1 {
            for dx in -1..=1 {
                let n = (x + dx, z + dz);
                if world.chunks.contains_key(&n) {
                    world.dirty_sections.insert_column_mask(n, mask);
                }
            }
        }
    }
}

/// The 26 loaded halo readers of a section whose border answer just changed.
fn dirty_neighbours(world: &mut World, (x, sy, z): (i32, i32, i32)) {
    let sections = world.layout().sections() as i32;
    for dx in -1..=1 {
        for dz in -1..=1 {
            if !world.chunks.contains_key(&(x + dx, z + dz)) {
                continue;
            }
            for dy in -1..=1 {
                let ny = sy + dy;
                if (dx, dy, dz) != (0, 0, 0) && (0..sections).contains(&ny) {
                    world.dirty_sections.insert((x + dx, ny, z + dz));
                }
            }
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::streaming::{ChunkGenerator, Done, MeshUpload, Proto};
    use crate::{
        assets::Assets, biome::Biomes, chunk::Chunk, light::LightTables, mesher::MeshTables, models,
    };
    use std::sync::OnceLock;
    use std::time::Duration;
    use sunrise_assets::resource_source::ResourceRoot;
    use sunrise_items::blocks::BlockFacts;
    use sunrise_world::registry::BlockStateId;

    pub(in crate::streaming) fn fixture() -> (World, Streaming, u32) {
        struct RemoteOnly;
        impl ChunkGenerator for RemoteOnly {
            type Worker = ();
            fn worker(&self) {}
            fn proto(&self, _: &mut (), _: i32, _: i32) -> Proto {
                panic!("a remote level never generates locally")
            }
            fn finish(&self, _: Proto) -> Chunk {
                panic!("a remote level never generates locally")
            }
        }
        static CONTEXT: OnceLock<Arc<WorkerContext>> = OnceLock::new();
        let context = CONTEXT.get_or_init(|| {
            let root =
                std::path::PathBuf::from(std::env::var_os("SUNRISE_MESH_TEST_RESOURCES").unwrap());
            let source = ResourceRoot::directory(root.clone());
            let assets = Arc::new(Assets::load(&root).unwrap());
            let render = Arc::new(assets.reload(&source).unwrap());
            let facts = BlockFacts::load(&root, assets.registry).unwrap();
            let visibility = models::load_visibility(&root, assets.registry).unwrap();
            Arc::new(WorkerContext {
                tables: Arc::new(
                    MeshTables::new(&assets, &render, &facts, visibility, None).unwrap(),
                ),
                light: Arc::new(LightTables::load(&root, assets.registry, &facts).unwrap()),
                biomes: Arc::new(Biomes::load(&source).unwrap()),
                assets,
                render,
                hidden_states: Arc::from([]),
                part_tables: Default::default(),
            })
        });
        let world = World::new(BlockStateId(0), 0);
        let mut streaming = Streaming::new(Arc::new(RemoteOnly), context.clone(), 1);
        streaming.remote = true;
        streaming.center_y = world.bottom_y();
        let stone = context
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap()
            .0;
        (world, streaming, stone)
    }

    const CENTRE: (i32, i32, i32) = (0, 0, 0);

    #[test]
    fn resident_capacity_does_not_stall_replacement_or_clear() {
        use crate::mesher::SectionPlanBuilder;
        static RESIDENT: AtomicUsize =
            AtomicUsize::new(MAX_RESIDENT_VISIBILITY_BYTES - ANSWER_BYTES);
        static SHARED: AtomicUsize = AtomicUsize::new(MAX_RESIDENT_VISIBILITY_BYTES - ANSWER_BYTES);
        let mut state = SectionPreparation::default();
        let mut lease = Lease::charge_in(&SHARED, REQUEST_BYTES).unwrap();
        let mut builder = SectionPlanBuilder::new();
        builder.hide_terrain();
        assert!(state.replace_answer(CENTRE, builder.finish(), &mut lease, &RESIDENT));
        drop(lease);
        assert_eq!(
            RESIDENT.load(Ordering::Acquire),
            MAX_RESIDENT_VISIBILITY_BYTES
        );
        let mut lease =
            Lease::charge_in(&SHARED, REQUEST_BYTES).expect("cache leaves room before capture");
        let mut builder = SectionPlanBuilder::new();
        builder.restrict([0, 0, 0], true, true, 61);
        assert!(state.replace_answer(CENTRE, builder.finish(), &mut lease, &RESIDENT));
        let stored = &state.answers[&CENTRE].answer;
        assert!(!stored.hides_terrain());
        assert_eq!(
            RESIDENT.load(Ordering::Acquire),
            MAX_RESIDENT_VISIBILITY_BYTES
        );
        assert!(state.replace_answer(CENTRE, None, &mut lease, &RESIDENT));
        assert!(!state.answers.contains_key(&CENTRE));
        drop(lease);
        assert_eq!(
            RESIDENT.load(Ordering::Acquire),
            MAX_RESIDENT_VISIBILITY_BYTES - ANSWER_BYTES
        );
        assert_eq!(
            SHARED.load(Ordering::Acquire),
            MAX_RESIDENT_VISIBILITY_BYTES - ANSWER_BYTES
        );
    }

    #[test]
    #[ignore = "daily"]
    fn remote_column_arrival_and_unload_remesh_the_actual_halo() {
        use crate::mesher::{SectionPlanBuilder, capture_input};
        use mod_api::{BlockStateId, SectionCell};

        let (mut world, mut streaming, stone) = fixture();
        let base = world.bottom_y();
        let mut centre = Chunk::empty(0, 0);
        centre.set(15, base + 4, 8, stone);
        streaming.insert_remote(&mut world, centre);
        assert!(streaming.update(&mut world).0.is_empty());
        assert!(streaming.take_visibility_requests().is_empty());
        assert!(
            world.dirty_sections.contains(&CENTRE),
            "the no-listener route still waits for a complete region"
        );
        streaming.refresh_section_visibility(&mut world, true);

        let compile = |world: &mut World, streaming: &mut Streaming, expected: SectionCell| {
            let mut captured = false;
            for _ in 0..100 {
                let (uploads, _) = streaming.update(world);
                for request in streaming.take_visibility_requests() {
                    let mut builder = SectionPlanBuilder::new();
                    if request.key == CENTRE {
                        let input = capture_input(&request.region, 0);
                        assert_eq!(input.cell([16, 4, 8]), Ok(expected));
                        captured = true;
                    } else if request.key == (1, 0, 0) {
                        builder.restrict([1, 4, 8], false, false, 0);
                    }
                    streaming.submit_visibility_answer(world, request, builder.finish());
                }
                let faces = uploads
                    .iter()
                    .find(|upload| {
                        upload.key == CENTRE && streaming.can_publish_mesh(world, upload)
                    })
                    .map(|upload| upload.mesh.solid.len() / 4);
                streaming.free(uploads);
                if let Some(faces) = faces {
                    assert!(captured);
                    return faces;
                }
                streaming.wait(Duration::from_millis(20));
            }
            panic!("the requested centre mesh did not complete");
        };
        assert_eq!(compile(&mut world, &mut streaming, SectionCell::Missing), 6);
        let mut east = Chunk::empty(1, 0);
        east.set(0, base + 4, 8, stone);
        streaming.insert_remote(&mut world, east);
        assert_eq!(
            compile(
                &mut world,
                &mut streaming,
                SectionCell::State(BlockStateId(stone))
            ),
            5
        );
        assert!(
            streaming
                .section_visibility
                .answers
                .contains_key(&(1, 0, 0))
        );
        let gone = [(1, 0)].into_iter().collect();
        assert_eq!(Streaming::take_remote_columns(&mut world, &gone).len(), 1);
        streaming.forget_columns(&mut world, &gone);
        assert!(
            !streaming
                .section_visibility
                .answers
                .contains_key(&(1, 0, 0)),
            "unload drops the answer and its lease"
        );
        assert_eq!(compile(&mut world, &mut streaming, SectionCell::Missing), 6);
    }

    #[test]
    #[ignore = "daily"]
    fn an_accepted_mesh_cannot_publish_after_policy_resource_or_column_changes() {
        use crate::mesher::SectionPlanBuilder;

        let (mut world, mut streaming, stone) = fixture();
        let base = world.bottom_y();
        let column = || {
            let mut chunk = Chunk::empty(0, 0);
            chunk.set(8, base + 4, 8, stone);
            chunk
        };
        let request = |world: &mut World, streaming: &mut Streaming| {
            streaming.update(world);
            streaming
                .take_visibility_requests()
                .into_iter()
                .find(|request| request.key == CENTRE)
                .unwrap()
        };
        // Drive the mailbox in a fixed order rather than depending on which worker finishes first.
        let completed = |streaming: &Streaming, seq| {
            streaming
                .shared
                .done
                .send(Done::Meshed(0, 0, 0, Default::default(), seq));
        };
        let upload = |world: &mut World, streaming: &mut Streaming| -> MeshUpload {
            let uploads = streaming.update(world).0;
            let mut centre = uploads.into_iter().filter(|upload| upload.key == CENTRE);
            let upload = centre.next().expect("one accepted centre mesh");
            assert!(
                centre.next().is_none(),
                "a second, stale compile was accepted"
            );
            upload
        };
        streaming.insert_remote(&mut world, column());
        streaming.refresh_section_visibility(&mut world, true);
        let first = request(&mut world, &mut streaming);
        completed(&streaming, first.seq);
        let before_panic = upload(&mut world, &mut streaming);
        assert!(streaming.can_publish_mesh(&world, &before_panic));
        world.dirty_sections.insert(CENTRE);
        assert!(
            !streaming.can_publish_mesh(&world, &before_panic),
            "a changed dependency invalidates an accepted upload before its next compile"
        );
        world.dirty_sections.take_column(0, 0);
        // This is precisely the invalidation used when a listener panics during the same update.
        streaming.refresh_section_visibility(&mut world, true);
        assert!(!streaming.can_publish_mesh(&world, &before_panic));
        let mut builder = SectionPlanBuilder::new();
        builder.hide_terrain();
        let resource_a = request(&mut world, &mut streaming);
        let seq_a = resource_a.seq;
        // Install a cached answer under A without sending live work into the synthetic mailbox.
        let mut resource_a = resource_a;
        streaming.section_visibility.replace_answer(
            CENTRE,
            builder.finish(),
            &mut resource_a.lease,
            &RESIDENT_BYTES,
        );
        let a = &resource_a.context;
        let context_b = Arc::new(WorkerContext {
            assets: a.assets.clone(),
            render: a.render.clone(),
            tables: a.tables.clone(),
            biomes: a.biomes.clone(),
            light: a.light.clone(),
            hidden_states: vec![true; a.assets.registry.len()].into(),
            part_tables: a.part_tables.clone(),
        });
        streaming.reload_resources(&mut world, context_b.clone());
        assert!(streaming.section_visibility.answers.is_empty());
        let resource_b = request(&mut world, &mut streaming);
        assert!(Arc::ptr_eq(&resource_b.context, &context_b));
        assert!(resource_a.context.hidden_states.is_empty());
        completed(&streaming, resource_b.seq);
        completed(&streaming, seq_a);
        let before_unload = upload(&mut world, &mut streaming);
        assert_eq!(before_unload.seq, resource_b.seq);
        assert!(streaming.can_publish_mesh(&world, &before_unload));
        let gone = [(0, 0)].into_iter().collect();
        Streaming::take_remote_columns(&mut world, &gone);
        streaming.forget_columns(&mut world, &gone);
        streaming.insert_remote(&mut world, column());
        let reloaded = request(&mut world, &mut streaming);
        completed(&streaming, reloaded.seq);
        let current = upload(&mut world, &mut streaming);
        assert!(streaming.can_publish_mesh(&world, &current));
        assert!(!streaming.can_publish_mesh(&world, &before_unload));
    }
}
