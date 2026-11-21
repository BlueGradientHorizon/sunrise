//! Prepared cauldron callbacks and their server-side dispatch.
use super::*;
use mod_api::{
    __private::game::{CallbackOwner, RegistrationContextNew, discard_registration},
    HookId,
};
use std::{cell::RefCell, sync::Arc};
use sunrise_client::cauldron::interactions::Dispatcher;

#[derive(Clone)]
pub(super) struct Callback {
    pub(super) handler: Arc<dyn mod_api::Interaction>,
    pub(super) owner: &'static str,
    pub(super) guard: Option<Arc<CallbackOwner>>,
}
impl Callback {
    fn new(handler: Arc<dyn mod_api::Interaction>) -> Self {
        let owner = mod_api::__private::cauldron_owner();
        let guard = CallbackOwner::new(owner).map(Arc::new);
        Self {
            handler,
            owner,
            guard,
        }
    }
}
pub(super) type Table = Dispatcher<Callback>;
thread_local! {
    static TABLE: RefCell<Arc<Table>> = RefCell::new(Arc::new(Table::default()));
}
pub(super) fn snapshot() -> Arc<Table> {
    TABLE.with_borrow(Arc::clone)
}
pub(super) fn clear() {
    install(Arc::new(Table::default()));
}
pub(super) fn install(table: Arc<Table>) {
    TABLE.with_borrow_mut(|old| *old = table);
}

/// What a `register_to_all` or `register_tag_to_all` entry answers for.
enum Target {
    Item(String),
    Tag(String),
}
pub(in crate::game) struct Registrar<'a> {
    table: Table,
    items: &'a Items,
    registry: &'a sunrise_world::registry::BlockRegistry,
    all: Vec<(Target, Callback)>,
    error: Option<mod_api::CauldronRegistrationError>,
}
/// A tag as entries keep it: its resource location, without `#`.
fn tag_id(tag: &str) -> Option<String> {
    let tag = tag.strip_prefix('#').unwrap_or(tag);
    mod_api::Identifier::parse(tag)
        .ok()
        .map(|id| id.to_string())
}
impl<'a> Registrar<'a> {
    fn new(items: &'a Items, registry: &'a sunrise_world::registry::BlockRegistry) -> Self {
        Self {
            table: Table::default(),
            items,
            registry,
            all: Vec::new(),
            error: None,
        }
    }

    pub(in crate::game) fn register_listener(
        &mut self,
        listen: &mut dyn FnMut(&mut dyn mod_api::CauldronInteractions) -> bool,
    ) {
        let mut pending = Registrar {
            table: Table::empty(),
            items: self.items,
            registry: self.registry,
            all: Vec::new(),
            error: None,
        };
        if listen(&mut pending) {
            let Registrar {
                table, all, error, ..
            } = pending;
            self.table.append_registrations(table);
            self.all.extend(all);
            if let Some(error) = error {
                self.error = Some(error);
            }
        } else {
            for Callback { handler, owner, .. } in pending
                .table
                .into_mod_handlers()
                .chain(pending.all.into_iter().map(|(_, callback)| callback))
            {
                discard_registration(
                    (
                        HookId::RegisterCauldronInteraction,
                        "register_cauldron_interaction",
                    ),
                    Some(owner),
                    handler,
                );
            }
        }
    }

    fn insert_tag(&mut self, block: String, tag: String, callback: Callback) {
        self.table.register_tag(block, tag, callback);
    }
    /// Record an admission error, which fails the whole preparation when it finishes.
    fn refuse(
        &mut self,
        error: mod_api::CauldronRegistrationError,
    ) -> std::result::Result<(), mod_api::CauldronRegistrationError> {
        self.error = Some(error);
        Err(error)
    }
    fn insert(&mut self, block: String, item: String, callback: Callback) {
        self.table.register(block, item, callback);
    }
    fn finish(mut self) -> Result<Arc<Table>> {
        if let Some(error) = self.error {
            return Err(format!("cauldron registration: {error:?}").into());
        }
        for (target, callback) in std::mem::take(&mut self.all) {
            for block in self.table.blocks() {
                match &target {
                    Target::Item(item) => self.insert(block, item.clone(), callback.clone()),
                    Target::Tag(tag) => self.insert_tag(block, tag.clone(), callback.clone()),
                }
            }
        }
        Ok(Arc::new(self.table))
    }
}
impl mod_api::CauldronInteractions for Registrar<'_> {
    fn register(
        &mut self,
        block: mod_api::BlockKey,
        item: mod_api::ItemKey,
        handler: Arc<dyn mod_api::Interaction>,
    ) -> std::result::Result<(), mod_api::CauldronRegistrationError> {
        use mod_api::CauldronRegistrationError as Error;
        let Some(block) = mod_api::__private::block_key_id(block)
            .filter(|id| self.registry.default_state(id).is_some())
        else {
            return self.refuse(Error::UnknownBlock);
        };
        let Some(item) =
            mod_api::__private::item_key_id(item).filter(|id| self.items.items.contains_key(*id))
        else {
            return self.refuse(Error::UnknownItem);
        };
        self.insert(block.to_string(), item.to_string(), Callback::new(handler));
        Ok(())
    }
    fn register_to_all(
        &mut self,
        item: mod_api::ItemKey,
        handler: Arc<dyn mod_api::Interaction>,
    ) -> std::result::Result<(), mod_api::CauldronRegistrationError> {
        let Some(item) =
            mod_api::__private::item_key_id(item).filter(|id| self.items.items.contains_key(*id))
        else {
            return self.refuse(mod_api::CauldronRegistrationError::UnknownItem);
        };
        self.all
            .push((Target::Item(item.to_string()), Callback::new(handler)));
        Ok(())
    }
    fn register_tag(
        &mut self,
        block: mod_api::BlockKey,
        tag: &str,
        handler: Arc<dyn mod_api::Interaction>,
    ) -> std::result::Result<(), mod_api::CauldronRegistrationError> {
        use mod_api::CauldronRegistrationError as Error;
        let Some(block) = mod_api::__private::block_key_id(block)
            .filter(|id| self.registry.default_state(id).is_some())
        else {
            return self.refuse(Error::UnknownBlock);
        };
        let Some(tag) = tag_id(tag) else {
            return self.refuse(Error::InvalidTag);
        };
        self.insert_tag(block.to_string(), tag, Callback::new(handler));
        Ok(())
    }
    fn register_tag_to_all(
        &mut self,
        tag: &str,
        handler: Arc<dyn mod_api::Interaction>,
    ) -> std::result::Result<(), mod_api::CauldronRegistrationError> {
        let Some(tag) = tag_id(tag) else {
            return self.refuse(mod_api::CauldronRegistrationError::InvalidTag);
        };
        self.all.push((Target::Tag(tag), Callback::new(handler)));
        Ok(())
    }
}
impl Game<'_> {
    pub(super) fn prepare_cauldron_interactions(&self, items: &Items) -> Result<Arc<Table>> {
        let mut registrar = Registrar::new(items, self.pack.assets.registry);
        Self::native_register_cauldron_interaction(
            &mod_api::RegistrationContext::new(),
            &mut registrar,
        );
        registrar.finish()
    }
    /// Invoke a mod's authoritative cauldron interaction on the server.
    pub(super) fn use_mod_cauldron(
        &mut self,
        pos: [i32; 3],
        under_water: bool,
        callback: &Callback,
    ) -> sunrise_client::cauldron::Outcome {
        use crate::game::api_values::TryIntoApi;
        use mod_api::{CauldronDecision, CauldronUse};
        use sunrise_client::cauldron::Outcome;
        let Some(guard) = callback.guard.as_ref() else {
            log::warn!(
                "cauldron interaction for {} has no installed owner",
                callback.owner
            );
            return Outcome::Pass;
        };
        let Some(held) = self.held().cloned() else {
            return Outcome::Pass;
        };
        let before = self.world.block(pos[0], pos[1], pos[2]);
        let Ok(index) = api_values::block_state::current() else {
            return Outcome::Pass;
        };
        let Ok(state) = index.to_api(before) else {
            return Outcome::Pass;
        };
        let Some(epoch) = self.native_lifecycle.epoch() else {
            return Outcome::Pass;
        };
        let player_id = self.native_acting_id();
        let entity_id = self
            .player_lookup_entity(player_id)
            .unwrap_or_else(|| self.native_acting_entity());
        let actor = mod_api::EntityRef::new(epoch, entity_id);
        let player = mod_api::__private::game::player_ref(actor, player_id);
        let level = self.level_ref(epoch, self.dim.key());
        let Ok(held) = held.try_into_api() else {
            return Outcome::Pass;
        };
        let input = CauldronUse {
            level,
            pos,
            state,
            player,
            hand: match self.hand {
                Hand::Main => mod_api::Hand::Main,
                Hand::Off => mod_api::Hand::Off,
            },
            held,
            underwater: under_water,
            creative: self.creative,
        };
        let decision = guard.call("cauldron interaction", "cauldron_interaction", || {
            callback.handler.interact(self, &input)
        });
        match decision {
            Ok(CauldronDecision::Success) => Outcome::Success,
            Ok(CauldronDecision::Consume) => Outcome::Consume,
            Ok(_) => Outcome::Pass,
            Err(why) => {
                log::warn!("cauldron interaction refused: {why}");
                Outcome::Pass
            }
        }
    }
}

#[cfg(test)]
#[path = "cauldron_interactions/tests.rs"]
mod tests;
