//! The hit-local kill-credit loan, separate from the level's block/random loan.
use super::{Attack, Hurting};
use crate::{
    items::ItemStack,
    mobs::{Mob, Who},
};

/// Resolves the pushed entity's credit from its current hurt memories. Implemented by the
/// game's narrow per-hit context; it does not expose the game through a level host.
pub trait KnockbackCredit {
    /// Commit accepted-hit player responsibility before the default push.
    fn accepted_mob_hit(&mut self, mob: &mut Mob);
    /// The credit entity and its main hand. Zero means a genuinely unresolved credit.
    fn snapshot<'a>(
        &'a self,
        pushed: &Hurting<'_>,
        attack: &'a Attack,
    ) -> (i32, Option<&'a ItemStack>);
}

/// The credit a level carries: the site's loan, or, only for a level built with no `Game`
/// (the lower crates, a borrowed rider's tick), none lent, resolved by [`from_memory`].
pub(super) enum Credit<'a> {
    Memory,
    /// A game site that never makes a living push: a push through it is a wrong choice of
    /// site, which debug builds and tests fail on and release resolves like `Memory`.
    NoPush,
    Loan(&'a mut dyn KnockbackCredit),
}

impl Credit<'_> {
    pub(super) fn reborrow(&mut self) -> Credit<'_> {
        match self {
            Credit::Memory => Credit::Memory,
            Credit::NoPush => Credit::NoPush,
            Credit::Loan(credit) => Credit::Loan(&mut **credit),
        }
    }
}

/// A push with no loan: a mob's credit from its own hurt memories (remembered player first,
/// else the recent mob, inclusive at tick 100), held items only when the credited entity is
/// this hit's attacker. Anything else, including every non-mob victim, is unresolved (zero).
pub(super) fn from_memory<'a>(
    pushed: &Hurting<'_>,
    attack: &'a Attack,
) -> (i32, Option<&'a ItemStack>) {
    let Hurting::Mob(mob, _) = pushed else {
        return (0, None);
    };
    let credited = match mob.recent_hurt_by() {
        Some(Who::Player) => attack.player_uuid.is_some(),
        Some(Who::Mob(id)) => mob.ai.last_hurt_by_player <= 0 && id as i32 == attack.attacker,
        None => false,
    };
    if credited {
        (attack.attacker, attack.held[0].as_ref())
    } else {
        (0, None)
    }
}
