//! One live registration table for provider type coverage and forwarding metadata.
use super::*;
use mod_vocab::__private::parts_installation::capability_provider_binding;
use std::any::TypeId;
use sunrise_mod_runtime::registry_keys::Names;

impl PartRegistrar {
    pub(crate) fn capability_parts() -> Parts {
        let mut parts = Parts::default();
        for (name, handler) in
            mod_vocab::__private::parts_installation::builtin_block_capabilities()
        {
            parts.block_capability_order.push(name);
            parts
                .block_capability_metadata
                .insert(name, (Arc::from(handler), None));
        }
        parts
    }

    pub(crate) fn validate_capability_name(&mut self, name: &str) -> bool {
        if crate::Identifier::parse_namespaced(name).is_some() {
            return true;
        }
        self.refused.push(format!(
            "mod {} used capability {name:?}, which is not a namespaced name",
            self.mod_id
        ));
        false
    }

    /// Registry requests already carry validated names, including forward references whose
    /// existence the completed registry will check at load.
    pub(crate) fn capability_key_target<'a>(
        &mut self,
        name: &str,
        kind: &str,
        registry: &'a Names,
        slot: u32,
    ) -> Option<&'a str> {
        let id = registry.id(slot);
        if id.is_none() {
            self.refused.push(format!(
                "mod {} used capability {name:?} for an invalid {kind} key slot {slot}",
                self.mod_id
            ));
        }
        id
    }

    /// Each table keeps one binding type per name, so only one row in each domain is needed.
    pub(crate) fn check_capability_binding(
        &mut self,
        by: ProvidedBy,
        name: &str,
        (binding_type, handler): (TypeId, &'static str),
    ) -> bool {
        let conflict = |table: &Capabilities<NativeCapabilityProvider>| {
            let existing = table.get(name)?.values().flatten().next()?;
            let (existing_type, existing_handler) = capability_provider_binding(&existing.provider);
            (existing_type != binding_type).then_some((existing.mod_id, existing_handler))
        };
        let existing = match by {
            ProvidedBy::Block | ProvidedBy::BlockEntity => conflict(&self.parts.block_capabilities)
                .or_else(|| conflict(&self.parts.capabilities)),
            ProvidedBy::Item => conflict(&self.parts.item_capabilities),
            ProvidedBy::Entity => conflict(&self.parts.entity_capabilities),
        };
        if let Some((owner, existing_handler)) = existing {
            let conflict = if existing_handler == handler {
                "incompatible handler or context types"
            } else {
                "two handler types"
            };
            self.refused.push(format!(
                "capability {name} is answered with {conflict}, {existing_handler} by {owner} and {handler} by {}",
                self.mod_id
            ));
            return false;
        }
        true
    }

    /// A name has one handler type whether first encountered by a provider, a query or metadata.
    pub(crate) fn note_block_capability(
        &mut self,
        name: &'static str,
        handler: &'static str,
        binding_type: Option<TypeId>,
    ) -> bool {
        if !self.validate_capability_name(name) {
            return false;
        }
        if let Some(binding_type) = binding_type
            && !self.check_capability_binding(ProvidedBy::Block, name, (binding_type, handler))
        {
            return false;
        }
        if let Some((known, _)) = self.parts.block_capability_metadata.get(name) {
            if known.as_ref() != handler {
                self.refused.push(format!("capability {name} is answered with two handler types, {known} and {handler} by {}", self.mod_id));
                return false;
            }
        } else {
            self.parts.block_capability_order.push(name);
            self.parts
                .block_capability_metadata
                .insert(name, (Arc::from(handler), None));
        }
        true
    }

    fn block_capability_covers(&self, name: &str, id: &str) -> bool {
        let declared_default = if name == crate::ITEMS.name {
            self.parts.default_block_containers.contains_key(id)
        } else if name == crate::FLUIDS.name {
            self.parts.default_block_fluids.contains_key(id)
        } else if name == crate::ENERGY.name {
            self.parts.default_block_energy.contains_key(id)
        } else {
            false
        };
        declared_default
            || self
                .parts
                .block_capabilities
                .get(name)
                .and_then(|types| types.get(id))
                .is_some_and(|providers| !providers.is_empty())
            || mod_vocab::__private::parts_installation::game_capability_registered(
                ProvidedBy::Block,
                name,
                id,
            )
            || self.parts.capabilities.get(name).is_some_and(|types| {
                types.iter().any(|(kind, providers)| {
                    !providers.is_empty()
                        && (self.block_entity_types.iter().any(|(_, declaration)| {
                            declaration.id == **kind
                                && declaration.blocks.iter().any(|block| block == id)
                        }) || mod_vocab::__private::parts_installation::block_entity_covers(
                            kind, id,
                        ))
                })
            })
            || mod_vocab::__private::parts_installation::block_capability_dependency(name)
                .is_some_and(|source| self.block_capability_covers(source, id))
    }
}

impl CapabilityRegistrationRegistrar for PartRegistrar {
    fn entity_type_key(&mut self, id: &str) -> crate::EntityTypeKey {
        match self
            .registry_keys
            .borrow_mut()
            .entity_types
            .request(self.mod_id, id)
        {
            Ok(slot) => mod_vocab::__private::parts_installation::entity_type_key(slot),
            Err(error) => {
                self.refused.push(error);
                mod_vocab::__private::parts_installation::entity_type_key(u32::MAX)
            }
        }
    }

    fn block_capability_registered(
        &mut self,
        name: &'static str,
        handler: &'static str,
        block: &crate::BlockKey,
    ) -> bool {
        let registry_keys = Rc::clone(&self.registry_keys);
        let keys = registry_keys.borrow();
        let Some(id) = self.capability_key_target(
            name,
            "block",
            &keys.blocks,
            mod_vocab::__private::parts_installation::block_key_slot(*block),
        ) else {
            return false;
        };
        self.note_block_capability(name, handler, None) && self.block_capability_covers(name, id)
    }

    fn entity_capability_registered(&mut self, name: &str, kind: &crate::EntityTypeKey) -> bool {
        if !self.validate_capability_name(name) {
            return false;
        }
        let registry_keys = Rc::clone(&self.registry_keys);
        let keys = registry_keys.borrow();
        let Some(id) = self.capability_key_target(
            name,
            "entity type",
            &keys.entity_types,
            mod_vocab::__private::parts_installation::entity_type_key_slot(*kind),
        ) else {
            return false;
        };
        self.parts
            .entity_capabilities
            .get(name)
            .and_then(|types| types.get(id))
            .is_some_and(|providers| !providers.is_empty())
            || mod_vocab::__private::parts_installation::game_capability_registered(
                ProvidedBy::Entity,
                name,
                id,
            )
    }

    fn item_capability_registered(&mut self, name: &str, item: &crate::ItemKey) -> bool {
        if !self.validate_capability_name(name) {
            return false;
        }
        let registry_keys = Rc::clone(&self.registry_keys);
        let keys = registry_keys.borrow();
        let Some(id) = self.capability_key_target(
            name,
            "item",
            &keys.items,
            mod_vocab::__private::parts_installation::item_key_slot(*item),
        ) else {
            return false;
        };
        self.parts
            .item_capabilities
            .get(name)
            .and_then(|types| types.get(id))
            .is_some_and(|providers| !providers.is_empty())
            || mod_vocab::__private::parts_installation::game_capability_registered(
                ProvidedBy::Item,
                name,
                id,
            )
            || (name == crate::ITEM_ITEMS.name
                && self.parts.default_item_containers.contains_key(id))
    }

    fn set_block_capability_proxyable(
        &mut self,
        name: &'static str,
        handler: &'static str,
        proxyable: bool,
    ) {
        if !self.note_block_capability(name, handler, None) {
            return;
        }
        let (_, known) = self
            .parts
            .block_capability_metadata
            .get_mut(name)
            .expect("declared above");
        if known.is_some_and(|known| known != proxyable) {
            self.refused.push(format!("mod {} marked capability {name} proxyable={proxyable}, contradicting its earlier explicit mark", self.mod_id));
            return;
        }
        *known = Some(proxyable);
    }

    fn block_capability_names(&self) -> Vec<String> {
        self.parts
            .block_capability_order
            .iter()
            .map(|name| (*name).to_owned())
            .collect()
    }

    fn block_capability_proxyable(&self, name: &str) -> bool {
        self.parts
            .block_capability_metadata
            .get(name)
            .is_some_and(|(_, proxyable)| *proxyable == Some(true))
    }
}
