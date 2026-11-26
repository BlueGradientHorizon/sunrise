//! A mod's context menu over the open screen (`ClientHost::open_context_menu`): a list of
//! rows, separators and nested menus drawn above everything else on the screen, taking the
//! mouse and keyboard until it closes. Vanilla and NeoForge have no such widget.
//!
//! The menu belongs to the screen instance it opened on and closes with it. Its entries hold
//! the mod's closures, so they are dropped and run under the mod's [`CallbackOwner`]: a panic
//! in one turns that mod off instead of taking the game down.

use super::Game;
use crate::game::{api_values::TryIntoEngine, host::draw::GameDrawContext};
use mod_api::{__private::game::CallbackOwner, DrawContext, client::widgets::ContextMenuEntry};
use sunrise_client::hud::HudBatch;
use winit::keyboard::KeyCode;

/// Height of a row without an icon, in GUI pixels.
const ROW: f32 = 12.0;
/// Height of a row in a level where some row has an icon.
const ICON_ROW: f32 = 18.0;
/// Height of a separator.
const SEPARATOR: f32 = 5.0;
/// Space inside the border around the rows and before each label.
const PAD: f32 = 3.0;
/// Room for a nested menu's arrow at the right of its row.
const ARROW: f32 = 10.0;

const BACKGROUND: u32 = 0xF010_0010;
const BORDER: u32 = 0xFF50_00A0;
const HOVER: u32 = 0xFF40_40A0;
const TEXT: u32 = 0xFFFF_FFFF;
const DISABLED: u32 = 0xFF80_8080;
const LINE: u32 = 0xFF50_5050;

/// An open context menu.
pub(super) struct ContextMenu {
    owner: CallbackOwner,
    /// The screen instance it opened on (`ScreenState::screen_instance`).
    instance: u64,
    at: [f32; 2],
    root: Vec<ContextMenuEntry>,
    /// One entry per open level: the row selected in it (by hover or the arrow keys). Level
    /// `k + 1` is the nested menu of level `k`'s selected row.
    selected: Vec<Option<usize>>,
}

impl Drop for ContextMenu {
    fn drop(&mut self) {
        let root = std::mem::take(&mut self.root);
        self.owner
            .dispose("context menu", "context_menu_drop", root);
    }
}

/// Where one open level and its rows are, in GUI pixels (`[x, y, w, h]`).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct LevelBox {
    pub(super) rect: [f32; 4],
    pub(super) rows: Vec<[f32; 4]>,
}

fn label_of(entry: &ContextMenuEntry) -> Option<String> {
    match entry {
        ContextMenuEntry::Action { label, .. } | ContextMenuEntry::Submenu { label, .. } => {
            let text: sunrise_client::text::Text = label.try_into_engine(&()).unwrap_or_default();
            Some(text.plain())
        }
        // A separator, or a row kind a later API adds that this game can't draw.
        _ => None,
    }
}

fn icon_of(entry: &ContextMenuEntry) -> Option<&mod_api::ItemStack> {
    match entry {
        ContextMenuEntry::Action { icon, .. } | ContextMenuEntry::Submenu { icon, .. } => {
            icon.as_ref()
        }
        _ => None,
    }
}

fn enabled(entry: &ContextMenuEntry) -> bool {
    match entry {
        ContextMenuEntry::Action { enabled, .. } | ContextMenuEntry::Submenu { enabled, .. } => {
            *enabled
        }
        _ => false,
    }
}

fn inside([mx, my]: [f32; 2], [x, y, w, h]: [f32; 4]) -> bool {
    mx >= x && my >= y && mx < x + w && my < y + h
}

impl ContextMenu {
    /// The rows of open level `k`.
    fn level(&self, k: usize) -> &[ContextMenuEntry] {
        let mut entries = self.root.as_slice();
        for row in self.selected.iter().take(k) {
            match row.and_then(|row| entries.get(row)) {
                Some(ContextMenuEntry::Submenu {
                    entries: nested, ..
                }) => entries = nested,
                _ => return &[],
            }
        }
        entries
    }

    fn level_mut(&mut self, k: usize) -> Option<&mut Vec<ContextMenuEntry>> {
        let mut entries = &mut self.root;
        for row in self.selected.iter().take(k) {
            match entries.get_mut((*row)?)? {
                ContextMenuEntry::Submenu {
                    entries: nested, ..
                } => entries = nested,
                _ => return None,
            }
        }
        Some(entries)
    }

    /// Each open level's box, kept inside a `size` window: a level that would cross the right
    /// or bottom edge moves left or up, and a nested level opens on its parent's left when
    /// its right has no room.
    pub(super) fn layout(&self, size: [f32; 2], width: &dyn Fn(&str) -> f32) -> Vec<LevelBox> {
        let mut boxes: Vec<LevelBox> = Vec::new();
        for k in 0..self.selected.len() {
            let entries = self.level(k);
            let icons = entries.iter().any(|entry| icon_of(entry).is_some());
            let nested = entries
                .iter()
                .any(|entry| matches!(entry, ContextMenuEntry::Submenu { .. }));
            let row = if icons { ICON_ROW } else { ROW };
            let label = entries
                .iter()
                .filter_map(label_of)
                .map(|label| width(&label))
                .fold(0.0, f32::max);
            let w = PAD * 2.0
                + label
                + if icons { ICON_ROW } else { 0.0 }
                + if nested { ARROW } else { 0.0 };
            let h = PAD * 2.0
                + entries
                    .iter()
                    .map(|entry| match entry {
                        ContextMenuEntry::Separator => SEPARATOR,
                        _ => row,
                    })
                    .sum::<f32>();
            let [mut x, mut y] = match boxes.last() {
                None => self.at,
                Some(parent) => {
                    let opener = self.selected[k - 1]
                        .and_then(|row| parent.rows.get(row))
                        .copied()
                        .unwrap_or(parent.rect);
                    let right = parent.rect[0] + parent.rect[2];
                    let x = if right + w <= size[0] {
                        right
                    } else {
                        parent.rect[0] - w
                    };
                    [x, opener[1] - PAD]
                }
            };
            x = x.min(size[0] - w).max(0.0);
            y = y.min(size[1] - h).max(0.0);
            let mut rows = Vec::with_capacity(entries.len());
            let mut top = y + PAD;
            for entry in entries {
                let height = match entry {
                    ContextMenuEntry::Separator => SEPARATOR,
                    _ => row,
                };
                rows.push([x + 1.0, top, w - 2.0, height]);
                top += height;
            }
            boxes.push(LevelBox {
                rect: [x, y, w, h],
                rows,
            });
        }
        boxes
    }

    /// The deepest level and row under `mouse`.
    fn hit(boxes: &[LevelBox], mouse: [f32; 2]) -> Option<(usize, Option<usize>)> {
        boxes.iter().enumerate().rev().find_map(|(k, level)| {
            inside(mouse, level.rect)
                .then(|| (k, level.rows.iter().position(|row| inside(mouse, *row))))
        })
    }

    /// Selects `row` of level `k`, closing deeper levels, and opens its nested menu when it
    /// is an enabled one.
    fn select(&mut self, k: usize, row: Option<usize>, open_nested: bool) {
        self.selected.truncate(k + 1);
        self.selected[k] = row;
        let opens = row
            .and_then(|row| self.level(k).get(row))
            .is_some_and(|entry| {
                enabled(entry)
                    && matches!(entry, ContextMenuEntry::Submenu { entries, .. } if !entries.is_empty())
            });
        if opens && open_nested {
            self.selected.push(None);
        }
    }

    /// The next selectable row of the deepest level after (or before, `step < 0`) the
    /// selected one, wrapping around.
    fn step(&mut self, step: isize) {
        let k = self.selected.len() - 1;
        let entries = self.level(k);
        let len = entries.len() as isize;
        if len == 0 {
            return;
        }
        let mut at = self.selected[k].map_or(if step > 0 { -1 } else { len }, |r| r as isize);
        for _ in 0..len {
            at = (at + step).rem_euclid(len);
            if !matches!(entries[at as usize], ContextMenuEntry::Separator) {
                self.selected[k] = Some(at as usize);
                self.selected.truncate(k + 1);
                return;
            }
        }
    }
}

/// What a press or key asked of the menu.
enum Outcome {
    Stay,
    Close,
    Pick(usize, usize),
}

impl Game<'_> {
    /// `ClientHost::open_context_menu`: false (nothing opened) with no screen open, an empty
    /// list, or outside a mod's call.
    pub(in crate::game) fn open_context_menu(
        &mut self,
        at: [f32; 2],
        entries: Vec<ContextMenuEntry>,
    ) -> bool {
        let Some(id) = mod_api::content::crash::current_native_mod() else {
            return false;
        };
        let owner = CallbackOwner::for_mod(id);
        if entries.is_empty()
            || self.observed_screen_view().is_none()
            || !at.iter().all(|v| v.is_finite())
        {
            owner.dispose("context menu", "context_menu_drop", entries);
            return false;
        }
        self.screen_state.context_menu = Some(ContextMenu {
            owner,
            instance: self.screen_state.screen_instance,
            at,
            root: entries,
            selected: vec![None],
        });
        true
    }

    /// `ClientHost::close_context_menu`: false when none is open.
    pub(in crate::game) fn close_context_menu(&mut self) -> bool {
        self.live_context_menu();
        self.screen_state.context_menu.take().is_some()
    }

    /// Whether a menu is open over the current screen (dropping one whose screen closed).
    pub(super) fn context_menu_open(&mut self) -> bool {
        self.live_context_menu()
    }

    /// Drops a menu whose screen closed or was replaced; whether one is still open.
    fn live_context_menu(&mut self) -> bool {
        let instance = self.screen_state.screen_instance;
        if self
            .screen_state
            .context_menu
            .as_ref()
            .is_some_and(|menu| menu.instance != instance || self.observed_screen_view().is_none())
        {
            self.screen_state.context_menu = None;
        }
        self.screen_state.context_menu.is_some()
    }

    fn context_menu_layout(&self) -> Vec<LevelBox> {
        let Some(menu) = &self.screen_state.context_menu else {
            return Vec::new();
        };
        let (w, h) = self.gui_size();
        let atlas = &self.gui.atlas;
        menu.layout([w, h], &|text| HudBatch::text_width(atlas, text))
    }

    /// A mouse button while a menu is open: a left press on a row picks it (or opens its
    /// nested menu), a press outside every level closes the menu, and every press and release
    /// is the menu's. False when no menu is open.
    pub(super) fn context_menu_mouse(&mut self, pressed: bool, left: bool) -> bool {
        if !self.live_context_menu() {
            return false;
        }
        if !pressed {
            return true;
        }
        let boxes = self.context_menu_layout();
        let mouse = self.cursor();
        let Some(menu) = self.screen_state.context_menu.as_mut() else {
            return true;
        };
        let outcome = match ContextMenu::hit(&boxes, mouse) {
            None => Outcome::Close,
            Some((_, None)) => Outcome::Stay,
            Some((k, Some(row))) => {
                let entry = &menu.level(k)[row];
                if !left || !enabled(entry) {
                    Outcome::Stay
                } else if matches!(entry, ContextMenuEntry::Action { .. }) {
                    Outcome::Pick(k, row)
                } else {
                    menu.select(k, Some(row), true);
                    Outcome::Stay
                }
            }
        };
        self.finish_context_menu(outcome);
        true
    }

    /// The pointer moved while a menu is open: the row under it is selected, and an enabled
    /// nested menu under it opens.
    pub(super) fn context_menu_hover(&mut self) {
        if !self.live_context_menu() {
            return;
        }
        let boxes = self.context_menu_layout();
        let mouse = self.cursor();
        if let Some(menu) = self.screen_state.context_menu.as_mut()
            && let Some((k, Some(row))) = ContextMenu::hit(&boxes, mouse)
            && menu.selected.get(k) != Some(&Some(row))
        {
            menu.select(k, Some(row), true);
        }
    }

    /// A key while a menu is open: Escape closes it, Up and Down move between rows, Right or
    /// Enter opens a nested menu, Left closes one, Enter picks an action. Every key is the
    /// menu's. False when no menu is open.
    pub(super) fn context_menu_key(&mut self, key: KeyCode) -> bool {
        if !self.live_context_menu() {
            return false;
        }
        let Some(menu) = self.screen_state.context_menu.as_mut() else {
            return true;
        };
        let k = menu.selected.len() - 1;
        let outcome = match key {
            KeyCode::Escape => Outcome::Close,
            KeyCode::ArrowUp => {
                menu.step(-1);
                Outcome::Stay
            }
            KeyCode::ArrowDown => {
                menu.step(1);
                Outcome::Stay
            }
            KeyCode::ArrowLeft => {
                if k > 0 {
                    menu.selected.truncate(k);
                }
                Outcome::Stay
            }
            KeyCode::ArrowRight | KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => {
                let chosen = menu.selected[k].map(|row| {
                    let entry = &menu.level(k)[row];
                    let action = matches!(entry, ContextMenuEntry::Action { .. });
                    (row, action, enabled(entry))
                });
                match chosen {
                    Some((row, true, true)) if key != KeyCode::ArrowRight => Outcome::Pick(k, row),
                    Some((row, false, _)) => {
                        menu.select(k, Some(row), true);
                        if menu.selected.len() > k + 1 {
                            menu.step(1);
                        }
                        Outcome::Stay
                    }
                    _ => Outcome::Stay,
                }
            }
            _ => Outcome::Stay,
        };
        self.finish_context_menu(outcome);
        true
    }

    fn finish_context_menu(&mut self, outcome: Outcome) {
        let (k, row) = match outcome {
            Outcome::Stay => return,
            Outcome::Close => {
                self.screen_state.context_menu = None;
                return;
            }
            Outcome::Pick(k, row) => (k, row),
        };
        // The menu closes first, then the action runs: it may open another menu or screen.
        let Some(mut menu) = self.screen_state.context_menu.take() else {
            return;
        };
        let picked = menu
            .level_mut(k)
            .and_then(|entries| entries.get_mut(row))
            .map(|entry| std::mem::replace(entry, ContextMenuEntry::Separator));
        let owner = menu.owner.clone();
        drop(menu);
        let Some(ContextMenuEntry::Action { on_select, .. }) = picked else {
            return;
        };
        if owner.is_enabled() {
            let _ = owner.call("context menu", "context_menu_select", || on_select(self));
        } else {
            owner.dispose("context menu", "context_menu_drop", on_select);
        }
    }

    /// Draws the open menu above everything else on the screen (after its tooltips and the
    /// screen's post-render hook).
    pub(super) fn draw_context_menu(&self, batch: &mut HudBatch) {
        let Some(menu) = &self.screen_state.context_menu else {
            return;
        };
        if menu.instance != self.screen_state.screen_instance {
            return;
        }
        let boxes = self.context_menu_layout();
        let mouse = self.cursor();
        let hovered = ContextMenu::hit(&boxes, mouse);
        let mut draw = GameDrawContext::new(self, batch);
        for (k, level) in boxes.iter().enumerate() {
            let [x, y, w, h] = level.rect;
            draw.fill([x, y, w, h], BORDER);
            draw.fill([x + 1.0, y + 1.0, w - 2.0, h - 2.0], BACKGROUND);
            let entries = menu.level(k);
            let icons = entries.iter().any(|entry| icon_of(entry).is_some());
            for (row, (entry, rect)) in entries.iter().zip(&level.rows).enumerate() {
                let [rx, ry, rw, rh] = *rect;
                if matches!(entry, ContextMenuEntry::Separator) {
                    draw.fill([rx + PAD, ry + rh / 2.0 - 0.5, rw - PAD * 2.0, 1.0], LINE);
                    continue;
                }
                let on = enabled(entry);
                let lit =
                    hovered == Some((k, Some(row))) || menu.selected.get(k) == Some(&Some(row));
                if on && lit {
                    draw.fill(*rect, HOVER);
                }
                let mut text_x = rx + PAD - 1.0;
                if icons {
                    if let Some(icon) = icon_of(entry) {
                        draw.item_stack(icon, [text_x, ry + 1.0], 16.0);
                    }
                    text_x += ICON_ROW;
                }
                let color = if on { TEXT } else { DISABLED };
                let text_y = ry + (rh - 8.0) / 2.0;
                if let Some(label) = label_of(entry) {
                    draw.text(&label, [text_x, text_y], color, true);
                }
                if matches!(entry, ContextMenuEntry::Submenu { .. }) {
                    draw.text(">", [rx + rw - ARROW + 2.0, text_y], color, true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop() -> impl FnOnce(&mut dyn mod_api::ClientHost) + Send + 'static {
        |_| {}
    }

    fn menu(root: Vec<ContextMenuEntry>) -> ContextMenu {
        ContextMenu {
            owner: CallbackOwner::for_mod("test_mod"),
            instance: 1,
            at: [10.0, 10.0],
            root,
            selected: vec![None],
        }
    }

    fn width(text: &str) -> f32 {
        text.len() as f32 * 6.0
    }

    #[test]
    fn levels_stay_inside_the_window_and_nest_beside_their_row() {
        let mut m = menu(vec![
            ContextMenuEntry::action(mod_api::Text::literal("Copy"), noop()),
            ContextMenuEntry::separator(),
            ContextMenuEntry::submenu(
                mod_api::Text::literal("More"),
                vec![ContextMenuEntry::action(
                    mod_api::Text::literal("Paste"),
                    noop(),
                )],
            ),
        ]);
        let boxes = m.layout([200.0, 100.0], &width);
        assert_eq!(boxes.len(), 1);
        // "Copy" and "More" are 24 wide, plus padding and the nested arrow.
        assert_eq!(boxes[0].rect, [10.0, 10.0, 40.0, 35.0]);
        assert_eq!(boxes[0].rows[1][3], SEPARATOR);

        m.select(0, Some(2), true);
        let boxes = m.layout([200.0, 100.0], &width);
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes[1].rect[0], 50.0, "opens at its parent's right edge");
        assert_eq!(boxes[1].rect[1], boxes[0].rows[2][1] - PAD);

        // Near the bottom right corner both levels move back inside.
        m.at = [190.0, 95.0];
        let boxes = m.layout([200.0, 100.0], &width);
        for level in &boxes {
            let [x, y, w, h] = level.rect;
            assert!(
                x >= 0.0 && y >= 0.0 && x + w <= 200.0 && y + h <= 100.0,
                "{level:?}"
            );
        }
        assert!(
            boxes[1].rect[0] < boxes[0].rect[0],
            "nests on the left with no room right"
        );
    }

    #[test]
    fn the_arrow_keys_skip_separators_and_wrap() {
        let mut m = menu(vec![
            ContextMenuEntry::action(mod_api::Text::literal("A"), noop()),
            ContextMenuEntry::separator(),
            ContextMenuEntry::action(mod_api::Text::literal("B"), noop()).enabled(false),
        ]);
        m.step(1);
        assert_eq!(m.selected, vec![Some(0)]);
        m.step(1);
        assert_eq!(m.selected, vec![Some(2)]);
        m.step(1);
        assert_eq!(m.selected, vec![Some(0)]);
        m.step(-1);
        assert_eq!(m.selected, vec![Some(2)]);
    }

    #[test]
    fn a_disabled_or_empty_nested_menu_does_not_open() {
        let mut m = menu(vec![
            ContextMenuEntry::submenu(
                mod_api::Text::literal("Off"),
                vec![ContextMenuEntry::action(
                    mod_api::Text::literal("X"),
                    noop(),
                )],
            )
            .enabled(false),
            ContextMenuEntry::submenu(mod_api::Text::literal("Empty"), Vec::new()),
            ContextMenuEntry::submenu(
                mod_api::Text::literal("On"),
                vec![ContextMenuEntry::action(
                    mod_api::Text::literal("Y"),
                    noop(),
                )],
            ),
        ]);
        m.select(0, Some(0), true);
        assert_eq!(m.selected.len(), 1);
        m.select(0, Some(1), true);
        assert_eq!(m.selected.len(), 1);
        m.select(0, Some(2), true);
        assert_eq!(m.selected.len(), 2);
        assert_eq!(m.level(1).len(), 1);
        // Selecting another row closes the nested level.
        m.select(0, Some(0), true);
        assert_eq!(m.selected, vec![Some(0)]);
    }

    #[test]
    fn hits_find_the_deepest_level_first() {
        let boxes = vec![
            LevelBox {
                rect: [0.0, 0.0, 50.0, 50.0],
                rows: vec![[1.0, 3.0, 48.0, 12.0]],
            },
            LevelBox {
                rect: [40.0, 0.0, 50.0, 50.0],
                rows: vec![[41.0, 3.0, 48.0, 12.0]],
            },
        ];
        assert_eq!(ContextMenu::hit(&boxes, [45.0, 5.0]), Some((1, Some(0))));
        assert_eq!(ContextMenu::hit(&boxes, [5.0, 5.0]), Some((0, Some(0))));
        assert_eq!(ContextMenu::hit(&boxes, [5.0, 40.0]), Some((0, None)));
        assert_eq!(ContextMenu::hit(&boxes, [95.0, 5.0]), None);
    }
}
