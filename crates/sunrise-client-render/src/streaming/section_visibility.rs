//! The world-thread step between choosing a section to mesh and queuing it, for
//! `section_visibility` (docs/design/mod-api/mesh-visibility.md): while a mod listens, a chosen
//! section waits here, the game asks the mods on its own thread
//! ([`Streaming::take_visibility_requests`]) and hands back their combined answer
//! ([`Streaming::submit_visibility_answers`]), which is checked against the section's current
//! mesh sequence and frozen into the job. With no mod listening a chosen section is queued at
//! once, exactly as before.
use super::{Job, Streaming};
use crate::fast_hash::{FxHashMap, FxHashSet};
use crate::mesher::{
    ANSWER_BYTES, Lease, PLAN_BYTES, Region, SectionAnswer, SectionVisibilityPlan, plan_section,
};
use crate::world::World;
use std::sync::Arc;

/// The sections waiting for the mods' visibility answers, and every section's last combined
/// answer, which its neighbours' plans read for their one-block border.
#[derive(Default)]
pub struct SectionPreparation {
    /// Whether a mod listens to `section_visibility`, set by
    /// [`Streaming::refresh_section_visibility`]; false is the no-mod path.
    listening: bool,
    pending: Vec<VisibilityRequest>,
    /// Sections whose answer is given but whose storage the shared in-flight bytes had no room
    /// for yet: admitted, in order, as bytes come free.
    waiting: Vec<(VisibilityRequest, Option<SectionAnswer>)>,
    answers: FxHashMap<(i32, i32, i32), HeldAnswer>,
    /// The mods and refusal codes logged since the last refresh: each is logged once.
    logged: FxHashSet<(&'static str, &'static str)>,
}

/// A section's cached answer and the in-flight bytes it holds until it is replaced or forgotten.
struct HeldAnswer {
    answer: SectionAnswer,
    _lease: Lease,
}

/// A frozen plan and the in-flight bytes it holds until the job carrying it ends.
pub struct FrozenPlan {
    pub(crate) plan: SectionVisibilityPlan,
    _lease: Lease,
}

impl SectionPreparation {
    /// Whether answers wait for room in the shared in-flight bytes.
    pub fn has_waiting(&self) -> bool {
        !self.waiting.is_empty()
    }
    /// Whether `mod_id`'s refusal `code` is new since the last refresh, so it is logged once.
    pub fn first_refusal(&mut self, mod_id: &'static str, code: &'static str) -> bool {
        self.logged.insert((mod_id, code))
    }
}

/// One section chosen to mesh, waiting for the mods' answers.
pub struct VisibilityRequest {
    /// The section: column x, section index, column z.
    pub key: (i32, i32, i32),
    /// The column and its eight neighbours as the job will mesh them: the input is captured from
    /// these, so the answers and the mesh read the same blocks.
    pub region: Arc<Region>,
    seq: u64,
    first: bool,
}

impl Streaming {
    /// Queues section `(x, sy, z)` to mesh under the current sequence, or, while a mod listens to
    /// `section_visibility`, holds it for the game to ask the mods first.
    pub(super) fn submit_mesh(
        &mut self,
        (x, sy, z): (i32, i32, i32),
        region: &Arc<Region>,
        first: bool,
    ) {
        if self.section_visibility.listening {
            self.section_visibility.pending.push(VisibilityRequest {
                key: (x, sy, z),
                region: region.clone(),
                seq: self.sequence,
                first,
            });
            return;
        }
        self.queue_mesh((x, sy, z), region.clone(), self.sequence, first, None);
    }
    fn queue_mesh(
        &mut self,
        (x, sy, z): (i32, i32, i32),
        region: Arc<Region>,
        seq: u64,
        first: bool,
        visibility: Option<FrozenPlan>,
    ) {
        let mut settings = self.mesh_settings();
        settings.visibility = visibility.map(Arc::new);
        let job = Job::Mesh(x, sy, z, region, seq, settings);
        self.submit(Some((x, z)), if first { 2 } else { 0 }, job);
    }
    /// The sections chosen this update that wait for the mods' `section_visibility` answers.
    /// Empty while no mod listens.
    pub fn take_visibility_requests(&mut self) -> Vec<VisibilityRequest> {
        std::mem::take(&mut self.section_visibility.pending)
    }
    /// The mods' combined answers to [`take_visibility_requests`](Self::take_visibility_requests),
    /// in its order, applied in one step on the world thread, after the sections still waiting
    /// for room. A section whose mesh sequence moved on (remeshed, reloaded, refreshed) or whose
    /// column unloaded is refused and its answer dropped: it is asked again when it is queued
    /// again. A section whose outer cells' answer changed dirties its loaded neighbours, whose
    /// border reads them. The cached answer and the frozen plan are charged to the workers'
    /// shared in-flight bytes ([`ANSWER_BYTES`], [`PLAN_BYTES`]) before anything is changed, and
    /// a section that finds no room waits, answered, for the next call.
    pub fn submit_visibility_answers(
        &mut self,
        world: &mut World,
        answered: Vec<(VisibilityRequest, Option<SectionAnswer>)>,
    ) {
        let mut answered = {
            let mut all = std::mem::take(&mut self.section_visibility.waiting);
            all.extend(answered);
            all
        };
        for (request, answer) in answered.drain(..) {
            let (x, sy, z) = request.key;
            if self.meshing.get(&request.key) != Some(&request.seq)
                || !world.chunks.contains_key(&(x, z))
            {
                continue;
            }
            let leases = Lease::charge(PLAN_BYTES).and_then(|plan| {
                let held = match &answer {
                    Some(_) => Some(Lease::charge(ANSWER_BYTES)?),
                    None => None,
                };
                Some((plan, held))
            });
            let Some((plan_lease, answer_lease)) = leases else {
                self.section_visibility.waiting.push((request, answer));
                continue;
            };
            let VisibilityRequest {
                key,
                region,
                seq,
                first,
                ..
            } = request;
            let answers = &mut self.section_visibility.answers;
            let before = answers.get(&key).map(|held| &held.answer);
            if SectionAnswer::border_differs(before, answer.as_ref()) {
                dirty_neighbours(world, key);
            }
            match (answer, answer_lease) {
                (Some(answer), Some(_lease)) => answers.insert(key, HeldAnswer { answer, _lease }),
                _ => answers.remove(&key),
            };
            let answers = &self.section_visibility.answers;
            let plan = plan_section(
                answers.get(&key).map(|held| &held.answer),
                |[dx, dy, dz]| {
                    answers
                        .get(&(x + dx, sy + dy, z + dz))
                        .map(|held| &held.answer)
                },
            )
            .map(|plan| FrozenPlan {
                plan,
                _lease: plan_lease,
            });
            self.queue_mesh(key, region, seq, first, plan);
        }
    }
    /// A mod's `refresh_render_visibility`, or the join: whether a mod listens now, every
    /// answer forgotten and, when a mod listens or listened, every loaded section meshed again
    /// (meshes in flight are refused as stale). With no mod before or after, nothing happens.
    pub fn refresh_section_visibility(&mut self, world: &mut World, listening: bool) {
        let state = &mut self.section_visibility;
        if !listening && !state.listening {
            return;
        }
        state.listening = listening;
        state.pending.clear();
        state.waiting.clear();
        state.answers.clear();
        state.logged.clear();
        self.reload_sections(world);
    }
    /// Columns that unloaded: their sections' answers go, and the loaded neighbours whose border
    /// read them mesh again.
    pub(super) fn forget_visibility(&mut self, world: &mut World, columns: &[(i32, i32)]) {
        let answers = &mut self.section_visibility.answers;
        if answers.is_empty() {
            return;
        }
        let gone: Vec<_> = answers
            .keys()
            .copied()
            .filter(|(x, _, z)| columns.contains(&(*x, *z)))
            .collect();
        for key in gone {
            let held = answers.remove(&key);
            if SectionAnswer::border_differs(held.as_ref().map(|held| &held.answer), None) {
                dirty_neighbours(world, key);
            }
        }
    }
}

/// Dirties the loaded sections around `key` (all 26, edges and corners too), whose one-block
/// border reads its cells.
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
