//! A client mod's view and its own items (docs/modding/src/how-to/native-mods.md, "A client mod's view and items"): the
//! first block along the view in the loaded chunks, and items whose
//! right-click is the mod's alone, heard as use events the mod polls.
use super::{Game, Screen};
pub use mod_api::game::{HeldItemSlot, ModEvent};
use std::{collections::BTreeMap, sync::Arc};

/// The farthest a ray along the view reaches, in blocks.
pub(crate) const RAYCAST_MAX: f32 = 1024.0;
/// The ids a mod may claim.
const CLAIMS_MAX: usize = 64;
/// Events kept for a mod between two polls.
const EVENTS_MAX: usize = 256;

/// Client input captured by a claimed main-hand item, not a living entity's item use.
pub(super) struct ActiveClientItemUse {
    mod_id: String,
    original: HeldItemSlot,
    /// Only for comparing the current stack's kind; the original snapshot never changes.
    item_id: Arc<str>,
    slot: usize,
    ticks: u32,
}

/// The items client mods claim and their events waiting for a poll.
#[derive(Default)]
pub(super) struct ModInput {
    claims: BTreeMap<String, Vec<String>>,
    events: BTreeMap<String, Vec<ModEvent>>,
}

impl ModInput {
    fn claim(&mut self, mod_id: &str, ids: &[String]) {
        let ids = ids.iter().take(CLAIMS_MAX).cloned().collect();
        self.claims.insert(mod_id.to_owned(), ids);
    }
    /// The mod that claims `item`, the first in id order.
    fn claimant(&self, item: &str) -> Option<&str> {
        self.claims
            .iter()
            .find(|(_, ids)| ids.iter().any(|i| i == item))
            .map(|(id, _)| id.as_str())
    }
    fn push(&mut self, mod_id: &str, event: ModEvent) {
        let events = self.events.entry(mod_id.to_owned()).or_default();
        if events.len() < EVENTS_MAX {
            events.push(event);
        }
    }
    fn poll(&mut self, mod_id: &str) -> Vec<ModEvent> {
        self.events.remove(mod_id).unwrap_or_default()
    }
}

impl Game<'_> {
    /// The stack in inventory slot `slot` as a mod's events carry it.
    fn mod_held(stack: &sunrise_client::items::ItemStack, slot: usize) -> Option<HeldItemSlot> {
        let extra = stack.extra.as_deref();
        Some(HeldItemSlot {
            id: mod_api::__private::item_key(&stack.item)?,
            count: stack.count,
            slot: slot as u32,
            custom_name: extra
                .and_then(|e| e.custom_name.as_ref())
                .map(sunrise_client::snbt::to_snbt),
            custom_data: extra
                .and_then(|e| e.other("minecraft:custom_data"))
                .map(sunrise_client::snbt::to_snbt),
        })
    }
    /// The first block along the view within `distance` (at most 1024), in loaded chunks only; `None` on a miss.
    pub(super) fn raycast(&self, distance: f32) -> Option<mod_api::Hit> {
        let (d, hit) = self.view_hit()?;
        (d <= distance.clamp(0.0, RAYCAST_MAX)).then_some(hit)
    }
    /// The first block along the view within `RAYCAST_MAX`, in loaded chunks only, and its distance; `None` on a miss.
    pub(super) fn view_hit(&self) -> Option<(f32, mod_api::Hit)> {
        self.player.as_ref()?;
        let eye = self.eye_position()?;
        let dir = super::view_vector(self.yaw, self.pitch);
        let reach = f64::from(RAYCAST_MAX);
        let hit = self.clip_ray(eye, dir, reach)?;
        let (pos, point) = (hit.pos, hit.point);
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let distance = (0..3)
            .map(|i| (point[i] - eye[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        Some((
            distance as f32,
            mod_api::Hit {
                block_x: pos[0],
                block_y: pos[1],
                block_z: pos[2],
                x: point[0],
                y: point[1],
                z: point[2],
                block: mod_api::__private::block_key(&self.res.facts().get(state).name)?,
            },
        ))
    }
    /// Right-clicks with these items (at most 64 ids) are mod `mod_id`'s; a later call replaces the list.
    pub(super) fn claim_items(&mut self, mod_id: &str, ids: &[String]) {
        self.native_input.claim(mod_id, ids);
    }
    /// Mod `mod_id`'s events since its last poll (at most 256 wait).
    pub(super) fn poll(&mut self, mod_id: &str) -> Vec<ModEvent> {
        self.native_input.poll(mod_id)
    }
    fn can_capture_item_use(&self) -> bool {
        self.using.is_none()
            && !self.spectator()
            && self.screen != Screen::Dead
            && self
                .player
                .as_ref()
                .is_some_and(|p| p.health.health() > 0.0)
            && self
                .remote
                .as_ref()
                .is_some_and(|r| r.logged_in && r.session.connected())
    }

    /// Give the claimed main-hand stack's owner this click instead of using a target or item.
    pub(in crate::game) fn start_client_item_use(&mut self) -> bool {
        let slot = self.selected;
        let Some(stack) = self.stack_in(slot) else {
            return false;
        };
        let Some(mod_id) = self.native_input.claimant(&stack.item) else {
            return false;
        };
        if !self.can_capture_item_use() {
            return false;
        }
        let Some(original) = Self::mod_held(stack, slot) else {
            return false;
        };
        let active = ActiveClientItemUse {
            mod_id: mod_id.to_owned(),
            original,
            item_id: Arc::clone(&stack.item),
            slot,
            ticks: 0,
        };
        self.native_input.push(
            &active.mod_id,
            ModEvent::UseStart(active.original.clone(), 0),
        );
        self.client_item_use = Some(active);
        true
    }

    /// Reset an invalid capture or count a held tick. Only input handling releases it.
    pub(in crate::game) fn tick_client_item_use(&mut self) {
        let Some(active) = self.client_item_use.as_ref() else {
            return;
        };
        if !self.can_capture_item_use()
            || active.slot != self.selected
            || !self
                .stack_in(active.slot)
                .is_some_and(|s| s.item == active.item_id)
        {
            self.reset_client_item_use();
        } else if self.right
            && let Some(active) = self.client_item_use.as_mut()
        {
            active.ticks = active.ticks.saturating_add(1);
        }
    }

    /// Retire the capture first. Both release and reset still report the legacy `UseStop`.
    pub(in crate::game) fn end_client_item_use(&mut self) -> bool {
        let Some(active) = self.client_item_use.take() else {
            return false;
        };
        self.native_input.push(
            &active.mod_id,
            ModEvent::UseStop(active.original, active.ticks),
        );
        true
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;

    fn keys() {
        mod_platform::__private::part_tests::install_parts_fixture(&[]).unwrap();
        mod_api::__private::resolve_items(["minecraft:stick"]).unwrap();
    }

    fn stick(slot: u32) -> HeldItemSlot {
        HeldItemSlot {
            id: mod_api::__private::item_key("minecraft:stick").unwrap(),
            count: 1,
            slot,
            custom_name: None,
            custom_data: Some("{orbital:1b}".into()),
        }
    }

    #[test]
    fn items_are_claimed_by_the_first_mod_in_id_order() {
        let mut input = ModInput::default();
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        input.claim("zeta", &ids(&["orbital:remote", "minecraft:stick"]));
        input.claim("alpha", &ids(&["minecraft:stick"]));
        assert_eq!(input.claimant("minecraft:stick"), Some("alpha"));
        assert_eq!(input.claimant("orbital:remote"), Some("zeta"));
        assert_eq!(input.claimant("minecraft:dirt"), None);
        input.claim("alpha", &[]);
        assert_eq!(input.claimant("minecraft:stick"), Some("zeta"));
        let many: Vec<String> = (0..100).map(|i| format!("m:item{i}")).collect();
        input.claim("alpha", &many);
        assert_eq!(input.claimant("m:item63"), Some("alpha"));
        assert_eq!(input.claimant("m:item64"), None, "64 at most");
    }

    #[test]
    fn events_wait_for_their_mod_up_to_a_limit() {
        keys();
        let mut input = ModInput::default();
        for _ in 0..EVENTS_MAX + 10 {
            input.push("a", ModEvent::UseStart(stick(0), 0));
        }
        input.push("b", ModEvent::UseStop(stick(0), 3));
        assert_eq!(input.poll("a").len(), EVENTS_MAX);
        assert!(input.poll("a").is_empty());
        assert_eq!(input.poll("b"), vec![ModEvent::UseStop(stick(0), 3)]);
    }
}
