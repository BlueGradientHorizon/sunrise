pub(super) struct ItemStackView<'a>(pub(super) super::ItemKey<'a>);

impl<'a> ItemStackView<'a> {
    pub(super) fn item(&self) -> super::ItemKey<'a> {
        self.0
    }
}
