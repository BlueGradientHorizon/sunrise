//! The world-thread step between choosing a section to mesh and queuing it, for
//! `section_visibility` (docs/design/mod-api/mesh-visibility.md): while a mod listens, a chosen
//! section waits here, the game asks the mods on its own thread
//! ([`Streaming::take_visibility_requests`]) and hands back their combined answer
//! ([`Streaming::submit_visibility_answers`]), which is checked against the section's current
//! mesh sequence and frozen into the job. With no mod listening a chosen section is queued at
//! once, exactly as before.
use super::{Job, Streaming};
use crate::mesher::{Region, SectionAnswer, SectionVisibilityPlan, plan_section};
use crate::world::World;
use crate::fast_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

/// The sections waiting for the mods' visibility answers, and every section's last combined
/// answer, which its neighbours' plans read for their one-block border.
#[derive(Default)]
pub struct SectionPreparation {
    /// Whether a mod listens to `section_visibility`, set by
    /// [`Streaming::refresh_section_visibility`]; false is the no-mod path.
    listening: bool,
    /// A listener panicked: no section is asked until the next refresh.
    disabled: bool,
    pending: Vec<VisibilityRequest>,
    answers: FxHashMap<(i32, i32, i32), Arc<SectionAnswer>>,
    /// The mods and refusal codes logged since the last refresh: each is logged once.
    logged: FxHashSet<(&'static str, &'static str)>,
}

impl SectionPreparation {
    /// Whether sections wait for the mods' answers before they are queued.
    pub fn listening(&self) -> bool {
        self.listening
    }
    /// Whether the mods are asked: listening and no listener failed since the last refresh.
    pub fn asks(&self) -> bool {
        self.listening && !self.disabled
    }
    /// A listener panicked: stop asking until the next refresh; sections mesh without answers.
    pub fn disable(&mut self) {
        self.disabled = true;
    }
    /// Whether `mod_id`'s refusal `code` is new since the last refresh, so it is logged once.
    pub fn first_refusal(&mut self, mod_id: &'static str, code: &'static str) -> bool {
        self.logged.insert((mod_id, code))
    }
    /// The last combined answer of section `key`; `None` when it changes nothing.
    pub fn answer(&self, key: (i32, i32, i32)) -> Option<&SectionAnswer> {
        self.answers.get(&key).map(|a| &**a)
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
        visibility: Option<Arc<SectionVisibilityPlan>>,
    ) {
        let mut settings = self.mesh_settings();
        settings.visibility = visibility;
        let job = Job::Mesh(x, sy, z, region, seq, settings);
        self.submit(Some((x, z)), if first { 2 } else { 0 }, job);
    }
    /// The sections chosen this update that wait for the mods' `section_visibility` answers.
    /// Empty while no mod listens.
    pub fn take_visibility_requests(&mut self) -> Vec<VisibilityRequest> {
        std::mem::take(&mut self.section_visibility.pending)
    }
    /// The mods' combined answers to [`take_visibility_requests`](Self::take_visibility_requests),
    /// in its order, applied in one step on the world thread. A section whose mesh sequence moved
    /// on (remeshed, reloaded, refreshed) or whose column unloaded is refused and its answer
    /// dropped: it is asked again when it is queued again. A section whose outer cells' answer
    /// changed dirties its loaded neighbours, whose border reads them.
    pub fn submit_visibility_answers(
        &mut self,
        world: &mut World,
        answered: Vec<(VisibilityRequest, Option<SectionAnswer>)>,
    ) {
        for (request, answer) in answered {
            let VisibilityRequest {
                key,
                region,
                seq,
                first,
            } = request;
            let (x, sy, z) = key;
            if self.meshing.get(&key) != Some(&seq) || !world.chunks.contains_key(&(x, z)) {
                continue;
            }
            let answers = &mut self.section_visibility.answers;
            if SectionAnswer::border_differs(answers.get(&key).map(|a| &**a), answer.as_ref()) {
                dirty_neighbours(world, key);
            }
            match answer {
                Some(answer) => answers.insert(key, Arc::new(answer)),
                None => answers.remove(&key),
            };
            let answers = &self.section_visibility.answers;
            let plan = plan_section(answers.get(&key).map(|a| &**a), |[dx, dy, dz]| {
                answers.get(&(x + dx, sy + dy, z + dz)).map(|a| &**a)
            });
            self.queue_mesh(key, region, seq, first, plan.map(Arc::new));
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
        state.disabled = false;
        state.pending.clear();
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
            let answer = answers.remove(&key);
            if SectionAnswer::border_differs(answer.as_deref(), None) {
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
