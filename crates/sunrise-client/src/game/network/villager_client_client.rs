#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::villager_client::BUILTIN;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::villager_client::BuiltinCache;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::villager_client::builtin_registries;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::villager_client::receive;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::villager_client::server_meta;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::villager_client::unhappy_counter;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::villager_client::villager_data;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::villager_client::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::render::VILLAGER_TYPES;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::{
    Mob, Species, villager::ResolvedProfessions,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::{entity_data::Value, registry::Builtin};

impl super::Game<'_> {
    pub(in crate::game::client_side) fn client_professions(
        &self,
    ) -> super::Result<Arc<ResolvedProfessions>> {
        let table = self.prepare_client_professions()?;
        self.install_client_professions(&table);
        Ok(table)
    }
    pub(in crate::game) fn install_client_professions(&self, table: &ResolvedProfessions) {
        sunrise_client::mobs::render::villager_client::install(
            table,
            &self
                .res
                .mob_data()
                .villager_hats
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }
    pub(in crate::game) fn prepare_client_professions(
        &self,
    ) -> super::Result<Arc<ResolvedProfessions>> {
        use mod_api::__private::game::RegistrationContextNew;
        let declarations = self.mods.profession_declarations.get_or_init(|| {
            let mut registrar = mod_api::ProfessionDeclarations::default();
            crate::game::native_hooks::native_register_villager_professions(
                &mod_api::RegistrationContext::new(),
                &mut registrar,
            );
            registrar.into_entries()
        });
        // The client consumes only identity slots. World-dependent POI/trade references
        // are validated by the authoritative server's complete world preparation.
        ResolvedProfessions::prepare(declarations.clone(), |_, _| true)
            .map(Arc::new)
            .map_err(|errors| format!("client villager professions refused: {errors:?}").into())
    }
}
