use super::*;
use sunrise_gameplay_common::game::lan_actions::attack_checks::arrow_attackable;
use sunrise_protocol::text::translatable;

impl crate::game::ServerGame<'_> {
    /// Whether a guest's attack on entity `id` ends before `Player.attack` (handleAttack).
    /// The guest who attacks an `ItemEntity`, an `ExperienceOrb`, themselves or an
    /// `AbstractArrow` that is not `isAttackable` is disconnected with
    /// `multiplayer.disconnect.invalid_entity_attacked`.
    pub(in crate::game::server_side) fn guest_attack_stops(&mut self, gi: usize, id: i32) -> bool {
        use crate::game::commands::Target;
        let Some(host) = self.server.lan.as_ref() else {
            return true;
        };
        let guest = &host.hub.guests[gi];
        let present = host.hub.here(guest)
            || (guest.is_fake() && guest.placed && guest.dimension == host.hub.config.dimension);
        if !present || !guest.left_reason.is_empty() {
            return true;
        }
        let conn = guest.conn;
        let target = if guest.entity_id == id {
            Some(guest.bounds())
        } else {
            self.common.level_entity(id).and_then(|target| {
                let refused = match target {
                    Target::Item(_) | Target::Orb(_) => true,
                    Target::Projectile(_) | Target::Trident(_) => {
                        let kind = self.common.target_type(target);
                        if !matches!(
                            kind.as_str(),
                            "minecraft:arrow" | "minecraft:spectral_arrow" | "minecraft:trident"
                        ) {
                            return None;
                        }
                        let sync = host.hub.lan.shared.sync();
                        !arrow_attackable(&sync.tags, sync.id("minecraft:entity_type", &kind))
                    }
                    _ => false,
                };
                refused
                    .then(|| self.selector_target_bounds(target))
                    .flatten()
            })
        };
        let Some(target) = target else {
            return false;
        };
        if !guest.client_loaded()
            || !self.inside_border(&target)
            || self.as_guest_at(gi, conn, |g| {
                g.within_attack_range(&target) && !g.holds_piercing_weapon()
            }) != Ok(true)
        {
            return true;
        }
        let host = self.server.lan.as_mut().unwrap();
        let name = host.hub.guests[gi].name.clone();
        host.hub.kick(
            conn,
            translatable("multiplayer.disconnect.invalid_entity_attacked", &[]),
        );
        log::warn!("Player {name} tried to attack an invalid entity");
        true
    }
    /// The main-hand stack has `minecraft:piercing_weapon` (a spear's).
    pub(in crate::game::server_side) fn holds_piercing_weapon(&self) -> bool {
        self.common.held().is_some_and(|stack| {
            self.common.pack
                .item_models
                .components_view(&sunrise_sim::item_models::stack_view(stack))
                .contains_key("minecraft:piercing_weapon")
        })
    }
}
