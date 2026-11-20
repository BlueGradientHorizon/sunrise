//! Components cross as the API's validated, flat node table, not Rust recursion.
use super::{Reader, Wire, WireError, Writer};
use crate::text::{Text, TextForm};

impl Wire for Text {
    const MIN: usize = TextForm::MIN;
    layout_via!(TextForm, "validated-text-node-table:v1", ordinary_only);
    fn put(&self, out: &mut Writer) {
        // Rebuilding the tree from its node table cannot charge the payload budget.
        if !out.ordinary_only() {
            return;
        }
        match TextForm::try_from(self) {
            Ok(form) => out.put(&form),
            Err(_) => out.fail(WireError::Depth),
        }
    }
    fn take(input: &mut Reader<'_>) -> Result<Self, WireError> {
        input.ordinary_only()?;
        Text::try_from(input.take::<TextForm>()?).map_err(|_| WireError::Invalid("text node table"))
    }
}
