#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{resource_tables::PackParts, *};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::CoreLoadError;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::CoreTables;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::GameplayTables;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::Progress;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::SKIP_FACTS_MEMO;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::STAGE_LINES;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::StartupLoadContext;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::Step;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::gameplay_tables;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::launch_world;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::load_core;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::load_resources;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::load_resources_for_mods;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::load_resources_skip_lang;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::load_resources_sync;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::load_resources_uncached;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::named_scope;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::publish_entity_type_defaults;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::synchronous_startup;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::take_modified_attribute_rows;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::client_side) use crate::game::resource_load::test_cache::{
    acquire_fresh_child_permit, cached_structure_spawns,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::with_mod_attribute_additions;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::with_mod_attributes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::with_mod_widened_bounds;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::with_skip_facts_memo;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::write_prepared_files;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::resource_load::*;

impl Progress {
    pub(in crate::game) fn fraction(&self) -> f32 {
        self.value.load(Ordering::Relaxed) as f32 / (self.lines.len() - 1) as f32
    }
    pub(in crate::game) fn line(&self) -> &'static str {
        self.lines[(self.value.load(Ordering::Relaxed) as usize).min(self.lines.len() - 1)]
    }
    pub(in crate::game) fn new() -> Self {
        Self::with(&STAGE_LINES)
    }
    pub(in crate::game) fn set(&self, stage: u32) {
        self.value.store(stage, Ordering::Relaxed);
    }
    pub(in crate::game) fn with(lines: &'static [&'static str]) -> Self {
        Self {
            value: Arc::new(AtomicU32::new(0)),
            lines,
        }
    }
}

impl<'scope, T: Send + 'scope> Step<'scope, T> {
    pub(in crate::game) fn join(self) -> T {
        match self {
            Self::Thread(handle) => handle.join().unwrap(),
            Self::Later(f) => f(),
        }
    }
    /// `name` names the thread when one is spawned ([`named_scope`]).
    pub(in crate::game) fn spawn(
        scope: &'scope std::thread::Scope<'scope, '_>,
        parallel: bool,
        name: &str,
        f: impl FnOnce() -> T + Send + 'scope,
    ) -> Self {
        if parallel {
            Self::Thread(named_scope(scope, name, f))
        } else {
            Self::Later(Box::new(f))
        }
    }
}
