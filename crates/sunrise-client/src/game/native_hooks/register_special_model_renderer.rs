use super::*;

impl Game<'_> {
    /// The special item model types a mod draws (NeoForge's `RegisterSpecialModelRendererEvent`):
    /// Register a decoder and bake factory for each type. The factory builds a renderer for each
    /// `minecraft:special` model node of that type as client resources load. A later registration
    /// replaces the earlier one, including builtin types. Asked once at client startup.
    pub(in crate::game) fn native_register_special_model_renderer(
        context: &mod_api::RegistrationContext,
        renderers: &mut dyn SpecialModelRenderers,
    ) {
        Self::fire_register_special_model_renderer(context, renderers);
    }
}
