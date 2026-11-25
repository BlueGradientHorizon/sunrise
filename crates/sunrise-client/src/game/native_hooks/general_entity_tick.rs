#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::{
    commands::{Target, entity_pitch},
    general::GeneralRow,
    transactions::OpenTransactions,
};
#[allow(unused_imports)]
use mod_api::{
    EntityRef, EntityServerAccess, EntityServerScope, EntitySubjectView, ResidentGeneralEntity,
    ServerHost,
};
#[allow(unused_imports)]
use sunrise_client::lan::playerdata::ints_uuid;
#[allow(unused_imports)]
use sunrise_core::geometry::Vec3;

pub(in crate::game) struct ResidentGeneralRow<'a> {
    pub(in crate::game) row: &'a mut GeneralRow,
    pub(in crate::game) journal: &'a mut OpenTransactions,
    pub(in crate::game) kind: &'a str,
}

pub(in crate::game) struct GeneralTickAccess<'g, 'w, 'k> {
    pub(in crate::game) game: &'g mut crate::game::ServerGame<'w>,
    pub(in crate::game) subject: EntityRef,
    pub(in crate::game) kind: &'k str,
}

/// The authoritative tick of one general entity row of type `kind`
/// (`mymod:robot`) (#792 stage 4): `ServerLevel.tickNonPassenger`'s
/// `entity.tick()` for a row the type registry owns, its rider subtree
/// ticked after it. The scope lends the live row sequentially with `ServerHost`.
/// Vanilla families never reach this call. Server-side.
pub(in crate::game) fn native_general_entity_tick(
    call: mod_api::__private::GeneralEntityTickCall<'_>,
) {
    crate::game::native_hooks::fire::fire_general_entity_tick(call);
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::game::{api_values::test_snapshot, native_entity_refs::tests::with_game};
}
