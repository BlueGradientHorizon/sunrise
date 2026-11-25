#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{EntitySections, LiveSectionObserver, Scope};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::observer_cache::ObserverCache;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::observer_cache::SharedObserver;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::observer_cache::SharedSections;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::observer_cache::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::mobs::living::section_writes::Observer;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{cell::RefCell, rc::Rc};

impl ObserverCache {
    pub(in crate::game) fn begin(
        &mut self,
        sections: &mut EntitySections,
        scope: Option<Scope>,
    ) -> (Option<SharedSections>, Option<Observer>) {
        let shared = self.sections.get_or_insert_with(Default::default);
        std::mem::swap(sections, &mut shared.borrow_mut());
        let observer = scope.map(|scope| {
            let observer = self.observer.get_or_insert_with(|| {
                Rc::new(RefCell::new(LiveSectionObserver::new(
                    Rc::clone(shared),
                    scope,
                )))
            });
            observer.borrow_mut().scope = scope;
            Rc::clone(observer) as Observer
        });
        (Some(Rc::clone(shared)), observer)
    }
}
