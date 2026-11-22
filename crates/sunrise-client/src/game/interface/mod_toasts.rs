//! Client-host notifications use the ordinary system-toast update and slots.
#[cfg(test)]
#[path = "mod_toasts/tests.rs"]
mod tests;
use super::Game;
use mod_api::{Identifier, ModToast, TextFormError, ToastIcon, ToastKind};
use std::cell::RefCell;
use sunrise_client::{
    hud::HudBatch,
    items::ItemStack,
    toasts::{SystemToast, SystemToastLayout},
};
use sunrise_client_lib::api_text::to_engine;

#[derive(Default)]
pub(super) struct ModToasts {
    pending: RefCell<Vec<Pending>>,
    queued: Vec<Entry>,
    visible: Vec<Entry>,
}

struct Pending {
    toast: SystemToast,
    kind: Option<Identifier>,
    icon: Option<Icon>,
}

struct Entry {
    request: Pending,
    layout: SystemToastLayout,
    // ToastInstance keeps the slots it took when it first became visible,
    // even if addOrUpdate changes the toast's dimensions.
    slots: u32,
}

enum Icon {
    Item(ItemStack),
    Sprite(String),
}

impl ModToasts {
    pub(super) fn slots(&self) -> u32 {
        self.visible
            .iter()
            .fold(0, |used, entry| used | entry.slots)
    }
}

impl Game<'_> {
    /// Queue from a shared ClientHost without dispatching back into a running hook.
    pub(in crate::game) fn add_toast(&self, request: ModToast) -> Result<(), TextFormError> {
        let title = to_engine(&request.title)?;
        let description = request.description.as_ref().map(to_engine).transpose()?;
        let mut toast = SystemToast::new(title, description);
        if let Some(ms) = request.display_ms {
            toast = toast.with_display_time(ms as f64);
        }
        let icon = match request.icon {
            Some(ToastIcon::Item(stack)) => {
                super::api_values::stack_value(&stack).ok().map(Icon::Item)
            }
            Some(ToastIcon::Sprite(sprite)) => Some(Icon::Sprite(sprite.to_string())),
            None | Some(_) => None,
        };
        self.mod_toasts.pending.borrow_mut().push(Pending {
            toast,
            kind: request.kind,
            icon,
        });
        Ok(())
    }

    pub(super) fn admit_mod_toasts(&mut self) {
        // Take one snapshot before invoking listeners. A request from toast_add is
        // for the next update, not a recursive add or part of this snapshot.
        for mut request in std::mem::take(self.mod_toasts.pending.get_mut()) {
            if !self.admit_toast(
                ToastKind::Mod,
                &mut request.toast.title,
                &mut request.toast.message,
            ) {
                continue;
            }
            let layout = request
                .toast
                .layout(&self.gui.atlas, request.icon.is_some());
            if let Some(kind) = &request.kind
                && let Some(entry) = self
                    .mod_toasts
                    .visible
                    .iter_mut()
                    .chain(&mut self.mod_toasts.queued)
                    .find(|entry| entry.request.kind.as_ref() == Some(kind))
            {
                entry.request.toast.reset_from(request.toast);
                entry.request.icon = request.icon;
                entry.layout = layout;
            } else {
                self.mod_toasts.queued.push(Entry {
                    request,
                    layout,
                    slots: 0,
                });
            }
        }
    }

    /// `ToastManager.findFreeSlotsIndex`: the first consecutive run that fits.
    pub(super) fn free_toast_slot(used: u32, count: usize) -> Option<usize> {
        let last = Self::TOAST_SLOTS.checked_sub(count)?;
        let mask = (1 << count) - 1;
        (0..=last).find(|slot| used & (mask << slot) == 0)
    }

    pub(super) fn draw_mod_toasts(&mut self, batch: &mut HudBatch, hidden: bool) {
        if self.mod_toasts.queued.is_empty() && self.mod_toasts.visible.is_empty() {
            return;
        }
        let multiplier = self.settings.notification_display_time;
        let mut sound_played = false;
        self.mod_toasts.visible.retain_mut(|entry| {
            let (sound, finished) = entry.request.toast.update(multiplier);
            if let Some(show) = sound
                && !std::mem::replace(&mut sound_played, true)
            {
                let event = if show {
                    "minecraft:ui.toast.in"
                } else {
                    "minecraft:ui.toast.out"
                };
                self.audio.play(event, None, 1.0, 1.0);
            }
            !finished
        });

        let title_has_toast = self.title.as_ref().is_some_and(|title| title.has_toast());
        let mut used = self.toast_slots_used() | u32::from(title_has_toast);
        if used.count_ones() < Self::TOAST_SLOTS as u32 {
            let started = self.mod_toasts.queued.extract_if(.., |entry| {
                let count = entry.layout.slots();
                let Some(slot) = Self::free_toast_slot(used, count) else {
                    return false;
                };
                entry.request.toast.slot = Some(slot);
                entry.slots = ((1 << count) - 1) << slot;
                used |= entry.slots;
                true
            });
            self.mod_toasts.visible.extend(started);
        }
        if hidden || self.ui.hide_gui {
            return;
        }
        let atlas = &self.gui.atlas;
        for entry in &self.mod_toasts.visible {
            let Some(slot) = entry.request.toast.slot else {
                continue;
            };
            let [x, y] = entry
                .request
                .toast
                .draw_wrapped(&entry.layout, atlas, batch, slot);
            // AdvancementToast's icon: 16 pixels square at (8, 8).
            let (x, y) = (x + 8.0, y + 8.0);
            match &entry.request.icon {
                Some(Icon::Item(stack)) => {
                    self.draw_item_model_with_owner(batch, stack, x, y, None)
                }
                Some(Icon::Sprite(id)) => {
                    batch.sprite_scaled(atlas, id, x, y, 16.0, 16.0, u32::MAX)
                }
                None => {}
            }
        }
    }
}
