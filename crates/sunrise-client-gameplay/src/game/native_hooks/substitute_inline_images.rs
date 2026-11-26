

#[cfg(feature = "client")]
impl crate::game::ClientGame<'_> {
    pub(in crate::game) fn substitute_inline_images(&self, text: &Text) -> Option<Text> {
        substitute(&self.client.gui.atlas, text, |run, images| {
            self.native_substitute_inline_images(mod_api::__private::SubstituteInlineImagesCall {
                run,
                images,
            });
        })
    }

    /// Ask client mods for replacement atlas images before laying out this text run.
    pub(in crate::game) fn native_substitute_inline_images(
        &self,
        call: mod_api::__private::SubstituteInlineImagesCall<'_>,
    ) {
        self.fire_substitute_inline_images(call);
    }
}