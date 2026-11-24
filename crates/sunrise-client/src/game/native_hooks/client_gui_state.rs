#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// The same screen identifier provided to screen hooks.
    pub(in crate::game) fn observed_screen_id(&self) -> Option<ScreenId> {
        self.observed_screen_view().map(|view| ScreenId {
            id: view.id.to_owned(),
        })
    }
}
