use super::*;

impl Game<'_> {
    /// A section is about to mesh (`section_visibility`): each listening mod may push one answer
    /// for it into `answers`, read from `input`, the raw section and its border. Client-side,
    /// on the world thread, before the section goes to a meshing worker.
    pub(in crate::game) fn native_section_visibility(
        &self,
        call: mod_api::__private::SectionVisibilityCall<'_>,
    ) {
        self.fire_section_visibility(call);
    }
}
