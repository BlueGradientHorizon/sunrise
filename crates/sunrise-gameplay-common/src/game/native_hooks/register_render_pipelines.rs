use super::*;

/// Collect the mods' render pipeline registrations once at startup.
pub fn native_register_render_pipelines(
    context: &mod_api::RegistrationContext,
    pipelines: &mut Vec<mod_api::RenderPipeline>,
) {
    crate::game::native_hooks::fire::fire_register_render_pipelines(context, pipelines);
}
