//! Per-section mesh visibility a client mod decides (`native_section_visibility`, the mesh
//! visibility design): asked on the world thread for each section chosen to mesh, before it goes
//! to a worker, with the raw section captured from the blocks the job will mesh. The mods'
//! answers are checked whole and combined here; the worker reads only the frozen plan.

use super::Game;
use sunrise_client::mesher::{SectionAnswer, SectionPlanBuilder, borrow, capture_input};
use sunrise_client::streaming::VisibilityRequest;
use mod_api::{
    Answered, BlockStateId, SectionCell, SectionVisibility, SectionVisibilityInput,
    SectionVisibilityRefusal,
};

impl Game<'_> {
    /// Asks the mods about the sections the streaming update chose to mesh, then queues them
    /// with their frozen plans. Nothing waits while no mod listens: the update queued them.
    pub(in crate::game) fn prepare_section_visibility(&mut self) {
        let requests = self.streaming.take_visibility_requests();
        if requests.is_empty() {
            return;
        }
        let switched_off = mod_api::__private::poisoned_listeners(mod_api::HookId::SectionVisibility);
        let mut answered = Vec::with_capacity(requests.len());
        let (mut cells, mut light) = (Vec::new(), Vec::new());
        for request in requests {
            let answer = self.ask_section_visibility(&request, &mut cells, &mut light);
            // A listener unwound in that call and was switched off alone: the other listeners
            // keep answering, but every answer given so far, to this batch and earlier ones,
            // may hold what it gave, so all are dropped and every section is asked again.
            if mod_api::__private::poisoned_listeners(mod_api::HookId::SectionVisibility)
                != switched_off
            {
                log::warn!(
                    "Sunrise: a section_visibility listener failed ({}); it is switched off for \
                     the session and every section is asked again without it",
                    SectionVisibilityRefusal::Failed
                );
                self.streaming
                    .refresh_section_visibility(&mut self.world, true);
                return;
            }
            answered.push((request, answer));
        }
        self.streaming
            .submit_visibility_answers(&mut self.world, answered);
    }

    /// One section's combined answer: every mod's answer checked whole, a refused one logged
    /// once and left out. `None` when the answers change nothing.
    fn ask_section_visibility(
        &mut self,
        request: &VisibilityRequest,
        cells: &mut Vec<Option<u32>>,
        light: &mut Vec<u8>,
    ) -> Option<SectionAnswer> {
        let (x, sy, z) = request.key;
        let region = borrow(&request.region);
        capture_input(&region, sy, cells, light);
        let min_y = region[1][1].map_or(self.world.bottom_y(), |c| c.min_y());
        let raw = cells
            .iter()
            .map(|cell| cell.map_or(SectionCell::Missing, |id| SectionCell::State(BlockStateId(id))))
            .collect();
        let input =
            match SectionVisibilityInput::new([x * 16, min_y + sy * 16, z * 16], raw, light.clone()) {
                Ok(input) => input,
                Err(refusal) => {
                    log::warn!("Sunrise: section visibility input refused: {refusal}");
                    return None;
                }
            };
        let mut answers: Vec<Answered<SectionVisibility>> = Vec::new();
        self.native_section_visibility(mod_api::__private::SectionVisibilityCall {
            input: &input,
            answers: &mut answers,
        });
        let mut plan = SectionPlanBuilder::new();
        for Answered { answer, mod_id, .. } in &answers {
            if let Err(refusal) = answer.check() {
                if self
                    .streaming
                    .section_visibility
                    .first_refusal(mod_id, refusal.code())
                {
                    log::warn!(
                        "Sunrise: mod {mod_id}'s section visibility at {:?} refused whole: {refusal}",
                        input.origin
                    );
                }
                continue;
            }
            match answer {
                SectionVisibility::HideTerrain => plan.hide_terrain(),
                SectionVisibility::Keep(cells) => {
                    for cell in cells {
                        plan.restrict(cell.pos, cell.model, cell.fluid, cell.faces);
                    }
                }
                other => {
                    if self
                        .streaming
                        .section_visibility
                        .first_refusal(mod_id, "E_SECTION_VISIBILITY_UNKNOWN")
                    {
                        log::warn!(
                            "Sunrise: mod {mod_id}'s section visibility answer {other:?} is not \
                             one this game knows (E_SECTION_VISIBILITY_UNKNOWN); refused whole"
                        );
                    }
                }
            }
        }
        plan.finish()
    }
}
