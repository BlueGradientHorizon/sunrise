use super::*;

/// Joins the world fluids native mods declare in code to the content mods of their namespaces:
/// a fluid's file, if it has one, keeps saying how it looks.
pub fn accept_world_fluids(mods: &mut [ContentMod], fluids: &[FluidLook]) {
    for fluid in fluids {
        let namespace = fluid.id.split_once(':').map_or("", |(ns, _)| ns);
        let Some(owner) = mods
            .iter_mut()
            .find(|m| m.content.namespaces.iter().any(|owned| owned == namespace))
        else {
            continue;
        };
        match owner.content.fluids.iter_mut().find(|f| f.id == fluid.id) {
            Some(look) => look.world = fluid.world,
            None => owner.content.fluids.push(fluid.clone()),
        }
    }
}

static FLUID_INTERACTIONS: std::sync::RwLock<Vec<sunrise_mod_package::content::FluidInteraction>> =
    std::sync::RwLock::new(Vec::new());

/// Remembers the fluid interactions the content mods' files declare
/// (`data/<id>/sunrise/fluid_interaction/`), in load order and file name, for each world's open.
pub fn set_fluid_interactions(mods: &[ContentMod]) {
    *FLUID_INTERACTIONS
        .write()
        .unwrap_or_else(|e| e.into_inner()) = mods
        .iter()
        .flat_map(|m| m.content.fluid_interactions.iter().cloned())
        .collect();
}

/// The interactions [`set_fluid_interactions`] remembered.
pub fn fluid_interactions() -> Vec<sunrise_mod_package::content::FluidInteraction> {
    FLUID_INTERACTIONS
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}
