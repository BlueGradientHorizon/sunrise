//! The one stored pick, refreshed before tick inputs and frame extraction.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::{Dim, view_vector};
#[allow(unused_imports)]
use mod_api::CrosshairTarget;

#[cfg(test)]
#[path = "client_pick/tests.rs"]
pub(in crate::game) mod tests;

#[derive(Default)]
pub(in crate::game) struct ClientPick {
    pub ray: Option<([f64; 3], [f64; 3])>,
    pub(in crate::game) epoch: Option<mod_api::WorldEpoch>,
    pub(in crate::game) dimension: Option<Dim>,
    pub(in crate::game) origin: [f64; 3],
    pub(in crate::game) entity_reach: f64,
    pub(in crate::game) result: Option<CrosshairTarget>,
    pub entity_metadata: (bool, bool),
    pub rider_candidates: Vec<crate::game::pick_ray::PickCandidate>,
    #[cfg(test)]
    pub(in crate::game) test_forced_hit: Option<crate::game::Hit>,
    pub sections: std::cell::RefCell<crate::game::pick_entity::pick_sections::PickSectionStorage>,
    pub(in crate::game) dispatching: bool,
    /// How many picks have been taken.
    pub(in crate::game) taken: u64,
}

impl Game<'_> {
    /// A window always has a decision, including before the first pick (a miss).
    /// None in the outer option means a server ray or the current pick computation.
    pub(in crate::game) fn cached_entity_pick(&self) -> Option<Option<(EntityRef, f64, [f64; 3])>> {
        self.side_uses_stored_pick()
            .then(|| match self.crosshair_target() {
                Some(CrosshairTarget::Entity { entity, point }) => {
                    Some((entity, self.pick_distance(point), point))
                }
                _ => None,
            })
    }

    pub(in crate::game) fn pick_target_live(&self, result: CrosshairTarget) -> bool {
        use crate::game::api_values::TryIntoApi;
        match result {
            CrosshairTarget::Block { pos, state, .. } => {
                self.world.block(pos.x, pos.y, pos.z).try_into_api().ok() == Some(state)
            }
            CrosshairTarget::Entity { entity, .. } => {
                self.native_client_entity_target(entity).is_some()
            }
            _ => false,
        }
    }

    /// Whether Pick Block can name an item for the entity (`Entity.getPickResult`).
    pub(in crate::game) fn has_pick_result(target: Option<crate::game::commands::Target>) -> bool {
        use crate::game::commands::Target;
        !matches!(
            target,
            Some(
                Target::Projectile(_)
                    | Target::DragonPart(_)
                    | Target::Other(_)
                    | Target::General(_)
                    | Target::Tnt(_)
                    | Target::Falling(_)
            )
        )
    }

    pub(in crate::game) fn ask_can_rider_interact(
        &mut self,
        subject: mod_api::EntityRef,
        allowed: &mut bool,
    ) {
        let mut access = ClientPickEntityAccess {
            game: self,
            subject,
        };
        crate::game::native_hooks::native_can_rider_interact(
            mod_api::__private::CanRiderInteractCall {
                subject: mod_api::EntityClientScope::new(&mut access),
                allowed,
            },
        );
    }

    pub(in crate::game) fn native_client_pick(
        &mut self,
        call: mod_api::__private::ClientPickCall<'_>,
    ) {
        self.fire_client_pick(call);
    }
}

pub(in crate::game) fn native_can_rider_interact(
    call: mod_api::__private::CanRiderInteractCall<'_>,
) {
    crate::game::native_hooks::fire::fire_can_rider_interact(call);
}

pub(in crate::game) struct ResidentClientPickEntity {
    pub(in crate::game) kind: Option<mod_api::EntityTypeKey>,
    pub(in crate::game) position: [f64; 3],
    pub(in crate::game) rotation: [f32; 2],
    pub(in crate::game) uuid: Option<u128>,
}

impl mod_api::ResidentClientEntity for ResidentClientPickEntity {
    fn kind(&self) -> Option<mod_api::EntityTypeKey> {
        self.kind
    }
    fn position(&self) -> [f64; 3] {
        self.position
    }
    fn rotation(&self) -> [f32; 2] {
        self.rotation
    }
    fn uuid(&self) -> Option<u128> {
        self.uuid
    }
}

pub(in crate::game) struct ClientPickEntityAccess<'g, 'w> {
    pub(in crate::game) game: &'g mut Game<'w>,
    pub(in crate::game) subject: mod_api::EntityRef,
}

impl mod_api::EntityClientAccess for ClientPickEntityAccess<'_, '_> {
    fn route_kind(&self) -> Option<mod_api::EntityTypeKey> {
        self.game.client_pick_route_kind(self.subject)
    }

    fn with_view(&mut self, call: &mut dyn FnMut(mod_api::EntityClientView<'_>)) {
        let Some(data) = self.game.client_pick_entity_view(self.subject) else {
            return;
        };
        call(mod_api::EntityClientView::new(self.subject, &data));
    }

    fn host(&mut self) -> &mut dyn mod_api::ClientHost {
        self.game
    }
}
