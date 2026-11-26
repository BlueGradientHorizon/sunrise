
use super::{mod_toasts, tutorial};
use crate::game::client_side::advancement_toast;
use std::{cell::RefCell, collections::VecDeque};
use sunrise_client::toasts::SystemToast;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SystemToastId {
    Narrator,
    PackLoadFailure,
    PackDownload(usize),
    ChunkIo(usize),
    Title,
}

pub enum Toast {
    Advancement(advancement_toast::Toast),
    Tutorial(tutorial::HintToast),
    System(Option<SystemToastId>, SystemToast),
    Diagnostic {
        toast: SystemToast,
        hide_with_f1: bool,
    },
    Mod(mod_toasts::Entry),
}

pub struct VisibleToast {
    pub(in crate::game) toast: Toast,
    pub(in crate::game) slot: usize,
    // A reset can resize the content, but the instance keeps its original slots.
    pub(in crate::game) slots: u32,
}

#[derive(Default)]
pub(in crate::game) struct ToastManager {
    pub(in crate::game) pending_mod: RefCell<Vec<mod_toasts::Pending>>,
    pub(in crate::game) queued: VecDeque<Toast>,
    pub(in crate::game) visible: Vec<VisibleToast>,
}

impl ToastManager {
    pub(in crate::game) fn all_mut(&mut self) -> impl Iterator<Item = &mut Toast> {
        self.visible
            .iter_mut()
            .map(|entry| &mut entry.toast)
            .chain(&mut self.queued)
    }

    /// `ToastManager.getToast` looks in the visible list before the queue.
    pub(in crate::game) fn find_mut<T>(
        &mut self,
        get: impl FnMut(&mut Toast) -> Option<&mut T>,
    ) -> Option<&mut T> {
        self.all_mut().find_map(get)
    }
}