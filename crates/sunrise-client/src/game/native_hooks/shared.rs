#[path = "shared/book_screen.rs"]
pub(in crate::game) mod book_screen;
#[cfg(test)]
#[path = "shared/item_fall_on_tests.rs"]
pub(in crate::game) mod item_fall_on_tests;
#[path = "shared/level_helpers.rs"]
pub(in crate::game) mod level_helpers;
#[cfg(test)]
#[path = "shared/particle_count_tests.rs"]
pub(in crate::game) mod particle_count_tests;
#[allow(unused_imports)]
#[cfg(test)]
use level_helpers::block_entity_for_block;
#[cfg(test)]
#[path = "shared/audio_math_tests.rs"]
pub(in crate::game) mod audio_math_tests;
#[cfg(test)]
#[path = "shared/block_property_tests.rs"]
pub(in crate::game) mod block_property_tests;
#[cfg(test)]
#[path = "shared/gameplay_hook_tests.rs"]
pub(in crate::game) mod gameplay_hook_tests;
#[path = "shared/item_fallthrough.rs"]
pub(in crate::game) mod item_fallthrough;
#[cfg(test)]
#[path = "shared/label_tests.rs"]
pub(in crate::game) mod label_tests;
#[cfg(test)]
#[path = "shared/live_damage_tests.rs"]
pub(in crate::game) mod live_damage_tests;
#[path = "shared/loot.rs"]
pub(in crate::game) mod loot;
#[cfg(test)]
#[path = "shared/packet_tests.rs"]
pub(in crate::game) mod packet_tests;
#[cfg(test)]
#[path = "shared/runtime_environment_tests.rs"]
pub(in crate::game) mod runtime_environment_tests;
#[cfg(test)]
#[path = "shared/screen_position_tests.rs"]
pub(in crate::game) mod screen_position_tests;
#[cfg(test)]
#[path = "shared/slot_icon_tests.rs"]
pub(in crate::game) mod slot_icon_tests;
#[cfg(test)]
#[path = "shared/slot_tooltip_tests.rs"]
pub(in crate::game) mod slot_tooltip_tests;
#[path = "shared/system_helpers.rs"]
pub(in crate::game) mod system_helpers;
#[path = "shared/world_helpers.rs"]
pub(in crate::game) mod world_helpers;

#[allow(unused_imports)]
use super::screen_build::ObservedScreen;
#[allow(unused_imports)]
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
#[allow(unused_imports)]
use screen_chars::chars_to_type;
#[allow(unused_imports)]
use sunrise_client::containers::ModBlockEntity as EngineBlockEntity;

pub(in crate::game) struct ContentsChangedGuard<'a, 'game> {
    pub(in crate::game) game: &'a mut Game<'game>,
}

/// Write back a `menu_button` cursor edit without revalidating an untouched stack.
pub(in crate::game) fn apply_menu_button_cursor(
    items: &sunrise_client::items::Items,
    original: &Option<mod_api::ItemStack>,
    cursor: &Option<mod_api::ItemStack>,
    carried: &mut Option<ItemStack>,
) {
    if cursor == original {
        return;
    }
    match cursor.try_into_engine(items) {
        Ok(updated) => *carried = updated,
        Err(error) => {
            log::warn!("mod menu_button write refused: {error:?}");
        }
    }
}

#[path = "shared/screen_view.rs"]
pub(in crate::game) mod screen_view;
#[allow(unused_imports)]
use screen_view::{screen_view_for_mod, screen_view_with_id};

/// What `decide_screen` says the hooks answered to a change of screen.
pub(in crate::game) enum ScreenDecision {
    /// The change goes ahead.
    Open,
    /// `Cancel`: the current screen stays.
    Keep,
    /// `ModScreen`: this mod's own screen opens instead.
    ModScreen(&'static str),
    /// `Vanilla`: this vanilla screen opens instead.
    Vanilla(ScreenTarget),
}

/// The title screen a `ScreenOpening::Vanilla` target is (its id), or None
/// for a world's screen.
pub(in crate::game) fn title_target_id(target: ScreenTarget) -> Option<&'static str> {
    match target {
        ScreenTarget::Title => Some("title"),
        ScreenTarget::WorldSelect => Some("world_select"),
        ScreenTarget::WorldCreate => Some("world_create"),
        ScreenTarget::Multiplayer => Some("multiplayer"),
        ScreenTarget::AddServer => Some("add_server"),
        ScreenTarget::DirectConnect => Some("direct_connect"),
        _ => None,
    }
}

pub(in crate::game) fn observed_overlay_id(
    overlays: &[(bool, &'static str)],
) -> Option<&'static str> {
    overlays.iter().find_map(|(open, id)| open.then_some(*id))
}

impl Game<'_> {
    /// Opens `screen` (`Gui.setScreen`; `Screen::Playing` is vanilla's null):
    /// the one place the game's screen changes. A different screen runs
    /// `screen_opening` first (`ScreenEvent.Opening`: a mod may keep the
    /// current screen or open its own), then `screen_closing` for the screen
    /// it replaces. A game with no mod listening to either, and a server's
    /// or a guest's `Game`, assign the screen directly. A new native menu or
    /// same-type vanilla container can take over the already shown `Inventory`
    /// screen in place.
    pub(in crate::game) fn set_screen(&mut self, screen: Screen) {
        self.set_screen_with(screen, |_| {});
    }
    /// The hooks' answer to a change from `current` to `new`, for every path
    /// that changes the screen: `screen_change` (Opening, then Closing for
    /// `current` when `closes`), then what `screen_decision` makes of it.
    pub(in crate::game) fn decide_screen(
        &self,
        current: &ScreenView<'_>,
        new: &ScreenView<'_>,
        closes: bool,
    ) -> ScreenDecision {
        self.screen_decision(self.screen_change(current, new, true, closes))
    }
    /// Like `decide_screen`, for `mod_screen_open`, which clears any active
    /// overlay via `close_replaced_screen_for_mod` before applying the
    /// decision, so `ScreenOpening::ModScreen` and `ScreenOpening::Vanilla`
    /// answers remain eligible even when replacing an overlay.
    pub(in crate::game) fn decide_screen_for_mod_open(
        &self,
        current: &ScreenView<'_>,
        new: &ScreenView<'_>,
        closes: bool,
    ) -> ScreenDecision {
        self.screen_decision_with(self.screen_change(current, new, true, closes), true)
    }
    /// What a mod's `screen_opening` answer does here: a `ModScreen` only
    /// where a mod's screen can open (`mod_screen_blocked`), a `Vanilla`
    /// target only where it opens (a title screen while the title shows, a
    /// world's screen where a mod's could open in a world), else the change
    /// goes ahead and the refused target is logged. Records whether a mod's
    /// answer changed what shows (`ScreenState::answered`).
    pub(in crate::game) fn screen_decision(
        &self,
        answer: Option<Answered<ScreenOpening>>,
    ) -> ScreenDecision {
        self.screen_decision_with(answer, false)
    }
    /// The events of one change of screen, in order, for
    /// `set_screen` and for the overlays `observe_screen` announces: `opens`
    /// fires `screen_opening` (`ScreenEvent.Opening`) from `current` to `new`
    /// (a screen of `playing` is vanilla's null) and returns a mod's answer;
    /// then, unless it cancelled, `closes` fires `screen_closing` for
    /// `current`.
    pub(in crate::game) fn screen_change(
        &self,
        current: &ScreenView<'_>,
        new: &ScreenView<'_>,
        opens: bool,
        closes: bool,
    ) -> Option<Answered<ScreenOpening>> {
        // Asked before either screen draws: nothing beside it yet.
        let current = &crate::game::screen_backgrounds::without_regions(current);
        let new = &crate::game::screen_backgrounds::without_regions(new);
        let mut outcome = None;
        if opens {
            self.native_screen_opening(current, new, &mut outcome);
        }
        if closes
            && outcome
                .as_ref()
                .is_none_or(|a| a.answer != ScreenOpening::Cancel)
        {
            self.native_screen_closing(current);
        }
        outcome
    }

    /// What `observe_screen` compares with the last frame's.
    pub(in crate::game) fn observed_now(&self) -> Option<ObservedScreen> {
        // Only the title's screens and the overlays show over `Screen::Playing`.
        self.observed_screen_view()
            .map(|view| self.observe_view(view))
    }

    /// Snapshot of the receiving screen for screen input hooks, including overlays.
    pub(in crate::game) fn observed_input_screen(&self) -> Option<ObservedScreen> {
        let view = self.observed_screen_view()?;
        let regions = view.regions.to_vec();
        let mut snapshot = self.observe_view(view);
        snapshot.regions = regions;
        Some(snapshot)
    }

    /// Shows `title`, as `Minecraft.setScreen(new TitleScreen())` (or the
    /// screen it opens on: the multiplayer list, a disconnect, an error)
    /// does at startup and when a world closes. `screen_opening` fires to its
    /// screen, then `screen_closing` for an overlay it replaces, before it
    /// shows; `screen_init_post` follows. With no world open nothing else can
    /// show, so a mod's `Cancel` doesn't keep it shut; `ModScreen` opens the
    /// mod's own screen over it, and `Vanilla` another title screen.
    pub(in crate::game) fn show_title(&mut self, mut title: sunrise_client::title::Title) {
        title.set_rule_registry(Arc::new(crate::game::api_values::game_rule_registry()));
        self.show_title_with_registry(title);
    }

    pub(in crate::game) fn observe_screen(&mut self) {
        self.settle_title_mode();
        self.refresh_screen_widgets();
    }
}

/// `Level.setBlock`'s flags, for the `native_set_block_property` helpers:
/// `flags::UPDATE_CLIENTS` as a vanilla crop grows, `flags::UPDATE_ALL` as a
/// furnace sets its `lit`.
#[allow(unused_imports)]
pub(in crate::game) use sunrise_world::level::flags;

/// A mod's camera (`native_camera_pose`): each value that isn't finite keeps vanilla's from `view`.
pub(in crate::game) fn posed_camera(
    view: (glam::DVec3, f32, f32),
    position: [f64; 3],
    yaw: f32,
    pitch: f32,
) -> (glam::DVec3, f32, f32) {
    let position = std::array::from_fn(|i| {
        if position[i].is_finite() {
            position[i]
        } else {
            view.0[i]
        }
    });
    (
        glam::DVec3::from_array(position),
        if yaw.is_finite() { yaw } else { view.1 },
        if pitch.is_finite() { pitch } else { view.2 },
    )
}

/// The largest turn a shake gives, in degrees, on each axis.
pub(in crate::game) const SHAKE_MAX_DEGREES: f32 = 10.0;
/// The largest offset a shake gives, in blocks, along each axis.
pub(in crate::game) const SHAKE_MAX_OFFSET: f32 = 0.5;

#[allow(unused_imports)]
use camera_pose::bounded_shake;
#[allow(unused_imports)]
pub(in crate::game) use camera_pose::hand_pose_matrix;

/// The pose a bounded shake multiplies into the projection, after the view
/// bobbing: the offset, then roll, pitch and yaw.
pub(in crate::game) fn shake_pose(shake: Shake) -> glam::Mat4 {
    let [x, y, z] = shake.offset;
    glam::Mat4::from_translation(glam::Vec3::new(-x, -y, -z))
        * glam::Mat4::from_rotation_z(shake.roll.to_radians())
        * glam::Mat4::from_rotation_x(shake.pitch.to_radians())
        * glam::Mat4::from_rotation_y(shake.yaw.to_radians())
}

/// This client's window-owned mod screen and its local slot state.
pub(in crate::game) struct OpenNativeScreen {
    /// Fixed for the opening; slot moves have their own revision.
    pub(in crate::game) layout_revision: u64,
    pub(in crate::game) slot_moves: SlotMoves,
    pub(in crate::game) screen: mod_api::__private::OwnedMenuScreen,
    pub(in crate::game) menu: OpenNativeMenu,
    /// Installed as the window's screen, not merely staged for ScreenOpening.
    pub(in crate::game) shown: bool,
    /// The server closed the logical menu (`LocalPlayer.clientSideCloseContainer`),
    /// while the displayed screen stays whole through `screen_closing` or a veto
    /// until its removal runs (`AbstractContainerScreen` owns its own `menu`).
    pub(in crate::game) detached: bool,
}

/// The menu the screen presents: where its slots are stored, the opener,
/// and the window's live data.
pub(in crate::game) struct OpenNativeMenu {
    pub(in crate::game) key: Pos,
    pub(in crate::game) window: i32,
    pub(in crate::game) generation: u64,
    /// The menu's type, which its screen factory is keyed by.
    pub(in crate::game) name: String,
    /// The server title captured for this screen, kept if the logical menu closes first.
    pub(in crate::game) title: Option<mod_api::Text>,
    /// The mod block whose matching container opened the menu, if any.
    pub(in crate::game) block: Option<String>,
    pub(in crate::game) pos: Option<[i32; 3]>,
    pub(in crate::game) mod_type: Option<&'static sunrise_client::containers::ModMenuType>,
    pub(in crate::game) data: Vec<i32>,
}

#[cfg(test)]
pub(in crate::game) fn seed_native_menu(game: &mut Game<'_>, window: i32, name: &str) -> u64 {
    if game.remote.is_none() {
        let (pipe, _server) = sunrise_protocol::connection::Pipe::pair();
        game.connect_local(pipe, "Native screen fixture").unwrap();
    }
    if game.remote.as_ref().unwrap().menu.as_ref().is_some() {
        game.remote_close_local();
    }
    let menu = &mut game.remote.as_mut().unwrap().menu;
    menu.set(Some((window, name.to_owned())));
    menu.generation
}

/// Where a mod moved the open screen's slots after it opened, over the
/// positions its `MenuScreenLayout` declared. Interior state, as the screen callbacks that
/// move slots hold `&self`. A move is pending until the layout build commits
/// it, so drawing and hit-testing read one position within a frame and both
/// take a move together from the next.
#[derive(Default)]
pub(in crate::game) struct SlotMoves {
    /// Each moved slot's position, or None for a slot a mod hid.
    pub(in crate::game) committed:
        std::cell::RefCell<std::collections::HashMap<usize, Option<[f32; 2]>>>,
    pub(in crate::game) pending:
        std::cell::RefCell<std::collections::HashMap<usize, Option<[f32; 2]>>>,
    /// How many moves changed a position, for the layout build's cache key.
    pub(in crate::game) count: std::cell::Cell<u32>,
}

impl SlotMoves {
    /// Slot `slot`'s position this frame: where the last build committed it
    /// (None once hidden), else `declared`.
    pub(in crate::game) fn position(
        &self,
        slot: usize,
        declared: Option<[f32; 2]>,
    ) -> Option<[f32; 2]> {
        self.committed
            .borrow()
            .get(&slot)
            .copied()
            .unwrap_or(declared)
    }
    /// Where slot `slot` of a screen that declared `slots` will be from the
    /// next build: the last move, pending or committed, else its declaration.
    pub(in crate::game) fn latest(
        &self,
        slots: &[Option<mod_api::ScreenSlot>],
        slot: usize,
    ) -> Option<[f32; 2]> {
        let declared = slots.get(slot)?.as_ref().map(|slot| slot.position);
        let pending = self.pending.borrow().get(&slot).copied();
        match pending {
            Some(pending) => pending,
            None => self.position(slot, declared),
        }
    }
    /// Moves or activates slot `slot` of a screen that declared `slots` to `pos`,
    /// or hides it with None, from the next build (`Slot.isActive()`). A refusal
    /// or a repeat changes nothing, not even the move count.
    pub(in crate::game) fn set(
        &self,
        slots: &[Option<mod_api::ScreenSlot>],
        slot: usize,
        pos: Option<[f32; 2]>,
    ) -> Result<(), mod_api::ScreenSlotRefusal> {
        use mod_api::ScreenSlotRefusal;
        if slot >= slots.len() {
            return Err(ScreenSlotRefusal::NoSuchSlot);
        }
        if pos.is_some_and(|[x, y]| !x.is_finite() || !y.is_finite()) {
            return Err(ScreenSlotRefusal::BadPosition);
        }
        if self.latest(slots, slot) != pos {
            self.pending.borrow_mut().insert(slot, pos);
            self.count.set(self.count.get().wrapping_add(1));
        }
        Ok(())
    }
    /// Makes the pending moves the positions drawing and hit-testing read.
    pub(in crate::game) fn commit(&self) {
        let mut pending = self.pending.borrow_mut();
        if !pending.is_empty() {
            self.committed.borrow_mut().extend(pending.drain());
        }
    }
}

impl Game<'_> {
    /// The menu type resolved for this client's opening, including in ScreenOpening.
    pub(in crate::game) fn native_screen_menu_type(
        &self,
        window: i32,
        name: &str,
    ) -> Option<&'static sunrise_client::containers::ModMenuType> {
        let menu = &self.active_native_screen()?.menu;
        (menu.window == window && menu.name == name).then_some(menu.mod_type)?
    }
}

impl OpenNativeScreen {
    pub(in crate::game) fn layout(&self) -> &mod_api::MenuScreenLayout {
        self.screen.layout()
    }

    #[cfg(test)]
    pub(in crate::game) fn layout_mut(&mut self) -> &mut mod_api::MenuScreenLayout {
        self.screen.layout_mut()
    }

    pub(in crate::game) fn frame(
        &self,
        batch: &HudBatch,
        (ox, oy): (f32, f32),
        (x, y): (f32, f32),
    ) -> mod_api::MenuScreenFrame<'_> {
        mod_api::MenuScreenFrame {
            origin: [ox, oy],
            pointer: [x as i32, y as i32],
            gui_size: [batch.width as i32, batch.height as i32],
            at: [x - ox, y - oy],
            data: &self.menu.data,
        }
    }

    /// How many slot moves changed a position, so a layout built before the
    /// last one is rebuilt.
    pub(in crate::game) fn slot_move_count(&self) -> u32 {
        self.slot_moves.count.get()
    }
    /// Makes the slot moves a mod made since the last build the screen's
    /// positions; the layout build calls it before it reads them.
    pub(in crate::game) fn commit_slot_moves(&self) {
        self.slot_moves.commit();
    }
    /// Data slot `index` of window `window`'s menu: false for another window.
    /// The caller resolves the shown opening before passing it a packet.
    pub(in crate::game) fn set_data(&mut self, window: i32, index: usize, value: i32) -> bool {
        if window != self.menu.window {
            return false;
        }
        if self.menu.mod_type.map_or(
            index < usize::from(sunrise_mod_package::content::MAX_SIGNED_DATA_SLOTS),
            |kind| index < kind.data_slots,
        ) {
            if self.menu.data.len() <= index {
                self.menu.data.resize(index + 1, 0);
            }
            self.menu.data[index] = value;
        }
        true
    }

    pub(in crate::game) fn container_slot_base(&self) -> usize {
        if self.menu.mod_type.is_some() {
            super::inventory_slots::MOD_MENU_SLOT_BASE
        } else {
            300
        }
    }

    pub(in crate::game) fn player_slots(&self) -> usize {
        self.menu
            .mod_type
            .map_or(36, sunrise_client::containers::ModMenuType::player_slots)
    }

    pub(in crate::game) fn visible_container_slots(&self) -> impl Iterator<Item = usize> + '_ {
        let base = self.container_slot_base();
        self.layout()
            .slots
            .iter()
            .enumerate()
            .filter_map(move |(i, slot)| {
                self.slot_moves
                    .position(i, slot.as_ref().map(|slot| slot.position))?;
                Some(base + i)
            })
    }

    pub(in crate::game) fn visible_slots(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.player_slots()).chain(self.visible_container_slots())
    }
}

/// The player as a native mod reads it.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct NativePlayer {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Degrees; 0 faces south, 90 west (`Direction.fromYRot`).
    pub yaw: f32,
    pub pitch: f32,
    /// `minecraft:overworld`, `minecraft:the_nether`, `minecraft:the_end`.
    pub dimension: &'static str,
}

/// How the panel lays out a recipe of `native_panel_recipes`.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeRecipeLayout {
    /// A crafting grid of `width` x `height` slots, an arrow and the result.
    Grid,
    /// One input over a flame, an arrow and the result (furnaces, smokers,
    /// campfires).
    Cooking,
    /// One input, an arrow and the result (the stonecutter).
    Single,
    /// Template, base and addition, an arrow and the result.
    Smithing,
    /// A potion and an ingredient, an arrow and the result.
    Brewing,
}

/// One result of a recipe the panel shows (`native_panel_recipes`): a slot
/// cycling through `stacks`, one a second, and the chance in percent the recipe
/// gives it (`Some(25.0)` draws "25%"); `None` gives it every time.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeRecipeResult {
    pub(crate) stacks: Vec<mod_api::Stack>,
    pub(crate) chance: Option<f32>,
}

/// A recipe the panel shows (`native_panel_recipes`). Each slot, and each
/// result, cycles through its stacks, one a second; an empty slot shows
/// nothing. A crafting recipe has one result and no chance.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeRecipeView {
    pub(crate) layout: NativeRecipeLayout,
    /// The grid's size (`Grid` only), at most 3 x 3; slots past it continue
    /// in rows under it, 27 in all.
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) slots: Vec<Vec<mod_api::Stack>>,
    /// Up to four, left to right after the arrow.
    pub(crate) results: Vec<NativeRecipeResult>,
    /// Fluids the recipe takes and gives, `amount` in millibuckets, each in
    /// a well with its amount under it; at most eight each. A well shows the
    /// fluid's client look (`data/<mod>/sunrise/fluid/`: sprite and tint), a
    /// flat square of vanilla's colour for water and lava and else grey when
    /// the fluid has none.
    pub(crate) fluid_inputs: Vec<FluidStack>,
    pub(crate) fluid_outputs: Vec<FluidStack>,
    /// A short line under the recipe (a cooking time), or "".
    pub(crate) note: String,
    /// The action button beside it: none, enabled, or shown greyed.
    pub(crate) action: Option<bool>,
}

/// An item of [`NativeRecipes::items`], by its index there, with a count
/// and the components it has beyond the item's own, in vanilla's item
/// syntax (`[minecraft:potion_contents={potion:"minecraft:swiftness"}]`),
/// or "".
#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeRecipeStack {
    pub(crate) item: u32,
    pub(crate) count: u32,
    pub(crate) components: String,
}

/// The station a recipe is made at.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeRecipeKind {
    CraftingShaped,
    CraftingShapeless,
    Smelting,
    Blasting,
    Smoking,
    Campfire,
    Stonecutting,
    Smithing,
    Brewing,
}

/// A recipe of [`NativeRecipes`].
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeRecipe {
    /// `minecraft:oak_planks`.
    pub(crate) id: String,
    pub(crate) kind: NativeRecipeKind,
    /// The recipe book's tab (`crafting_building_blocks`, `furnace_food`),
    /// or "".
    pub(crate) category: String,
    /// Recipes the book shows on one button share a group, or "".
    pub(crate) group: String,
    /// A shaped recipe's grid: `slots` holds its cells row by row. 0 for
    /// the other kinds.
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Each slot's stacks, any one of which fits; an empty list is an empty
    /// cell. Cooking and stonecutting: the input. Smithing: the template,
    /// the base, the addition. Brewing: the bottle, the reagent.
    pub(crate) slots: Vec<Vec<NativeRecipeStack>>,
    /// What it makes; several when that varies (a trim on any armour).
    pub(crate) results: Vec<NativeRecipeStack>,
    /// Cooking: game ticks and experience; 0 for the other kinds.
    pub(crate) cooking_time: u32,
    pub(crate) experience: f32,
    /// The recipe book's id for it (one per way the book shows it), as
    /// `known_recipes` lists them; -1 for the kinds the book doesn't
    /// show (campfire, stonecutting, smithing, brewing).
    pub(crate) book_id: i32,
}

/// An item of [`NativeRecipes::items`].
#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeItem {
    /// `minecraft:diamond`, `<mod>:<path>`.
    pub(crate) id: String,
    /// In the player's language.
    pub(crate) name: String,
    /// Its item tags, without the `#`.
    pub(crate) tags: Vec<String>,
}

/// Every item and recipe the game knows (`native_recipes`): `items` in the
/// registry's order, which each [`NativeRecipeStack::item`] indexes; the
/// recipes are the book's in its order, then campfire, stonecutting,
/// smithing and brewing.
#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeRecipes {
    pub(crate) items: Vec<NativeItem>,
    pub(crate) recipes: Vec<NativeRecipe>,
}

/// Which mouse Post hook owes dispatch (`ScreenMousePost`).
pub(in crate::game) enum ScreenMousePostKind {
    Scroll([f64; 2]),
    Drag { button: i32, delta: [f32; 2] },
}

/// The Post a mouse input owes once the screen has handled it, built when
/// its Pre let the input through (`Game::screen_mouse_post`), carrying the
/// receiving screen and modifiers captured at Pre time.
pub(in crate::game) struct ScreenMousePost {
    pub(in crate::game) receiver: ObservedScreen,
    pub(in crate::game) modifiers: u32,
    pub(in crate::game) kind: ScreenMousePostKind,
    pub(in crate::game) handled: bool,
}

impl ScreenMousePost {
    pub(in crate::game) fn new(
        receiver: ObservedScreen,
        modifiers: u32,
        kind: ScreenMousePostKind,
    ) -> Self {
        Self {
            receiver,
            modifiers,
            kind,
            handled: false,
        }
    }
}

/// What a drag's Pre decided (`Game::screen_mouse_dragged`).
pub(in crate::game) enum ScreenMousePre {
    /// No screen hook ran: no drag, a synthetic move or no screen open.
    NotFired,
    Canceled,
    Passed(ScreenMousePost),
}

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// Admission for ordinary toasts. Only an accepted candidate enters its
    /// queue; mandatory callback-panic diagnostics bypass mod admission.
    pub(in crate::game) fn admit_toast(
        &self,
        kind: mod_api::ToastKind,
        title: &mut Text,
        description: &mut Option<Text>,
    ) -> bool {
        // Shared advancement/recipe producers also run on logical servers;
        // only their receiving client owns toast presentation and its hook.
        if self.is_logical_server() {
            return false;
        }
        let mut cancel = false;
        self.native_toast_add(kind, title, description, &mut cancel);
        !cancel
    }

    /// The milliseconds a client tick lasts at this frame's clock advance
    /// (`Minecraft.getTickTargetMillis`), as a mod's `tick_target_millis` leaves it. An answer
    /// that is not a finite number above zero keeps the clock's own.
    pub(in crate::game) fn tick_target_millis(&self) -> f32 {
        let vanilla = self.clock.tick_target_millis();
        let mut millis = vanilla;
        self.native_tick_target_millis(&mut millis);
        if millis.is_finite() && millis > 0.0 {
            millis
        } else {
            vanilla
        }
    }
    /// The hearts the local player's health bar draws (`Hud.HeartType.forPlayer`, then
    /// NeoForge's `PlayerHeartTypeEvent`): poisoned, else withered, else frozen when fully
    /// frozen, else normal, as a mod's `player_heart_type` leaves it. None without a player.
    pub(in crate::game) fn player_heart_kind(
        &mut self,
    ) -> Option<mod_api::client::hud_layout::HeartKind> {
        use mod_api::client::hud_layout::HeartKind;
        let p = self.player.as_ref()?;
        let effects = &self.potions.effects;
        let mut kind = if effects.get("poison").is_some() {
            HeartKind::Poisoned
        } else if effects.get("wither").is_some() {
            HeartKind::Withered
        } else if p.is_fully_frozen() {
            HeartKind::Frozen
        } else {
            HeartKind::Normal
        };
        self.native_player_heart_type(&mut kind);
        Some(kind)
    }
    /// Deliver a non-repeat key press to the native panel.
    pub(in crate::game) fn native_panel_key_event(
        &mut self,
        key: KeyCode,
        pressed: bool,
        repeat: bool,
    ) {
        if pressed
            && !repeat
            && let Some(name) = keybinds::key_name(key)
        {
            self.native_panel_key(name);
        }
    }
    /// Whether any mod listens to a hook given the open screen's
    /// `ScreenView` (render, input, open and close, room, exclusion): only
    /// then are its regions collected.
    pub(in crate::game) fn screen_view_listened(&self) -> bool {
        use mod_api::HookId;
        [
            HookId::ScreenRenderPre,
            HookId::ScreenRenderBackground,
            HookId::ScreenRenderForeground,
            HookId::ScreenRenderPost,
            HookId::ScreenMouseButtonPressedPre,
            HookId::ScreenMouseButtonPressedPost,
            HookId::ScreenMouseButtonReleasedPre,
            HookId::ScreenMouseButtonReleasedPost,
            HookId::ScreenMouseDraggedPre,
            HookId::ScreenMouseDraggedPost,
            HookId::ScreenMouseScrolledPre,
            HookId::ScreenMouseScrolledPost,
            HookId::ScreenKeyPressedPre,
            HookId::ScreenKeyPressedPost,
            HookId::ScreenKeyReleasedPre,
            HookId::ScreenKeyReleasedPost,
            HookId::ScreenCharTypedPre,
            HookId::ScreenCharTypedPost,
            HookId::ScreenOpening,
            HookId::ScreenClosing,
            HookId::ScreenInitPost,
            HookId::ScreenRoom,
            HookId::ScreenExclusionAreas,
        ]
        .into_iter()
        .any(mod_api::__private::listened)
    }
    /// Whether a mod listens to one of the four screen render hooks; nothing
    /// is built to fire them otherwise.
    pub(in crate::game) fn screen_render_listened(&self) -> bool {
        use mod_api::HookId;
        [
            HookId::ScreenRenderPre,
            HookId::ScreenRenderBackground,
            HookId::ScreenRenderForeground,
            HookId::ScreenRenderPost,
        ]
        .into_iter()
        .any(mod_api::__private::listened)
    }
    /// `Render.Pre`: whether the screen draws. The screen owns its later phases.
    pub(in crate::game) fn screen_render_begin(
        &self,
        draw: &mut HudBatch,
        screen: &ScreenView<'_>,
    ) -> bool {
        let mut canceled = false;
        self.native_screen_render_pre(draw, screen, &mut canceled);
        !canceled
    }
    /// `Render.Post`, including when Pre cancelled the screen's drawing.
    pub(in crate::game) fn screen_render_end(&self, draw: &mut HudBatch, screen: &ScreenView<'_>) {
        self.native_screen_render_post(draw, screen);
    }
    /// The open screen as a native mod sees it (`ScreenView`): its id,
    /// the open menu's type and title, its slot count, and its panel's
    /// rectangle, with the mouse and the partial tick. `screen` is the screen
    /// drawn this frame, which `ScreenView.id` comes from. Borrowed:
    /// nothing is copied per frame.
    pub(in crate::game) fn screen_view(&self, batch: &HudBatch, screen: Screen) -> ScreenView<'_> {
        self.screen_view_at(screen, self.screen_rect(batch, screen))
    }
    /// `screen_view` with the panel `rect` the caller is laying out: the
    /// container build tells a mod the panel it is about to draw, not the one
    /// the previous frame drew.
    pub(in crate::game) fn screen_view_at(&self, screen: Screen, rect: [f32; 4]) -> ScreenView<'_> {
        // Only the inventory screen shows a server's menu: a menu staged for
        // a screen a mod replaced is not the pause or chat screen's.
        let menu_shown = screen == Screen::Inventory;
        let shown_native = menu_shown.then(|| self.shown_native_screen()).flatten();
        ScreenView::from(mod_api::__private::built::ScreenView {
            id: screen.id(),
            mod_screen: (screen == Screen::Mod).then(|| self.mod_screen()).flatten(),
            menu: self
                .remote_menu_type()
                .filter(|m| menu_shown && !m.is_empty())
                .or_else(|| {
                    shown_native
                        .map(|s| s.menu.name.as_str())
                        .filter(|m| !m.is_empty())
                }),
            title: self
                .remote
                .as_ref()
                .and_then(|r| r.menu_title_snapshot())
                .filter(|_| menu_shown)
                .or_else(|| shown_native.and_then(|s| s.menu.title.as_ref())),
            rect,
            slots: self.slot_count(),
            mouse: self.cursor(),
            partial_tick: self.clock.fraction() as f32,
            regions: self.screen_regions(screen),
        })
    }
    /// The open screen's container panel in GUI pixels (`screen`'s, the one
    /// this frame draws), or the whole window for a screen with none
    /// (`ScreenView.rect`).
    pub(in crate::game) fn screen_rect(&self, batch: &HudBatch, screen: Screen) -> [f32; 4] {
        if screen != Screen::Inventory && screen != Screen::Creative {
            return [0.0, 0.0, batch.width, batch.height];
        }
        // The window the game itself draws (`container_image`): the merchant
        // and the beacon are wider and shifted, creative is its own size.
        let image = self.container_image(batch);
        [
            image.left as f32,
            image.top as f32,
            image.width as f32,
            image.height as f32,
        ]
    }
    /// Button Post sees the original receiver and its answer; return the final handled bit.
    pub(in crate::game) fn screen_button_post(
        &self,
        receiver: &ObservedScreen,
        button: i32,
        pressed: bool,
        modifiers: u32,
        double_click: bool,
        handled: bool,
    ) -> bool {
        let screen = receiver.view();
        let mut result = TriState::Default;
        if pressed {
            self.native_screen_mouse_button_pressed_post(
                &screen,
                button,
                modifiers,
                double_click,
                handled,
                &mut result,
            );
        } else {
            self.native_screen_mouse_button_released_post(
                &screen,
                button,
                modifiers,
                handled,
                &mut result,
            );
        }
        result.or(handled)
    }

    /// Unhandled scroll/drag notices go to the receiver captured before handling.
    pub(in crate::game) fn screen_mouse_post(&self, post: ScreenMousePost) {
        if post.handled {
            return;
        }
        let screen = post.receiver.view();
        match post.kind {
            ScreenMousePostKind::Scroll(scroll) => {
                self.native_screen_mouse_scrolled_post(&screen, scroll, post.modifiers);
            }
            ScreenMousePostKind::Drag { button, delta } => {
                self.native_screen_mouse_dragged_post(&screen, button, delta, post.modifiers);
            }
        }
    }
    /// A key press's characters (`ScreenEvent.CharacterTyped.Pre`, vanilla's
    /// `KeyboardHandler.charTyped`), dispatched once the screen has acted on
    /// the key: GLFW's char callback comes after its key callback, so the
    /// characters go to the screen that is open now — a key that closed the
    /// screen leaves none to hear them, as vanilla's closed screen hears
    /// nothing, and one that moved the focus types into the box it moved to.
    /// Each printable one snapshots the current screen, runs
    /// `native_screen_char_typed_pre` (when hooks apply), then mounted widgets,
    /// then the focused text box, and unhandled characters fire
    /// `native_screen_char_typed_post` on that character's captured receiver.
    pub(in crate::game) fn screen_chars_typed(&mut self, text: Option<&str>) {
        let Some(text) = text else {
            return;
        };
        if self.observed_screen_view().is_none() && self.headless_screen_snapshot().is_none() {
            return;
        }
        // A held edit modifier makes a shortcut of the key, not text.
        let text = chars_to_type(text, self.types_text());
        if text.is_empty() {
            return;
        }
        self.screen_chars_dispatch(&text, |game, c| game.type_chars_into_screen(c).is_ok());
    }
    pub(in crate::game) fn observe_client_packet(
        &mut self,
        packet: &sunrise_protocol::packets::play::Clientbound,
    ) {
        if !mod_api::__private::listened(mod_api::HookId::ClientPacket) {
            return;
        }
        let view = client_packet_view(packet);
        self.native_client_packet(&view);
    }
    /// Whether a mod cancelled `packet` on its way to the server (`client_packet_send`).
    /// Builds the packet's view only when a mod listens.
    pub(in crate::game) fn client_packet_send_canceled(
        &self,
        packet: &sunrise_protocol::packets::play::Serverbound,
    ) -> bool {
        if !mod_api::__private::listened(mod_api::HookId::ClientPacketSend) {
            return false;
        }
        let mut canceled = false;
        self.native_client_packet_send(&serverbound_packet_view(packet), &mut canceled);
        canceled
    }
    /// The living entity an effect decision names: `Some(id)` a mob's
    /// `Entity.getId`, `None` the acting player. `None` when no native
    /// world runs, and the decision falls back to vanilla's.
    pub(in crate::game) fn effect_living_ref(&self, entity: Option<i32>) -> Option<LivingRef> {
        let epoch = self.native_lifecycle.epoch()?;
        let id = match entity {
            Some(id) => id as u32,
            None => self.own_entity_id() as u32,
        };
        Some(LivingRef::new(EntityRef::new(epoch, id)))
    }
    /// Deliver what the batch just ticked recorded, in order: `minecart_pass`
    /// to the parts, and the writes a landing, step, or mid-walk `entity_inside`
    /// contact asked for to the level. A write is skipped when the block at
    /// its position is no longer the state it touched, as vanilla never sees
    /// a stale one. The parts' own `entity_inside` contacts fire mid-walk now,
    /// on the shared inside sweep, not here.
    pub(in crate::game) fn drain_entity_inside(&mut self) {
        for contact in sunrise_client::block_contact::take_pending() {
            let (state, pos) = match &contact {
                Pending::Landed(landed) => (landed.state, landed.position),
                Pending::MinecartPass(pass) => (pass.state, pass.position),
                Pending::InsideWritten(written) => (written.state, written.position),
            };
            let Some(level) = self.level(self.dim.id()) else {
                continue;
            };
            if self.level_block_state(level, [pos.x, pos.y, pos.z]) != Some(state) {
                continue;
            }
            match contact {
                Pending::MinecartPass(pass) => {
                    if let Some(entity) = u32::try_from(pass.entity)
                        .ok()
                        .and_then(|id| self.entity(id))
                    {
                        self.native_minecart_pass(state, pos, entity);
                    }
                }
                Pending::Landed(landed) => {
                    for write in landed.writes {
                        self.apply_landing_write(level, pos, write);
                    }
                }
                // The mid-walk contacts' own level writes, guarded on the touched
                // state above, as a landing's are.
                Pending::InsideWritten(written) => {
                    for write in written.writes {
                        self.apply_inside_write(level, pos, write);
                    }
                }
            }
        }
    }

    /// Apply one already-validated engine write, at the block the entity landed on.
    /// The original-state guard runs once before the ordered batch, not between writes.
    pub(in crate::game) fn apply_landing_write(
        &mut self,
        level: mod_api::LevelRef,
        pos: BlockPos,
        write: sunrise_world::level::LandingWrite,
    ) {
        let at = [pos.x, pos.y, pos.z];
        match write {
            // `Level.setBlockAndUpdate`: flag 3, the neighbours and the clients.
            sunrise_world::level::LandingWrite::SetBlock(state) => {
                let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
                let _ = self.level_set_block(level, at, state, flags);
            }
            sunrise_world::level::LandingWrite::DestroyBlock { drop } => {
                self.level_destroy_block(level, at, drop, None, false);
            }
            // A write a later API adds that this game does not know is not applied.
            _ => {}
        }
    }
    /// A write an `entity_inside` part asked for, applied as a landing write is: at the
    /// touched block, once the entity's batch has ticked.
    pub(in crate::game) fn apply_inside_write(
        &mut self,
        level: mod_api::LevelRef,
        pos: BlockPos,
        write: sunrise_world::level::InsideWrite,
    ) {
        let at = [pos.x, pos.y, pos.z];
        match write {
            sunrise_world::level::InsideWrite::SetBlock(state) => {
                let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
                let _ = self.level_set_block(level, at, state, flags);
            }
            sunrise_world::level::InsideWrite::DestroyBlock { drop } => {
                self.level_destroy_block(level, at, drop, None, false);
            }
            // A write a later API adds that this game does not know is not applied.
            _ => {}
        }
    }
}

/// `native_living_use_totem` for the damage code: whether it
/// canceled the totem. The entity and the source are named as the damage hook names them.
pub(in crate::game) fn use_totem(
    level: DamageLevel<'_>,
    hurting: Hurting<'_>,
    off_hand: bool,
) -> bool {
    if !mod_api::__private::listened(mod_api::HookId::LivingUseTotem) {
        return false;
    }
    let (entity, kind, source) = hurt_server_args(hurting, level.attack());
    let hand = if off_hand { Hand::Off } else { Hand::Main };
    let mut cancel = false;
    HitLevel(level).native_living_use_totem(
        entity,
        kind,
        &source,
        hand,
        "minecraft:totem_of_undying",
        &mut cancel,
    );
    cancel
}

/// The level a hit or a landing happens in, as its hooks' level host: its blocks, its random and its
/// own attachments. Not `GameLevel`, which a hit can't build: a mob's hit lands in the mob
/// tick, where the world is read by every mob and its random is lent out, and building one
/// per hit would allocate its entity boxes.
pub(in crate::game) struct HitLevel<'a>(pub(in crate::game) DamageLevel<'a>);

/// Let listeners override the vanilla invulnerability decision for this hit.
pub(in crate::game) fn invulnerability_check(
    level: DamageLevel<'_>,
    hurting: Hurting<'_>,
    vanilla: bool,
) -> bool {
    if !mod_api::__private::listened(mod_api::HookId::EntityInvulnerabilityCheck) {
        return vanilla;
    }
    let (entity, kind, source) = hurt_server_args(hurting, level.attack());
    let mut invulnerable = vanilla;
    HitLevel(level).native_entity_invulnerability_check(
        entity,
        kind,
        &source,
        vanilla,
        &mut invulnerable,
    );
    invulnerable
}

pub(in crate::game) fn engine_reduction(
    reduction: mod_api::Reduction,
) -> sunrise_world::living_damage::Reduction {
    match reduction {
        mod_api::Reduction::Invulnerability => {
            sunrise_world::living_damage::Reduction::Invulnerability
        }
        mod_api::Reduction::Armor => sunrise_world::living_damage::Reduction::Armor,
        mod_api::Reduction::Enchantments => sunrise_world::living_damage::Reduction::Enchantments,
        mod_api::Reduction::MobEffects => sunrise_world::living_damage::Reduction::MobEffects,
        mod_api::Reduction::Absorption => sunrise_world::living_damage::Reduction::Absorption,
        mod_api::Reduction::InnateResistance => {
            sunrise_world::living_damage::Reduction::InnateResistance
        }
        _ => unreachable!("unknown API reduction"),
    }
}

pub(in crate::game) struct LiveDamageRead<'a>(
    pub(in crate::game) &'a sunrise_world::living_damage::DamageState,
);

impl mod_api::__private::DamageContainerRead for LiveDamageRead<'_> {
    fn original(&self) -> f32 {
        self.0.original()
    }
    fn new_damage(&self) -> f32 {
        self.0.new_damage()
    }
    fn blocked(&self) -> f32 {
        self.0.blocked()
    }
    fn shield_damage(&self) -> f32 {
        self.0.shield_damage()
    }
    fn post_attack_invulnerability_ticks(&self) -> i32 {
        self.0.post_attack_invulnerability_ticks()
    }
    fn should_cause_side_effects(&self) -> bool {
        self.0.should_cause_side_effects()
    }
    fn inflicted(&self) -> f32 {
        self.0.inflicted()
    }
    fn health_damage(&self) -> f32 {
        self.0.health_damage()
    }
    fn reduction(&self, reduction: mod_api::Reduction) -> f32 {
        self.0.reduction(engine_reduction(reduction))
    }
}

pub(in crate::game) struct LiveDamageContainer<'a>(pub(in crate::game) &'a mut DamageContainer);

impl mod_api::__private::DamageContainerRead for LiveDamageContainer<'_> {
    fn original(&self) -> f32 {
        self.0.original()
    }
    fn new_damage(&self) -> f32 {
        self.0.new_damage()
    }
    fn blocked(&self) -> f32 {
        self.0.blocked()
    }
    fn shield_damage(&self) -> f32 {
        self.0.shield_damage()
    }
    fn post_attack_invulnerability_ticks(&self) -> i32 {
        self.0.post_attack_invulnerability_ticks()
    }
    fn should_cause_side_effects(&self) -> bool {
        self.0.should_cause_side_effects()
    }
    fn inflicted(&self) -> f32 {
        self.0.inflicted()
    }
    fn health_damage(&self) -> f32 {
        self.0.health_damage()
    }
    fn reduction(&self, reduction: mod_api::Reduction) -> f32 {
        self.0.reduction(engine_reduction(reduction))
    }
}

impl mod_api::__private::DamageContainerBackend for LiveDamageContainer<'_> {
    fn set_new_damage(&mut self, value: f32) {
        self.0.record_new_damage(value);
    }
    fn set_post_attack_invulnerability_ticks(&mut self, value: i32) {
        self.0.record_post_attack_invulnerability_ticks(value);
    }
    fn add_reduction_modifier(
        &mut self,
        reduction: mod_api::Reduction,
        owner: Option<&'static str>,
        modifier: Box<dyn FnMut(mod_api::DamageContainerView<'_>, f32) -> f32>,
    ) {
        let mut wrapped = mod_api::__private::wrap_reduction_modifier(owner, modifier);
        self.0.add_reduction_modifier(
            engine_reduction(reduction),
            Box::new(move |engine_container: &DamageContainer, amount: f32| {
                wrapped(
                    mod_api::DamageContainerView::new(&LiveDamageRead(engine_container)),
                    amount,
                )
            }),
        );
    }
}

/// `native_living_incoming_damage` for the damage code: whether it
/// canceled the hit.
pub(in crate::game) fn incoming_damage(
    level: DamageLevel<'_>,
    hurting: Hurting<'_>,
    victim: Victim,
    container: &mut DamageContainer,
) -> bool {
    let listens_incoming = mod_api::__private::listened(mod_api::HookId::LivingIncomingDamage);
    if listens_incoming
        || mod_api::__private::listened(mod_api::HookId::LivingDamagePre)
        || mod_api::__private::listened(mod_api::HookId::LivingDamagePost)
    {
        container.enable_live_reductions();
    }
    if !listens_incoming {
        return false;
    }
    let (entity, kind, source) = hurt_server_args(hurting, level.attack());
    let mut canceled = false;
    let attack = level.attack();
    let Ok(weapon) = attack.weapon_of(source.direct).try_into_api() else {
        return false;
    };
    let causing_weapon = (source.causing != 0 && source.causing == attack.attacker)
        .then_some(attack.held[0].as_ref())
        .flatten();
    let Ok(causing_weapon) = causing_weapon.try_into_api() else {
        return false;
    };
    let mut live = LiveDamageContainer(container);
    HitLevel(level.caused_by(source.causing)).native_living_incoming_damage(
        mod_api::__private::LivingIncomingDamageCall {
            entity,
            kind,
            source: &source,
            victim: &victim,
            weapon: weapon.as_ref(),
            causing_weapon: causing_weapon.as_ref(),
            container: mod_api::DamageContainer::new(&mut live),
            canceled: &mut canceled,
        },
    );
    canceled
}

/// `native_living_damage_pre` for the damage code.
pub(in crate::game) fn damage_pre(
    level: DamageLevel<'_>,
    hurting: Hurting<'_>,
    container: &mut DamageContainer,
) {
    if !mod_api::__private::listened(mod_api::HookId::LivingDamagePre) {
        return;
    }
    container.enable_live_reductions();
    let (entity, kind, source) = hurt_server_args(hurting, level.attack());
    let mut live = LiveDamageContainer(container);
    HitLevel(level.caused_by(source.causing)).native_living_damage_pre(
        mod_api::__private::LivingDamagePreCall {
            entity,
            kind,
            source: &source,
            container: mod_api::DamageContainer::new(&mut live),
        },
    );
}

/// `native_living_damage_post` for the damage code.
pub(in crate::game) fn damage_post(
    level: DamageLevel<'_>,
    hurting: Hurting<'_>,
    container: &DamageContainer,
) {
    if !mod_api::__private::listened(mod_api::HookId::LivingDamagePost) {
        return;
    }
    let (entity, kind, source) = hurt_server_args(hurting, level.attack());
    let live = LiveDamageRead(container);
    HitLevel(level.caused_by(source.causing)).native_living_damage_post(
        entity,
        kind,
        &source,
        mod_api::DamageContainerView::new(&live),
    );
}

/// `native_living_shield_block` for the damage code. Its one
/// caller. The hurt entity and the source are named as the damage hook names them.
pub(in crate::game) fn shield_block(
    level: DamageLevel<'_>,
    hurting: Hurting<'_>,
    damage: f32,
    original_blocked: bool,
    block: &mut ShieldBlock,
) {
    let (entity, kind, source) = hurt_server_args(hurting, level.attack());
    HitLevel(level).native_living_shield_block(
        entity,
        kind,
        &source,
        damage,
        original_blocked,
        block,
    );
}

impl Game<'_> {
    /// [`Self::attack_of`] for the attacker a damage source names: the player, a mob, or (none)
    /// the attack the enclosing [`Self::with_attack`] lends.
    pub(in crate::game) fn attack_of_who(&self, attacker: Option<Who>) -> Attack {
        match attacker {
            Some(Who::Player) => self.attack_of(self.player.as_ref().map_or(0, |p| p.entity_id)),
            Some(Who::Mob(id)) => self.attack_of(id as i32),
            None => self.current_attack(),
        }
    }
}

/// `native_living_knockback` for the knockback code: whether it
/// canceled the push. Its one caller. The pushed entity and the source are named as the
/// damage hook names them.
pub(in crate::game) fn knockback(
    level: DamageLevel<'_>,
    pushed: Hurting<'_>,
    knock: &mut Knock,
) -> bool {
    if !mod_api::__private::listened(mod_api::HookId::LivingKnockback) {
        return false;
    }
    let attack = level.attack();
    let attacker = match &pushed {
        Hurting::Mob(mob, damage) => match damage.attacker.or(mob.ai.last_hurt_by) {
            Some(Who::Mob(id)) => id as i32,
            _ => attack.attacker,
        },
        Hurting::Player(_, source)
        | Hurting::Dragon(_, source)
        | Hurting::ArmorStand(_, source)
        | Hurting::Entity(_, _, source) => match source.causing {
            0 => attack.attacker,
            causing => causing,
        },
    };
    let (kill_credit, credit_main) = level.knockback_credit(&pushed);
    let Ok(credit_main) = credit_main.map(TryIntoApi::try_into_api).transpose() else {
        return false;
    };
    let (entity, kind, source) = hurt_server_args(pushed, level.attack());
    let mut canceled = false;
    let [main, off] = &attack.held;
    let (Ok(main), Ok(off)) = (main.try_into_api(), off.try_into_api()) else {
        return false;
    };
    let entry = level.knockback_entry();
    HitLevel(level).native_living_knockback(mod_api::__private::LivingKnockbackCall {
        entity,
        kind,
        source: &source,
        attacker,
        attacker_main: main.as_ref(),
        attacker_off: off.as_ref(),
        kill_credit,
        kill_credit_main: credit_main.as_ref(),
        entry,
        strength: &mut knock.strength,
        x: &mut knock.x,
        z: &mut knock.z,
        canceled: &mut canceled,
    });
    canceled
}

/// The damage hooks every level of the game lends to its hits (`Game.damage_hooks`).
pub(in crate::game) const DAMAGE_HOOKS: DamageHooks = DamageHooks {
    invulnerable: Some(invulnerability_check),
    incoming: Some(incoming_damage),
    pre: Some(damage_pre),
    post: Some(damage_post),
    owner_post_listened: Some(|| mod_api::__private::listened(mod_api::HookId::LivingDamagePost)),
    knockback: Some(knockback),
    totem: Some(use_totem),
    shield: Some(shield_block),
    accepted: Some(living_death::capture_hit),
};
/// The contacts of a simulating entity with a block, as the simulation's own code (client-lib)
/// reports them.
pub(in crate::game) const BLOCK_CONTACT_HOOKS: sunrise_client::block_contact::Hooks =
    sunrise_client::block_contact::Hooks {
        step_listens: |state| {
            crate::game::api_values::block_state::block_listens(mod_api::HookId::StepOn, state)
        },
        step_on: |level, step, movement| {
            use self::bounce_restitution::StepOnView;
            use sunrise_client::block_contact::StepLevel;
            match level {
                StepLevel::Server(level) => StepOnView(mod_api::LevelRandomSide::Server(
                    &mut HitLevel(level.reborrow()),
                ))
                .native_step_on(step, movement),
                StepLevel::Client(level) => StepOnView(mod_api::LevelRandomSide::Client(
                    &crate::game::native_surface::FrictionView(level),
                ))
                .native_step_on(step, movement),
            }
        },
        any_inside: || mod_api::__private::has_scoped_hook(mod_api::HookId::EntityInside),
        inside_listens: |state| {
            crate::game::api_values::block_state::block_listens(
                mod_api::HookId::EntityInside,
                state,
            )
        },
        pass_listens: |state| {
            crate::game::api_values::block_state::block_listens(
                mod_api::HookId::MinecartPass,
                state,
            )
        },
        fall_listens: |state| {
            crate::game::api_values::block_state::block_listens(mod_api::HookId::FallOn, state)
        },
        bounce_listens: |state| {
            crate::game::api_values::block_state::block_listens(mod_api::HookId::BounceOn, state)
        },
        inside_fire: |level, dimension, entity, kind, state, pos, precise, staged, writes| {
            // The whole callback is staged: a panic is contained and the walk
            // keeps the caller's collector and write list as they were (#2247's
            // rule: a mod's panic never breaks the model).
            let fired = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                HitLevel(level.reborrow()).native_entity_inside(
                    crate::game::native_hooks::fire::EntityInsideCall {
                        dimension,
                        entity,
                        kind,
                        state,
                        pos,
                        precise,
                        effects: staged,
                        writes,
                    },
                );
            }));
            if let Err(payload) = fired {
                if let Some(text) = payload.downcast_ref::<&str>() {
                    eprintln!("an entity_inside part panicked at {text:?}");
                } else if let Some(text) = payload.downcast_ref::<String>() {
                    eprintln!("an entity_inside part panicked at {text:?}");
                }
            }
        },
        fall_on: |dimension, level, c, distance, damage_multiplier, writes| {
            HitLevel(level.reborrow()).native_fall_on(
                crate::game::native_hooks::fire::FallOnCall {
                    dimension,
                    entity: c.entity,
                    kind: c.kind,
                    state: c.state,
                    pos: c.position,
                    distance,
                    damage_multiplier,
                    writes,
                },
            );
        },
        bounce_on: |dimension, level, c, fall_distance, movement| {
            let mut answer = [movement.x, movement.y, movement.z];
            HitLevel(level.reborrow()).native_bounce_on(
                crate::game::native_hooks::fire::BounceOnCall {
                    dimension,
                    entity: c.entity,
                    kind: c.kind,
                    state: c.state,
                    pos: c.position,
                    fall_distance,
                    movement: &mut answer,
                },
            );
            *movement = sunrise_core::geometry::Vec3::new(answer[0], answer[1], answer[2]);
        },
    };
#[allow(unused_imports)]
pub(in crate::game) use living_death::hurt_server_args;
#[allow(unused_imports)]
pub(in crate::game) use sunrise_client::health::mob_source;

/// Whether a native mod's particles can be sent. Block particle types need
/// a known state (`state_ok`), item particles a validated stack; trail and vibration
/// aren't supported by the network dispatch. A mod's own type must be namespaced.
pub(in crate::game) fn mod_particles_ok(
    options: &ParticleOptions,
    state: Option<BlockStateId>,
    state_ok: impl Fn(BlockStateId) -> bool,
    positions: &[[f64; 3]],
    spread: [f64; 3],
    speed: f64,
) -> bool {
    use ParticleType as T;
    // A native mod's own type: the name has to be namespaced, and a client
    // draws it only when its pack has the sprite set.
    if let Some(name) = &options.mod_particle {
        let namespaced = name
            .split_once(':')
            .is_some_and(|(ns, path)| !ns.is_empty() && !path.is_empty());
        if !namespaced {
            return false;
        }
    }
    let supported = match options.ty {
        T::Block | T::BlockMarker | T::FallingDust | T::DustPillar | T::BlockCrumble => {
            state.is_some_and(state_ok)
        }
        T::Item | T::Trail | T::Vibration => crate::game::item_particle_send::supported(options),
        T::Geyser | T::GeyserPlume | T::GeyserBase | T::GeyserPoof => options
            .geyser
            .is_some_and(|(_, impulse)| impulse.is_finite()),
        _ => true,
    };
    supported
        && positions.iter().flatten().all(|v| v.is_finite())
        && spread
            .into_iter()
            .all(|v| v.is_finite() && v.abs() <= f32::MAX as f64)
        && speed.is_finite()
        && speed.abs() <= f32::MAX as f64
        && options.color.into_iter().all(f32::is_finite)
        && options.to_color.into_iter().all(f32::is_finite)
        && options.scale.is_finite()
}

pub(in crate::game) fn dimension_id(dimension: LevelDimension) -> &'static str {
    match dimension {
        LevelDimension::Overworld => "minecraft:overworld",
        LevelDimension::Nether => "minecraft:the_nether",
        LevelDimension::End => "minecraft:the_end",
        LevelDimension::Custom(key) => key.id(),
    }
}

/// A mob's kind as the hook points name it (`minecraft:zombie`).
pub(in crate::game) fn mob_kind(mob: &sunrise_client::mobs::Mob) -> String {
    crate::game::native_namespaced_id::namespaced(&mob.kind.id)
}

/// The level behaviour's answer, or `None` to try `useWithoutItem`.
/// `ServerPlayerGameMode.useItemOn` lets both FAIL and PASS go to the item.
/// Both success swing sources swing on this server path.
pub(in crate::game) fn block_use_result(
    answer: mod_api::InteractionResult,
) -> Option<sunrise_world::level::InteractionResult> {
    use sunrise_world::level::InteractionResult;
    match answer {
        mod_api::InteractionResult::Success | mod_api::InteractionResult::SuccessServer => {
            Some(InteractionResult::Success)
        }
        mod_api::InteractionResult::Consume => Some(InteractionResult::Consume),
        mod_api::InteractionResult::TryWithEmptyHand => None,
        _ => Some(InteractionResult::Pass),
    }
}

/// What the mods answered a right click on a block (`right_click_block`).
pub(in crate::game) enum BlockClickAnswer {
    /// Cancelled: this answers the click instead of the block and the item.
    Canceled(mod_api::InteractionResult),
    /// The block's use and the item's `useOn` run as these say.
    Go {
        use_block: TriState,
        use_item: TriState,
    },
}

/// What `BonemealEvent` settled for one use of bone meal.
pub(in crate::game) struct BonemealAsk {
    /// `Some(successful)` when a mod canceled the event: `growCrop`'s answer.
    pub crop: Option<bool>,
    /// `isValidBonemealTarget` as the event asked it, when a mod listens:
    /// `growCrop` takes it instead of asking again.
    pub check: Option<BonemealCheck>,
}

/// Where the game calls the hook points.
impl Game<'_> {
    /// `BonemealEvent` for bone meal from `stack` on `pos`, used by the
    /// acting player (`by_player`) or by a dispenser or a villager:
    /// `Some(successful)` when a mod canceled it, the answer `growCrop`
    /// gives in vanilla's place. The caller keeps what the mods did to
    /// `stack`.
    pub(in crate::game) fn bonemeal(
        &mut self,
        pos: BlockPos,
        by_player: bool,
        stack: &mut ItemStack,
    ) -> BonemealAsk {
        if !mod_api::__private::listened(mod_api::HookId::Bonemeal) {
            return BonemealAsk {
                crop: None,
                check: None,
            };
        }
        let client = self.remote.is_some();
        let check = self.with_level(|b, level, _| b.is_valid_bonemeal_target(level, pos, client));
        let valid = check.is_valid();
        let state = self.world.block(pos.x, pos.y, pos.z);
        let (player, player_entity_id) = if by_player {
            let (uuid, entity) = self.native_clicker();
            (Some(uuid), Some(entity))
        } else {
            (None, None)
        };
        let (mut successful, mut canceled) = (false, false);
        let mut snapshot = Some(stack.clone());
        self.native_bonemeal(crate::game::native_hooks::fire::BonemealCall {
            dimension: self.dim.level(),
            player,
            player_entity_id,
            pos,
            state,
            source: BonemealSource::Interaction,
            stack: &mut snapshot,
            valid_bonemeal_target: valid,
            successful: &mut successful,
            canceled: &mut canceled,
        });
        match snapshot {
            Some(next) => *stack = next,
            None => stack.count = 0,
        }
        BonemealAsk {
            crop: canceled.then_some(successful),
            check: Some(check),
        }
    }
    /// Whether a mod canceled `placer` writing `state` at `pos` (`entity_place`):
    /// `placer` is an entity id, `placed_against` the block it goes against and
    /// `companion` the second half of a bed, door or tall plant.
    pub(in crate::game) fn place_canceled(
        &mut self,
        placer: u32,
        pos: BlockPos,
        state: BlockStateId,
        placed_against: BlockStateId,
        companion: Option<(BlockPos, BlockStateId)>,
    ) -> bool {
        if !mod_api::__private::listened(mod_api::HookId::EntityPlace) {
            return false;
        }
        self.placement_canceled(self.entity(placer), pos, state, placed_against, companion)
    }

    pub(in crate::game) fn placement_canceled(
        &mut self,
        placer: Option<EntityRef>,
        pos: BlockPos,
        state: BlockStateId,
        placed_against: BlockStateId,
        extra: impl IntoIterator<Item = (BlockPos, BlockStateId)>,
    ) -> bool {
        let mut extra = extra.into_iter();
        let placement = if let Some(first) = extra.next() {
            let Some(writes) = std::iter::once((pos, state))
                .chain(std::iter::once(first))
                .chain(extra)
                .map(|(at, state)| Some(BlockPlacement::new(at, state.try_into_api().ok()?)))
                .collect()
            else {
                return false;
            };
            Placement::Multi(writes)
        } else {
            Placement::Single
        };
        let mut canceled = false;
        self.native_entity_place(placer, pos, state, placed_against, placement, &mut canceled);
        canceled
    }
    /// Let `entity_place` veto exactly the decision the world will apply. If the
    /// callback changed the inputs, do not choose another target behind its back.
    pub(in crate::game) fn place_item_block(
        &mut self,
        ctx: &PlaceContext,
        by_player: bool,
        stack: &ItemStack,
    ) -> crate::Result<Option<sunrise_world::level::AppliedPlacement>> {
        let registry = self.pack.assets.registry;
        let game_master = ctx.can_use_game_master_blocks;
        let stack_state =
            |state| crate::game::block_items::item_block_state(registry, state, stack, game_master);
        let has_part =
            mod_api::__private::item_listens(mod_api::HookId::PlacementContext, &stack.item);
        let placer = (has_part && by_player)
            .then(|| self.placement_placer())
            .flatten();
        let Some((decision, preview)) = self.with_level(|b, level, ticks| {
            b.decide_placement(level, ticks, ctx, stack_state, |level, context| {
                has_part
                    .then(|| level.rewrite_placement_context(stack, context, placer))
                    .flatten()
            })
        }) else {
            return Ok(None);
        };
        let veto = mod_api::__private::listened(mod_api::HookId::EntityPlace);
        if veto {
            let against_pos = decision.placed_against_pos();
            let block = |game: &Game<'_>, pos: BlockPos| game.world.block(pos.x, pos.y, pos.z);
            let against = block(self, against_pos);
            let target_before = block(self, preview.pos);
            // One proposed state per affected cell. A companion at the pre-clear
            // position supersedes the intermediate air or water write.
            let extra = preview.companion.into_iter().chain(
                preview
                    .pre_clear
                    .filter(|(pos, _)| preview.companion.is_none_or(|(at, _)| at != *pos)),
            );
            let hand = if by_player {
                self.held().cloned()
            } else {
                None
            };
            let placer = by_player
                .then(|| self.native_clicker().1.cast_unsigned())
                .and_then(|entity| self.entity(entity));
            // The block revision and journal epoch cover supporting blocks and
            // block-entity shapes without tracing ordinary reads.
            let level = (self.dim, self.world.revision, self.transactions.writes());
            if self.placement_canceled(placer, preview.pos, preview.state, against, extra)
                || level != (self.dim, self.world.revision, self.transactions.writes())
                || block(self, preview.pos) != target_before
                || block(self, against_pos) != against
                || (by_player && self.held() != hand.as_ref())
            {
                return Ok(None);
            }
        }
        let Some(placed) = self
            .with_level(|b, level, ticks| b.apply_placement(level, ticks, decision, stack_state))
        else {
            return Ok(None);
        };
        if placed.matches_candidate {
            let pos = placed.context.pos;
            let target = [pos.x, pos.y, pos.z];
            self.apply_block_item_data(target, placed.state, stack, game_master);
            self.with_level(|b, level, ticks| b.placement_set_placed_by(level, ticks, &placed));
            let placer = by_player.then(|| self.native_acting_player()).flatten();
            self.set_placed_by(target, placer, Some(stack))?;
            self.block_item_placed(stack, placer, &placed.context);
            if by_player {
                self.trigger_block("minecraft:placed_block", target, stack);
            }
        }
        self.with_level(|b, level, ticks| b.report_placement(level, ticks, &placed));
        Ok(Some(placed))
    }
    /// `MobEvent::PlaceBlock`: a mob sets `state` at `pos` unless a mod
    /// cancels `entity_place`. Enderman attempts use `enderman_place_block`.
    pub(in crate::game) fn mob_place_block(
        &mut self,
        mob: u32,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> crate::Result<()> {
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        let against = self.world.block(pos[0], pos[1] - 1, pos[2]);
        let kind = self
            .mobs
            .iter()
            .find(|m| m.id == mob)
            .map(|m| m.kind.id.clone());
        if self.place_canceled(mob, at, state, against, None) {
            return Ok(());
        }
        let facts = self.res.facts();
        let from = facts
            .get(self.world.block(pos[0], pos[1], pos[2]))
            .name
            .clone();
        let to = facts.get(state).name.clone();
        self.level_set(pos, state)?;
        if let Some(kind) = kind {
            self.native_entity_change_block(mob, &kind, pos, &from, &to);
        }
        Ok(())
    }
    /// `PlayerInteractEvent.RightClickBlock` for a click on `hit` with
    /// `hand` holding `stack`: what the mods let it use.
    pub(in crate::game) fn right_click_block(
        &mut self,
        hit: &super::Hit,
        hand: Hand,
        stack: Option<&ItemStack>,
    ) -> BlockClickAnswer {
        let (mut use_block, mut use_item) = (TriState::Default, TriState::Default);
        if !mod_api::__private::listened(mod_api::HookId::RightClickBlock) {
            return BlockClickAnswer::Go {
                use_block,
                use_item,
            };
        }
        let Ok(snapshot) = stack.try_into_api() else {
            return BlockClickAnswer::Go {
                use_block,
                use_item,
            };
        };
        let click = self.block_click(hit, hand, snapshot.as_ref());
        let (mut result, mut canceled) = (mod_api::InteractionResult::Pass, false);
        self.native_right_click_block(
            &click,
            &mut use_block,
            &mut use_item,
            &mut result,
            &mut canceled,
        );
        if canceled {
            BlockClickAnswer::Canceled(result)
        } else {
            BlockClickAnswer::Go {
                use_block,
                use_item,
            }
        }
    }
    /// The chosen phase's broadcast event, before its scoped behaviour.
    pub(in crate::game) fn use_item_on_block(
        &mut self,
        hit: &super::Hit,
        hand: Hand,
        stack: Option<&ItemStack>,
        phase: UsePhase,
    ) -> Option<mod_api::InteractionResult> {
        if !mod_api::__private::listened(mod_api::HookId::UseItemOnBlock) {
            return None;
        }
        let snapshot = stack.try_into_api().ok()?;
        let click = self.block_click(hit, hand, snapshot.as_ref());
        let (mut result, mut canceled) = (mod_api::InteractionResult::TryWithEmptyHand, false);
        self.native_use_item_on_block(&click, phase, &mut result, &mut canceled);
        if !canceled {
            return None;
        }
        if phase == UsePhase::ItemAfterBlock
            && result.consumes_action()
            && let Some(stack) = stack
        {
            self.with_hand(hand, |game| {
                if game.is_logical_server() {
                    game.award_stat("used", &stack.item, 1);
                }
                game.apply_after_use_components(stack);
            });
        }
        Some(result)
    }
    /// `ItemStack.onItemUseFirst` for a click on a block: the broadcast
    /// event's `ItemBeforeBlock` phase, then the held item's Part. Canceling skips that Part.
    /// Only `Pass` continues to the block.
    pub(in crate::game) fn item_use_first(
        &mut self,
        hit: &super::Hit,
        hand: Hand,
        stack: &ItemStack,
    ) -> Option<mod_api::InteractionResult> {
        if let Some(answer) =
            self.use_item_on_block(hit, hand, Some(stack), UsePhase::ItemBeforeBlock)
        {
            return Some(answer);
        }
        if !mod_api::__private::has_part_hook(mod_api::HookId::OnItemUseFirst) {
            return None;
        }
        let snapshot = stack.try_into_api().ok()?;
        let click = self.block_click(hit, hand, Some(&snapshot));
        let mut answer = None;
        self.native_on_item_use_first(&snapshot, &click, &mut answer);
        answer.filter(|answer| *answer != mod_api::InteractionResult::Pass)
    }
    /// `GameRuleChangedEvent` once the server set the rule `name` to `value`
    /// (`GameRules.set` with a server).
    pub(in crate::game) fn game_rule_changed(
        &mut self,
        name: &str,
        value: GameRuleValue,
        actor: Option<&GameRuleActor<'_>>,
    ) {
        if mod_api::__private::listened(mod_api::HookId::GameRuleChanged) {
            self.native_game_rule_changed(name, value, actor);
        }
    }
    /// `AnimalTameEvent` for the mob at `index`, whose taming roll by the
    /// player `tamer` (`tamer_entity_id`) succeeded: true when a mod cancels
    /// it, and the taming fails.
    pub(in crate::game) fn animal_tame_cancelled(
        &mut self,
        index: usize,
        tamer: u128,
        tamer_entity_id: i32,
    ) -> bool {
        if !mod_api::__private::listened(mod_api::HookId::AnimalTame) {
            return false;
        }
        let m = &self.mobs[index];
        let (entity, kind) = (m.id, mob_kind(m));
        let mut cancel = false;
        self.native_animal_tame(tamer, tamer_entity_id, entity, &kind, &mut cancel);
        cancel
    }
    /// `entity_tame` for the pet at `index`, just tamed by its owner.
    pub(in crate::game) fn pet_tamed(&mut self, index: usize) {
        let m = &self.mobs[index];
        let (entity, kind) = (m.id, mob_kind(m));
        let owner = m
            .extra
            .owner
            .map(sunrise_client::lan::playerdata::ints_uuid);
        self.native_entity_tame(entity, &kind, owner);
    }
    /// The acting player as the hook points name one: the guest whose turn
    /// it is, else the host.
    pub(in crate::game) fn native_acting_id(&mut self) -> u128 {
        sunrise_client::lan::playerdata::ints_uuid(self.acting_player_uuid())
    }

    /// Tells the mods of the chunks that came into this level.
    pub(in crate::game) fn native_chunks_loaded(&mut self) {
        self.fire_pending_block_entity_data_loads();
        let loaded = std::mem::take(&mut self.world.loaded_chunks);
        if !self.is_logical_server() {
            return;
        }
        let dimension = self.dim.id();
        for (x, z, new) in loaded {
            self.native_chunk_load(dimension, [x, z], new);
        }
    }
    /// Tells the mods of the chunks leaving this level, after their last save.
    pub(in crate::game) fn native_chunks_unloaded(
        &mut self,
        unloaded: &[std::sync::Arc<sunrise_client::chunk::Chunk>],
    ) {
        if !self.is_logical_server() {
            return;
        }
        let dimension = self.dim.id();
        for chunk in unloaded {
            self.native_chunk_and_block_entities_unload(dimension, [chunk.x, chunk.z]);
        }
    }
    /// The players still here leave with the server (`PlayerList.removeAll`):
    /// the mods hear it once, whichever path closes it.
    pub(in crate::game) fn native_players_leaving(&mut self) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        if std::mem::replace(&mut host.players_told_left, true) {
            return;
        }
        let players: Vec<(u128, String)> = host
            .hub
            .guests
            .iter()
            .filter(|g| g.placed && g.participates_online())
            .map(|g| (g.uuid, g.name.clone()))
            .collect();
        for (player, name) in players {
            self.native_player_logged_out(player, &name);
        }
    }
    /// The mobs from index `from` on are new in the world.
    pub(in crate::game) fn native_mobs_joined(&mut self, from: usize, natural: bool) {
        // Preserve saved/constructed UUIDs. Natural spawns built with provisional
        // IDs get their persistent identity when admitted, not on their first hit.
        for mob in self.mobs.iter().skip(from) {
            self.persist
                .uuids
                .get_or_insert_with(mob.id, sunrise_client::save::random_uuid);
        }
        self.join_spawn_tags(from);
        // A mod can keep a mob out; it is then taken out and not told of.
        self.native_joins_flush(false, 0);
        for i in from..self.mobs.len() {
            let m = &self.mobs[i];
            let (kind, pos) = (mob_kind(m), [m.position.x, m.position.y, m.position.z]);
            self.native_mob_spawned(self.dim.id(), &kind, pos, natural);
        }
    }
    /// The acting player's blow killed mob `index`.
    pub(in crate::game) fn native_kill_event(&mut self, index: usize) {
        let (entity, kind) = (self.mobs[index].id, mob_kind(&self.mobs[index]));
        let player = self.native_acting_id();
        self.native_mob_killed(player, entity, &kind);
    }
    /// The acting player's hit landed on mob `index`, holding `stack`.
    pub(in crate::game) fn native_hurt_enemy_event(&mut self, stack: &ItemStack, index: usize) {
        let mob = &self.mobs[index];
        let kind = ["minecraft:", mob.kind.id.trim_start_matches("minecraft:")].concat();
        self.native_hurt_enemy(stack, mob.id, &kind);
    }
    /// `Inventory.tick` for the acting player: each carried stack goes through
    /// `native_inventory_tick`.
    pub(in crate::game) fn native_inventory_ticks(&mut self) {
        if self.player.is_none() {
            return;
        }
        for i in 0..self.inventory.len() {
            self.native_slot_tick(i as i32, i == self.selected);
        }
        // Head first here; `Inventory` saves feet as 100 and head as 103.
        for i in 0..self.armor.len() {
            self.native_slot_tick(103 - i as i32, false);
        }
        self.native_slot_tick(-106, false);
    }
    pub(in crate::game) fn native_slot_tick(&mut self, slot: i32, selected: bool) {
        if !mod_api::__private::has_part_hook(mod_api::HookId::InventoryTick) {
            return;
        }
        let Some(stack) = self.native_carried_slot(slot).and_then(|s| s.clone()) else {
            return;
        };
        self.native_inventory_tick(&stack, slot, selected);
        if let Some(stack) = self.native_carried_slot(slot)
            && stack.as_ref().is_some_and(|s| s.count == 0)
        {
            *stack = None;
        }
    }
    /// The slot a player's carried stack is in, by `native_inventory_tick`'s
    /// numbers: none for another number.
    pub(in crate::game) fn native_carried_slot(
        &mut self,
        slot: i32,
    ) -> Option<&mut Option<ItemStack>> {
        match slot {
            0.. if (slot as usize) < self.inventory.len() => self.inventory.get_mut(slot as usize),
            100..=103 => self.armor.get_mut(103 - slot as usize),
            -106 => Some(&mut self.offhand),
            _ => None,
        }
    }
    /// `EntityBlock.newBlockEntity` of block `name`: Sunrise's own, else a
    /// native mod's, made as a write of the level creates it
    /// (`GameLevel::new_block_entity`). Only a declared mod type builds the
    /// level.
    #[cfg(test)]
    pub(in crate::game) fn new_block_entity(&mut self, name: &str) -> Option<BlockEntity> {
        if sunrise_client::block_entity_types::type_for(name).is_none() {
            return block_entity_for_block(
                self.pack.assets.registry,
                self.res.facts(),
                name,
                Compound::default(),
            );
        }
        self.with_level(|_, level, _| level.new_block_entity(name))
    }
    /// Compile static state-factory ownership once per level, including on clients.
    pub(in crate::game) fn ensure_stateful_entity_table(&mut self) {
        if self.world.block_entities.has_stateful_entity_table() {
            return;
        }
        let registry = self.pack.assets.registry;
        let mut blocks = FxHashMap::default();
        let states = (0..registry.len() as u32)
            .map(|id| {
                let Some(state) = registry.state(BlockStateId(id)) else {
                    return false;
                };
                if state.name.starts_with("minecraft:") {
                    return false;
                }
                *blocks.entry(state.name.as_str()).or_insert_with(|| {
                    let mut stateful = false;
                    self.native_stateful_block_entity(&state.name, &mut stateful);
                    stateful
                })
            })
            .collect();
        self.world.block_entities.set_stateful_entities(states);
    }
    /// Create a block entity from an actual state using the level's creation hook.
    pub(in crate::game) fn new_block_entity_for_state(
        &mut self,
        state: BlockStateId,
    ) -> Option<BlockEntity> {
        self.with_level(|_, level, _| level.new_block_entity_for_state(state))
    }
    /// The containers of the mod blocks' block entities, by block state
    /// (`native_block_entity_container`), checked.
    pub(in crate::game) fn native_containers(&self) -> FxHashMap<u32, Arc<ModContainer>> {
        let registry = self.pack.assets.registry;
        let mut by_block: FxHashMap<String, Option<Arc<ModContainer>>> = FxHashMap::default();
        let mut out = FxHashMap::default();
        for i in 0..registry.len() as u32 {
            let Some(state) = registry.state(sunrise_world::registry::BlockStateId(i)) else {
                continue;
            };
            if state.name.starts_with("minecraft:") {
                continue;
            }
            let container = by_block.entry(state.name.clone()).or_insert_with(|| {
                let mut container = None;
                self.native_block_entity_container(&state.name, &mut container);
                container.map(|mut c| {
                    let n = c.slots;
                    for list in c.faces.iter_mut().chain([&mut c.inputs, &mut c.outputs]) {
                        list.retain(|&s| s < n);
                    }
                    c.insert_rules.truncate(n);
                    c.menu = c
                        .menu
                        .filter(|m| mod_menu(m).is_some_and(|(_, size)| size == n));
                    Arc::new(c)
                })
            });
            if let Some(c) = container {
                out.insert(i, Arc::clone(c));
            }
        }
        out
    }
    /// The fluid tanks of the mod blocks' block entities, by block state
    /// (`native_block_entity_fluids`), checked as the containers are.
    pub(in crate::game) fn native_fluid_tables(&self) -> FxHashMap<u32, Arc<ModFluidTanks>> {
        let registry = self.pack.assets.registry;
        let mut by_block: FxHashMap<String, Option<Arc<ModFluidTanks>>> = FxHashMap::default();
        let mut out = FxHashMap::default();
        for i in 0..registry.len() as u32 {
            let Some(state) = registry.state(sunrise_world::registry::BlockStateId(i)) else {
                continue;
            };
            if state.name.starts_with("minecraft:") {
                continue;
            }
            let tanks = by_block.entry(state.name.clone()).or_insert_with(|| {
                let mut fluids = None;
                self.native_block_entity_fluids(&state.name, &mut fluids);
                fluids.map(|mut f| {
                    f.tanks.truncate(16);
                    // Clamped, not dropped: the face and fill lists name
                    // tanks by number, which must not shift.
                    for c in &mut f.tanks {
                        *c = (*c).clamp(1, 1_000_000);
                    }
                    let n = f.tanks.len();
                    for list in f.faces.iter_mut().chain([&mut f.inputs, &mut f.outputs]) {
                        list.retain(|&t| t < n);
                    }
                    Arc::new(f)
                })
            });
            if let Some(t) = tanks {
                out.insert(i, Arc::clone(t));
            }
        }
        out
    }
    /// The energy buffers of the mod blocks' block entities, by block state
    /// (`native_block_entity_energy`), checked as the containers are.
    pub(in crate::game) fn native_energy_tables(&self) -> FxHashMap<u32, Arc<ModEnergyBuffer>> {
        let registry = self.pack.assets.registry;
        let mut by_block: FxHashMap<String, Option<Arc<ModEnergyBuffer>>> = FxHashMap::default();
        let mut out = FxHashMap::default();
        for i in 0..registry.len() as u32 {
            let Some(state) = registry.state(sunrise_world::registry::BlockStateId(i)) else {
                continue;
            };
            if state.name.starts_with("minecraft:") {
                continue;
            }
            let buffer = by_block.entry(state.name.clone()).or_insert_with(|| {
                let mut energy = None;
                self.native_block_entity_energy(&state.name, &mut energy);
                energy.filter(|e| e.capacity >= 1).map(|mut e| {
                    e.max_insert = e.max_insert.clamp(0, e.capacity);
                    e.max_extract = e.max_extract.clamp(0, e.capacity);
                    Arc::new(e)
                })
            });
            if let Some(b) = buffer {
                out.insert(i, Arc::clone(b));
            }
        }
        out
    }
    /// Whether a native mod's block `name` has a ticking block entity.
    pub(in crate::game) fn native_ticks(&self, name: &str) -> bool {
        let mut ticks = false;
        self.native_block_entity_ticks(name, &mut ticks);
        ticks
    }
    /// A mod block's container at `key` opens its menu: its slots grown to
    /// the menu's. False for any other block.
    pub(in crate::game) fn open_mod_menu(&mut self, key: Pos) -> bool {
        let state = self.world.block(key.0, key.1, key.2).0;
        let Some(c) = self.world.block_entities.mod_container(state) else {
            return false;
        };
        let slots = c.slots;
        if c.menu.is_none() {
            return false;
        }
        let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&key) else {
            return false;
        };
        if m.items.len() < slots {
            m.items.resize(slots, None);
        }
        true
    }
    /// The menu type of the mod container at `key`, if it has one.
    pub(in crate::game) fn mod_menu_at(&self, key: Pos) -> Option<&'static str> {
        let state = self.world.block(key.0, key.1, key.2).0;
        let c = self.world.block_entities.mod_container(state)?;
        mod_menu(c.menu.as_deref()?).map(|(menu, _)| menu)
    }
    /// Where the open mod screen puts container slot `slot` this frame: where
    /// a mod last moved it, else where the screen declared it; None when
    /// inactive/hidden.
    pub(in crate::game) fn native_screen_slot(&self, slot: usize) -> Option<(f32, f32)> {
        let s = self.shown_native_screen()?;
        let declared = s
            .layout()
            .slots
            .get(slot)?
            .as_ref()
            .map(|slot| slot.position);
        let [x, y] = s.slot_moves.position(slot, declared)?;
        Some((x, y))
    }
    pub(in crate::game) fn native_screen_slot_icon(&self, slot: usize) -> Option<&str> {
        self.shown_native_screen()?
            .layout()
            .slots
            .get(slot)?
            .as_ref()?
            .no_item_icon
            .as_deref()
    }
    /// Where the open screen of mod menu type `menu` puts container slot
    /// `slot` from the next frame: the mod's last move, else the screen's
    /// declaration; None for an inactive/hidden slot, another menu type's screen or none.
    pub(in crate::game) fn screen_slot(&self, menu: &str, slot: usize) -> Option<[f32; 2]> {
        let s = self.shown_native_screen().filter(|s| s.menu.name == menu)?;
        s.slot_moves.latest(&s.layout().slots, slot)
    }
    /// Moves or activates container slot `slot` of the open screen of menu type
    /// `menu` to `pos` (relative to the screen's origin), or hides it with None,
    /// from the next frame (`SlotMoves::set`).
    pub(in crate::game) fn set_screen_slot(
        &self,
        menu: &str,
        slot: usize,
        pos: Option<[f32; 2]>,
    ) -> Result<(), mod_api::ScreenSlotRefusal> {
        let s = self
            .shown_native_screen()
            .filter(|s| s.menu.name == menu)
            .ok_or(mod_api::ScreenSlotRefusal::NoScreen)?;
        s.slot_moves.set(&s.layout().slots, slot, pos)
    }
    /// The internal container slot base of the open mod screen (`MOD_MENU_SLOT_BASE`
    /// for a `ModMenuType`, `300` for a stock menu).
    pub(in crate::game) fn native_screen_container_slot_base(&self) -> usize {
        self.shown_native_screen()
            .map_or(300, OpenNativeScreen::container_slot_base)
    }
    /// The container slots the open mod screen shows this frame.
    pub(in crate::game) fn native_screen_slots(&self) -> Option<Vec<usize>> {
        Some(
            self.shown_native_screen()?
                .visible_container_slots()
                .collect(),
        )
    }
    /// The first row of the player's inventory on the open mod screen.
    pub(in crate::game) fn native_screen_inventory_y(&self) -> Option<f32> {
        self.shown_native_screen().map(|s| s.layout().inventory_y)
    }
    /// The open mod screen's inventory's left column.
    pub(in crate::game) fn native_screen_inventory_x(&self) -> Option<f32> {
        self.shown_native_screen().map(|s| s.layout().inventory_x)
    }
    /// The open mod screen's width.
    pub(in crate::game) fn native_screen_width(&self) -> Option<f32> {
        self.shown_native_screen().map(|s| s.layout().width)
    }
    /// Menu button `id` on a mod menu (`clickMenuButton`), with the acting
    /// player's cursor stack (`carried`, read and written). A block-backed
    /// menu lends its live resident block entity through `BlockEntityScope`;
    /// a blockless menu lends no view while keeping `host()` and
    /// `transaction(...)` usable.
    pub(in crate::game) fn mod_menu_button(
        &mut self,
        menu: &str,
        context: mod_api::item::menu::MenuButtonContext,
        block: Option<&str>,
        backing: Option<(i32, i32, i32)>,
        carried: &mut Option<ItemStack>,
    ) {
        if !mod_api::__private::has_part_hook(mod_api::HookId::MenuButton) {
            return;
        }
        let Ok(mut api_cursor) = carried.try_into_api() else {
            return;
        };
        let original_api = api_cursor.clone();
        let target_pos = backing.filter(|&pos| self.mod_menu_at(pos) == Some(menu));
        let target = target_pos.and_then(|pos| self.resident_block_entity_target(pos));
        let items_changed = {
            let mut access = match target {
                Some((handle, state)) => ResidentBlockEntityAccess::for_handle(self, handle, state),
                None => ResidentBlockEntityAccess::blockless(self),
            };
            Game::native_menu_button(
                menu,
                block,
                &context,
                mod_api::BlockEntityScope::new(&mut access),
                &mut api_cursor,
            );
            access.items_changed()
        };
        if items_changed
            && let Some((handle, _)) = target
            && let Some(pos) = self.native_block_entity_write_key(handle)
        {
            self.after_commit(crate::game::transactions::After::Changed(pos));
        }
        apply_menu_button_cursor(self.items(), &original_api, &api_cursor, carried);
    }
    /// Options > Mods' rows for loaded code mods, after the content-only rows.
    pub(in crate::game) fn list_native_mods(&mut self) {
        let registered = super::mods::collect_key_mappings(
            self.options.execution_role,
            mod_api::__private::key_mappings(),
        );
        let native = self.mods.native.clone();
        let mut rows: Vec<_> = self
            .settings
            .mods
            .installed
            .iter()
            .filter(|row| {
                mod_api::__private::mod_index(&row.id).is_some()
                    && !native.built().iter().any(|m| m.id == row.id)
            })
            .cloned()
            .collect();
        rows.extend(native.packages().iter().map(|package| {
            let m = &package.manifest;
            self.settings
                .mods
                .info
                .insert(m.id.clone(), sunrise_client::mod_list::ModInfo::of(m, None));
            Installed {
                id: m.id.clone(),
                title: format!("{} {}", m.name, m.version),
                description: m.description.clone(),
                problem: None,
                dev: false,
                native: None,
            }
        }));
        self.settings
            .mods
            .installed
            .retain(|row| !rows.iter().any(|code| code.id == row.id));
        for mut row in rows {
            let settings = self.native_page_settings(&row.id);
            let mappings: Vec<KeyMapping> = registered
                .iter()
                .filter(|(owner, _)| *owner == row.id)
                .map(|(_, k)| KeyMapping {
                    id: k.id.clone(),
                    label: k.label.clone(),
                    key: k.key.clone(),
                    modifiers: k.modifiers.bits,
                    context: crate::game::api_values::TryIntoEngine::try_into_engine(
                        &k.context,
                        &(),
                    )
                    .unwrap(),
                    category: k.category.clone(),
                })
                .collect();
            row.native = Some(NativePage { mappings, settings });
            self.settings.mods.installed.push(row);
        }
    }
    /// The settings mod `mod_id` registered, with their kept values, as its page shows them.
    pub(in crate::game) fn native_page_settings(&self, mod_id: &str) -> Vec<NativeSetting> {
        mod_api::__private::settings()
            .iter()
            .filter(|(owner, _)| *owner == mod_id)
            .map(|(_, spec)| {
                let kept = self.stored_setting(mod_id, &spec.key);
                let value = match (&spec.kind, spec.read(kept.as_deref())) {
                    (_, mod_api::SettingValue::Toggle(on)) => NativeSettingValue::Toggle(on),
                    (
                        mod_api::SettingKind::Choice { options, .. },
                        mod_api::SettingValue::Choice(index),
                    ) => NativeSettingValue::Choice {
                        options: options.clone(),
                        index,
                    },
                    (
                        mod_api::SettingKind::Range { min, max, step, .. },
                        mod_api::SettingValue::Range(value),
                    ) => NativeSettingValue::Range {
                        min: *min,
                        max: *max,
                        step: *step,
                        value,
                    },
                    (_, mod_api::SettingValue::Color(rgb)) => NativeSettingValue::Color(rgb),
                    _ => NativeSettingValue::Toggle(false),
                };
                NativeSetting {
                    key: spec.key.clone(),
                    label: spec.label.clone(),
                    tooltip: spec.tooltip.clone(),
                    value,
                }
            })
            .collect()
    }
    /// Setting `j` of native mod `i`'s page (its index in Options > Mods) changed on the page:
    /// kept as the mod registered it, then the mod is told (`setting_changed`).
    pub(in crate::game) fn native_setting_event(&mut self, i: usize, j: usize) {
        let Some(row) = self.settings.mods.installed.get(i) else {
            return;
        };
        let Some(setting) = row.native.as_ref().and_then(|page| page.settings.get(j)) else {
            return;
        };
        let (id, key) = (row.id.clone(), setting.key.clone());
        let value = match &setting.value {
            NativeSettingValue::Toggle(on) => mod_api::SettingValue::Toggle(*on),
            NativeSettingValue::Choice { index, .. } => mod_api::SettingValue::Choice(*index),
            NativeSettingValue::Range { value, .. } => mod_api::SettingValue::Range(*value),
            NativeSettingValue::Color(rgb) => mod_api::SettingValue::Color(*rgb),
        };
        let kept = mod_api::__private::setting(&id, &key).and_then(|spec| spec.store(value));
        let Some(kept) = kept else {
            return;
        };
        if self.store_setting(&id, &key, &kept) {
            self.native_setting_changed(&id, &key);
        }
    }
    /// Native mod `i`'s page opened: its settings' kept values read again.
    pub(in crate::game) fn native_settings_refresh(&mut self, i: usize) {
        let Some(id) = self.settings.mods.installed.get(i).map(|m| m.id.clone()) else {
            return;
        };
        let settings = self.native_page_settings(&id);
        let row = self.settings.mods.installed.get_mut(i);
        if let Some(page) = row.and_then(|m| m.native.as_mut()) {
            page.settings = settings;
        }
    }
    /// The zoom in effect while playing (a native mod's); the spyglass's own
    /// view wins.
    pub(in crate::game) fn native_zoom_now(&self) -> Option<mod_api::Zoom> {
        if !self.playing() || self.scoping() {
            return None;
        }
        let mut zoom = None;
        self.native_zoom(&mut zoom);
        let z = zoom?;
        let factor = finite(z.factor, 1.0, 10.0);
        (factor > 1.0).then_some(mod_api::Zoom {
            factor,
            smooth: z.smooth,
            turn: if z.turn.is_finite() {
                z.turn.clamp(0.05, 1.0)
            } else {
                1.0
            },
        })
    }
    /// The pose of native mods' camera shake in this frame (`native_camera_shake`),
    /// none while not playing.
    pub(in crate::game) fn native_shake_matrix(&self, partial: f32) -> glam::Mat4 {
        if !self.playing() {
            return glam::Mat4::IDENTITY;
        }
        let mut shake = Shake::default();
        self.native_camera_shake(partial, &mut shake);
        if shake == Shake::default() {
            return glam::Mat4::IDENTITY;
        }
        shake_pose(bounded_shake(shake, self.screen_effect_scale()))
    }

    /// The camera native mods draw this frame from (`native_camera_pose`), starting from
    /// `view`, vanilla's position, yaw and pitch.
    pub(in crate::game) fn native_camera_pose_now(
        &mut self,
        partial: f32,
        view: (glam::DVec3, f32, f32),
    ) -> (glam::DVec3, f32, f32) {
        let (mut position, mut yaw, mut pitch) = (view.0.to_array(), view.1, view.2);
        self.native_camera_pose(partial, &mut position, &mut yaw, &mut pitch);
        let posed = posed_camera(view, position, yaw, pitch);
        if self.apply_requested_camera_type()
            && let Some(refreshed) = self.camera_view(partial)
        {
            // Refresh vanilla components, not the listener's explicit overrides.
            return (
                if posed.0 == view.0 {
                    refreshed.0
                } else {
                    posed.0
                },
                if posed.1 == view.1 {
                    refreshed.1
                } else {
                    posed.1
                },
                if posed.2 == view.2 {
                    refreshed.2
                } else {
                    posed.2
                },
            );
        }
        posed
    }

    /// What native mods hand shader packs in this frame (`native_shader_params`),
    /// with values that aren't finite as 0.
    pub(in crate::game) fn native_shader_params_now(&self, partial: f32) -> [[f32; 4]; 8] {
        let mut params = [[0.0; 4]; 8];
        self.native_shader_params(partial, &mut params);
        finite_params(params)
    }
}

/// Whether F3's debug screen, open when `debug`, covers `corner`
/// (`DebugScreenOverlay.extractRenderState`): its left and right columns
/// the top corners, and its charts the bottom corners `charts` (bottom left,
/// bottom right) says they are drawn in.
pub(in crate::game) fn debug_takes(corner: Corner, debug: bool, charts: [bool; 2]) -> bool {
    debug
        && match corner {
            Corner::TopLeft | Corner::TopRight => true,
            Corner::BottomLeft => charts[0],
            Corner::BottomRight => charts[1],
        }
}

/// A supported type for a block's container menu. A transient container is never block-backed.
pub(in crate::game) fn mod_menu(name: &str) -> Option<(&'static str, usize)> {
    MOD_MENUS
        .into_iter()
        .find(|(menu, _)| *menu == name)
        .or_else(|| {
            sunrise_client::containers::mod_menu_type(name)
                .filter(|menu| menu.owned_slots.is_none())
                .map(|menu| (menu.id, menu.slots))
        })
}

/// `state` with its property `key` set to `value`; none when the block has
/// no such property or value.
pub(in crate::game) fn with_property(
    registry: &BlockRegistry,
    state: BlockStateId,
    key: &str,
    value: &str,
) -> Option<BlockStateId> {
    let mut s = registry.state(state)?.clone();
    match s.properties.get(key) {
        Some(v) if v == value => return Some(state),
        Some(_) => {}
        None => return None,
    }
    s.properties.insert(key.into(), value.into());
    registry.resolve(&s).ok()
}

/// The state a mod's property write changes `state` to; none when the block
/// has no such property or value, or already has it: `LevelChunk.setBlockState`
/// returns null for the same state, so `Level.setBlock` returns false.
pub(in crate::game) fn property_change(
    registry: &BlockRegistry,
    state: BlockStateId,
    key: &str,
    value: &str,
) -> Option<BlockStateId> {
    with_property(registry, state, key, value).filter(|&id| id != state)
}

/// `config/` in the game folder, beside `options.txt`, made when missing.
pub(in crate::game) fn config_dir(game_dir: &Path) -> std::io::Result<PathBuf> {
    let dir = game_dir.join("config");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// An item entity as `native_item_entities` lists it.
/// Read by mods only.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(in crate::game) struct NativeItemEntity {
    /// The entity's ID.
    pub id: u32,
    pub position: [f64; 3],
    pub stack: ItemStack,
    /// Ticks before a player may pick it up (`ItemEntity.pickupDelay`);
    /// hoppers take it whatever this is.
    pub pickup_delay: u32,
}

/// `AABB.intersects`: the boxes overlap, touching faces excluded.
pub(in crate::game) fn box_meets(
    b: &sunrise_core::geometry::Aabb,
    min: [f64; 3],
    max: [f64; 3],
) -> bool {
    let (lo, hi) = (b.min_position(), b.max_position());
    lo.x < max[0]
        && hi.x > min[0]
        && lo.y < max[1]
        && hi.y > min[1]
        && lo.z < max[2]
        && hi.z > min[2]
}

/// What `mod_screen_open(mod_id)` does before running a screen transition
/// (`unavailable`: no renderer, acting for a guest, neither a player nor a
/// title screen, or a world/connection is loading; `owner`: the mod whose
/// `Screen::Mod` is currently shown on top, if any): `Some(false)` refuses
/// without changing state, `Some(true)` keeps `mod_id`'s already open screen,
/// and `None` proceeds through the normal screen-transition path.
pub(in crate::game) fn mod_screen_opens(
    screen: Screen,
    unavailable: bool,
    owner: Option<&str>,
    mod_id: &str,
) -> Option<bool> {
    if unavailable {
        return Some(false);
    }
    if screen == Screen::Mod && owner == Some(mod_id) {
        return Some(true);
    }
    None
}

pub(in crate::game) fn client_packet_view(
    packet: &sunrise_protocol::packets::play::Clientbound,
) -> ClientPacket {
    use mod_api::Identifier;
    use std::sync::LazyLock;
    use sunrise_protocol::packets::play::Clientbound;

    static TYPES: LazyLock<Vec<Identifier>> =
        LazyLock::new(|| packet_types(sunrise_protocol::ids::play::clientbound::NAMES));
    let payload = match packet {
        Clientbound::CustomPayload(payload) => Some(payload),
        _ => None,
    };
    packet_view(&TYPES, packet.id(), payload)
}

/// `client_packet_send`'s view of a packet the game sends: as `client_packet_view`.
pub(in crate::game) fn serverbound_packet_view(
    packet: &sunrise_protocol::packets::play::Serverbound,
) -> ClientPacket {
    use mod_api::Identifier;
    use std::sync::LazyLock;
    use sunrise_protocol::packets::play::Serverbound;

    static TYPES: LazyLock<Vec<Identifier>> =
        LazyLock::new(|| packet_types(sunrise_protocol::ids::play::serverbound::NAMES));
    let payload = match packet {
        Serverbound::CustomPayload(payload) => Some(payload),
        _ => None,
    };
    packet_view(&TYPES, packet.id(), payload)
}

/// One direction's packet type identifiers, by protocol id.
pub(in crate::game) fn packet_types(names: &[&str]) -> Vec<mod_api::Identifier> {
    names
        .iter()
        .map(|name| mod_api::Identifier::parse(name).expect("protocol packet type identifier"))
        .collect()
}

/// A packet's type, `sunrise:unknown` outside the protocol's table, and its custom payload.
pub(in crate::game) fn packet_view(
    types: &[mod_api::Identifier],
    id: i32,
    payload: Option<&sunrise_protocol::packets::configuration::CustomPayload>,
) -> ClientPacket {
    use mod_api::Identifier;
    use std::sync::LazyLock;

    static UNKNOWN: LazyLock<Identifier> =
        LazyLock::new(|| Identifier::parse("sunrise:unknown").unwrap());
    let id = usize::try_from(id)
        .ok()
        .and_then(|id| types.get(id))
        .unwrap_or(&UNKNOWN)
        .clone();
    let payload = payload.map(|p| ClientPayload::new(p.channel.clone(), p.data.clone()));
    ClientPacket::new(id, payload)
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[test]
    #[ignore = "daily"]
    fn declared_types_create_tick_and_reload_through_game() {
        use super::super::native_entity_refs::tests::{record_key, run_fresh_cases, with_game_at};
        use mod_api::{self, *};
        use std::sync::Arc;
        use sunrise_client::{chunk::Chunk, containers::BlockEntity, save};

        const CHILD: &str = "SUNRISE_DECLARED_BE_GAME_CHILD";
        if run_fresh_cases(
            "game::native_hooks::tests::declared_types_create_tick_and_reload_through_game",
            CHILD,
            &["valid", "unknown_block", "unknown_type", "block_not_type"],
        ) {
            return;
        }

        const FIRST: &str = "be_test:press";
        const SECOND: &str = "be_test:ornate_banner";
        const EMPTY: &str = "be_test:blank";
        const PLAIN: &str = "be_test:plain_banner";
        #[derive(Default)]
        struct Keys {
            own: std::sync::OnceLock<BlockEntityTypeKey>,
            shared: std::sync::OnceLock<BlockEntityTypeKey>,
            vanilla: std::sync::OnceLock<BlockEntityTypeKey>,
        }
        trait Probe {
            fn value(&self) -> u32;
        }
        struct Marker(u32);
        impl Probe for Marker {
            fn value(&self) -> u32 {
                self.0
            }
        }
        const PROBE: CapabilityKey<dyn Probe> = CapabilityKey::new("be_test:probe");
        struct Press;
        #[mod_api::sunrise_part]
        impl BlockPart for Press {}
        #[mod_api::sunrise_part]
        impl BlockEntityPart for Press {
            fn block_entity(
                &self,
                _: LevelSide<'_>,
                _: &mut BlockEntityEvent<'_>,
            ) -> Option<Compound> {
                Some(Compound::from([("energy".into(), Tag::Long(6300))]))
            }
            fn block_entity_tick(&self, event: &mut BlockEntityTickEvent<'_>) {
                event.subject.with_view(|mut entity| {
                    let key = "energy".into();
                    let Some(Tag::Long(energy)) = entity.nbt().get(&key) else {
                        panic!("factory payload missing");
                    };
                    assert!(entity.set_nbt(key, Some(Tag::Long(energy + 1))));
                });
            }
        }
        struct User {
            keys: Arc<Keys>,
            unknown: Option<&'static str>,
        }
        #[mod_api::__subscriptions]
        impl Hooks for User {
            fn register(&self, registrar: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                let kind = registrar.block_entity_type_key("be_test:shared");
                registrar
                    .block_entity_part(&kind, Press)
                    .expect("native type");
                registrar.block_entity_capability(&PROBE, &kind, |_, _, _, _, offer| {
                    offer.offer(|| Box::new(Marker(6300)))
                });
                record_key(&self.keys.shared, kind);
                let vanilla = registrar.block_entity_type_key("minecraft:chest");
                registrar.block_entity_capability(&PROBE, &vanilla, |_, _, _, _, offer| {
                    offer.offer(|| Box::new(Marker(26)))
                });
                record_key(&self.keys.vanilla, vanilla);
                if let Some(id) = self.unknown {
                    registrar.block_entity_type_key(id);
                }
            }
        }
        struct TestMod {
            invalid: bool,
            keys: Arc<Keys>,
        }
        #[mod_api::__subscriptions]
        impl Hooks for TestMod {
            fn register(&self, registrar: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                let kind = registrar
                    .block_entity_type("shared")
                    .blocks(if self.invalid {
                        &["be_test:missing"]
                    } else {
                        &[FIRST, SECOND]
                    })
                    .build();
                record_key(&self.keys.own, kind);
                registrar
                    .block_entity_type("empty")
                    .blocks(&[EMPTY])
                    .build();
            }
            fn blocks(&self, _context: &mod_api::RegistrationContext, event: &mut BlocksEvent<'_>) {
                for block in [FIRST, SECOND, EMPTY, PLAIN]
                    .map(|id| mod_api::Block::new(id, "minecraft:white_banner"))
                {
                    event.register(block).unwrap();
                }
            }
        }
        let case = std::env::var(CHILD).unwrap();
        let invalid = case == "unknown_block";
        let unknown = match case.as_str() {
            "unknown_type" => Some("other:missing"),
            "block_not_type" => Some("minecraft:red_shulker_box"),
            _ => None,
        };
        let keys = Arc::new(Keys::default());
        let mods: Mods = Arc::from([
            ModEntry::new(
                "be_user",
                Box::new(User {
                    keys: keys.clone(),
                    unknown,
                }),
            ),
            ModEntry::new(
                "be_test",
                Box::new(TestMod {
                    invalid,
                    keys: keys.clone(),
                }),
            ),
        ]);
        mod_platform::__private::install(mods.clone());
        // `be_user` attaches a Part to the type `be_test` declares, which names `be_test`
        // among its dependencies.
        let package = |id: &str, depends_on: &[&str]| {
            PackageInfo::new(
                id.into(),
                "1".into(),
                PackageKind::Code,
                Side::Both,
                vec![id.into()],
                depends_on
                    .iter()
                    .map(|owner| {
                        PackageDependency::new(
                            (*owner).into(),
                            "*".into(),
                            DependencyKind::Required,
                            DependencyOrder::After,
                            Side::Both,
                            None,
                            None,
                        )
                    })
                    .collect(),
            )
        };
        let base = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        mod_platform::__private::with_test_package_inventory(
            vec![package("be_test", &[]), package("be_user", &["be_test"])],
            || {
                crate::game::api_values::install_parts(&mods).unwrap();
                let root = super::super::mods::content_root(&base, &Default::default());
                if invalid {
                    let error = root.unwrap_err().to_string();
                    assert!(error.contains("be_test:shared"), "{error}");
                    assert!(error.contains("be_test:missing"), "{error}");
                    // A refused declaration builds no content root. The roots folder's own `.lock`
                    // is made by every start and never deleted, so it is not a root.
                    let built: Vec<_> = std::fs::read_dir(".sunrise-cache/mod-content")
                        .into_iter()
                        .flatten()
                        .map(|entry| entry.unwrap().file_name())
                        .filter(|name| name != ".lock")
                        .collect();
                    assert!(built.is_empty(), "{built:?}");
                    return;
                }
                if let Some(id) = unknown {
                    let error = root.unwrap_err().to_string();
                    assert!(error.contains("be_user"), "{error}");
                    assert!(error.contains("unknown block entity type"), "{error}");
                    assert!(error.contains(id), "{error}");
                    assert!(sunrise_client::block_entity_types::current().is_none());
                    return;
                }
                assert_eq!(keys.shared.get(), keys.own.get());
                with_game_at(root.unwrap().prepared().to_path_buf(), |game| {
                    game.options.dedicated = Some(Vec::new());
                    game.world
                        .chunks
                        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                    let level = game.level(game.dim.id()).unwrap();
                    let stone = game
                        .pack
                        .assets
                        .registry
                        .default_state("minecraft:stone")
                        .unwrap();
                    for (x, block) in [
                        (1, FIRST),
                        (2, SECOND),
                        (3, EMPTY),
                        (4, PLAIN),
                        (5, "minecraft:chest"),
                    ] {
                        let state = game.pack.assets.registry.default_state(block).unwrap();
                        assert_eq!(
                            game.level_set_block(level, [x, 63, 0], stone, BlockUpdateFlags::NONE),
                            Ok(true)
                        );
                        assert_eq!(
                            game.level_set_block(level, [x, 64, 0], state, BlockUpdateFlags::NONE),
                            Ok(true)
                        );
                        let pos = (x, 64, 0);
                        if block == PLAIN {
                            // No declared type claims it, and no mod adds it to `minecraft:banner`'s
                            // valid blocks, so its template creates no entity (`validateBlockState`).
                            assert!(game.world.block_entities.get(&pos).is_none());
                            assert!(game.block_entity(level, [x, 64, 0]).is_none());
                            continue;
                        }
                        let handle = game.block_entity(level, [x, 64, 0]).unwrap();
                        if block == "minecraft:chest" {
                            assert_eq!(handle.kind(game).as_ref(), keys.vanilla.get());
                            let probe = level.capability(game, &PROBE, [x, 64, 0], None).unwrap();
                            assert_eq!((probe.value(), probe.owner()), (26, "be_user".into()));
                            continue;
                        }
                        let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get(&pos)
                        else {
                            panic!("declared type did not override its banner template");
                        };
                        if block == EMPTY {
                            assert_eq!(entity.id(), "be_test:empty");
                            assert!(entity.nbt.is_empty());
                            continue;
                        }
                        assert_eq!(entity.id(), "be_test:shared");
                        assert_eq!(entity.nbt.get(&"energy".into()), Some(&Tag::Long(6300)));
                        assert_eq!(handle.kind(game).as_ref(), keys.shared.get());
                        let probe = level.capability(game, &PROBE, [x, 64, 0], None).unwrap();
                        assert_eq!((probe.value(), probe.owner()), (6300, "be_user".into()));
                        game.tick_mod_block_entity(pos);
                        let saved = save::block_entity_encode::encode(
                            pos,
                            game.world.block_entities.get(&pos).unwrap(),
                            block,
                            &Default::default(),
                        );
                        assert_eq!(saved.get("energy"), Some(&Tag::Long(6301)));
                        let (saved_pos, restored) =
                            save::decode_block_entity_for_block(&saved, block).unwrap();
                        assert_eq!(saved_pos, pos);
                        assert_eq!(restored.id(), "be_test:shared");
                        assert!(save::decode_block_entity_for_block(&saved, EMPTY).is_none());
                        game.world.block_entities.insert(pos, restored);
                        game.tick_mod_block_entity(pos);
                        let saved = save::block_entity_encode::encode(
                            pos,
                            game.world.block_entities.get(&pos).unwrap(),
                            block,
                            &Default::default(),
                        );
                        assert_eq!(saved.get("energy"), Some(&Tag::Long(6302)));
                    }

                    // A level hook's `LevelWriteHost::set_block` fires the same hook in its write.
                    let first = game.pack.assets.registry.default_state(FIRST).unwrap();
                    use crate::game::api_values::TryIntoApi;
                    let written = game.with_level(|_, level, ticks| {
                        LevelWriteHost::set_block(
                            &mut super::super::mod_api_host::LevelTicks::new(level, ticks),
                            [5, 64, 0],
                            first.try_into_api().unwrap(),
                            BlockUpdateFlags::NONE,
                        )
                    });
                    assert!(written);
                    let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get(&(5, 64, 0))
                    else {
                        panic!("a level write did not create the declared block entity");
                    };
                    assert_eq!(entity.id(), "be_test:shared");
                    assert_eq!(entity.nbt.get(&"energy".into()), Some(&Tag::Long(6300)));
                });
            },
        );
    }

    #[test]
    fn observed_overlay_prefers_first_open_screen() {
        assert_eq!(
            super::observed_overlay_id(&[(false, "sign"), (false, "book")]),
            None
        );
        assert_eq!(
            super::observed_overlay_id(&[(false, "sign"), (true, "book")]),
            Some("book")
        );
        assert_eq!(
            super::observed_overlay_id(&[(true, "sign"), (true, "book")]),
            Some("sign")
        );
    }

    /// Exercise the gameplay constructor, where name-based entities compete
    /// with a mod block's sign template.
    #[test]
    #[ignore = "daily"]
    fn mod_sign_constructor_prioritizes_templates_over_suffixes() {
        // This fixture adds blocks that are absent from the shared vanilla
        // game's frozen process-wide block definitions.
        if super::super::native_entity_refs::tests::run_fresh_cases(
            "game::native_hooks::tests::mod_sign_constructor_prioritizes_templates_over_suffixes",
            "SUNRISE_SIGN_CONSTRUCTOR_CHILD",
            &["signs"],
        ) {
            return;
        }
        use super::super::{Audio, Game, Options, Progress, Settings, load_resources};
        use sunrise_client::mod_content::{ContentMod, build};
        use sunrise_mod_package::content::{Block, Content};

        let base = std::path::PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES")
                .expect("set SUNRISE_TEST_RESOURCES to prepared resources"),
        );
        let root = std::env::temp_dir().join(format!(
            "sunrise-sign-constructor-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cases = [
            ("ruby:notice_banner", "minecraft:oak_sign", false),
            ("ruby:board_shelf", "minecraft:oak_wall_sign", false),
            ("ruby:lantern_head", "minecraft:oak_hanging_sign", true),
            (
                "ruby:bracket_skull",
                "minecraft:oak_wall_hanging_sign",
                true,
            ),
        ];
        let mut content = Content::declared("ruby");
        let blocks: Vec<_> = cases
            .iter()
            .map(|&(id, template, _)| (id, template))
            .chain([("ruby:road_sign", "minecraft:stone")])
            .map(|(id, template)| Block {
                id: id.into(),
                template: template.into(),
                item: false,
                sign_text_color: (id == "ruby:notice_banner").then(|| "light_blue".into()),
                sign_text_glow: (id == "ruby:notice_banner").then_some(true),
                ..Default::default()
            })
            .collect();
        content.declare(&blocks, &[]).unwrap();
        build(
            &base,
            std::path::Path::new(""),
            &[ContentMod {
                id: "ruby".into(),
                content,
                jars: Vec::new(),
            }],
            &root,
            &sunrise_mod_package::enabled::EnabledMods::new([("ruby".into(), mod_api::Side::Both)]),
        )
        .unwrap();
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            super::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!("no menu handoff"),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = super::ExecutionRole::IntegratedServer;
        let mut game = Game::new(
            &loaded.resources,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.to_path_buf(),
            )),
            None,
        )
        .unwrap();

        for (name, hanging) in cases
            .map(|(name, _, hanging)| (name, hanging))
            .into_iter()
            .chain([
                ("minecraft:oak_sign", false),
                ("minecraft:oak_hanging_sign", true),
            ])
        {
            match game.new_block_entity(name) {
                Some(super::BlockEntity::Sign(sign)) => {
                    assert_eq!(sign.hanging, hanging, "{name}")
                }
                entity => panic!("{name}: expected a sign, got {entity:?}"),
            }
        }
        // `createDefaultSignText`: a declared colour and glow on both sides of a new sign, black
        // and unlit on one that declares none.
        let mut text = |name| match game.new_block_entity(name) {
            Some(super::BlockEntity::Sign(sign)) => {
                assert_eq!(sign.front.color, sign.back.color);
                assert_eq!(sign.front.glowing, sign.back.glowing);
                (sign.front.color, sign.front.glowing)
            }
            entity => panic!("{name}: expected a sign, got {entity:?}"),
        };
        assert_eq!(text("ruby:notice_banner"), (3, true));
        assert_eq!(text("ruby:board_shelf"), (15, false));
        assert_eq!(text("minecraft:oak_sign"), (15, false));
        assert!(game.new_block_entity("ruby:road_sign").is_none());
        assert!(matches!(
            game.new_block_entity("minecraft:white_banner"),
            Some(super::BlockEntity::Banner(_))
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore = "daily"]
    fn generated_fire_matches_dispatch() {
        use super::super::{Audio, Game, Options, Progress, Settings, load_resources};
        use mod_api::{ClientHost, Flow, Hooks, ModEntry};
        use std::sync::{Arc, Mutex};

        #[derive(Default)]
        struct Listener(usize, Arc<Mutex<Vec<usize>>>);
        #[mod_api::sunrise_mod]
        impl Hooks for Listener {
            fn block_highlight(
                &self,
                _: &dyn ClientHost,
                _: &mut mod_api::BlockHighlightEvent,
            ) -> Flow {
                self.1.lock().unwrap().push(self.0);
                if self.0 == 1 {
                    Flow::Cancel
                } else {
                    Flow::Continue
                }
            }
            fn screen_exclusion_areas(
                &self,
                _: &dyn ClientHost,
                event: &mut mod_api::ScreenExclusionAreasEvent<'_>,
            ) {
                self.1.lock().unwrap().push(self.0);
                event.areas.push([self.0 as f32, 0.0, 0.0, 0.0]);
            }
            fn movement_input_update(
                &self,
                _: &dyn ClientHost,
                event: &mut mod_api::MovementInputUpdateEvent<'_>,
            ) {
                self.1.lock().unwrap().push(self.0);
                event.input.move_vector[1] += self.0 as f32;
            }
        }
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let game = Game::new(
            &loaded.resources,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.to_path_buf(),
            )),
            None,
        )
        .unwrap();
        let trace = Arc::new(Mutex::new(Vec::new()));
        let mods = (0..3)
            .map(|i| ModEntry::new("listener", Box::new(Listener(i, trace.clone()))))
            .collect::<Vec<_>>();
        mod_platform::testing::with_mods(mods.into(), || {
            let run = |fire: bool| {
                trace.lock().unwrap().clear();
                let mut canceled = false;
                if fire {
                    game.fire_block_highlight(
                        [1, 2, 3],
                        super::BlockStateId(7),
                        0.5,
                        &mut canceled,
                    );
                } else {
                    mod_api::dispatch_block_highlight(
                        &game,
                        [1, 2, 3],
                        crate::game::api_values::TryIntoApi::try_into_api(&super::BlockStateId(7))
                            .unwrap(),
                        0.5,
                        &mut canceled,
                    );
                }
                let cancel_trace = trace.lock().unwrap().clone();
                trace.lock().unwrap().clear();
                let view = super::ObservedScreen {
                    id: "inventory",
                    overlay: false,
                    mod_screen: None,
                    menu: None,
                    menu_generation: None,
                    dialog_key: None,
                    title: None,
                    rect: [0.0; 4],
                    screen_size: [0.0; 2],
                    slots: 0,
                    mouse: [0.0; 2],
                    partial_tick: 0.0,
                    regions: Vec::new(),
                };
                let mut areas = Vec::new();
                if fire {
                    game.fire_screen_exclusion_areas(&view.view(), &mut areas);
                } else {
                    mod_api::dispatch_screen_exclusion_areas(&game, &view.view(), &mut areas);
                }
                let collect_trace = trace.lock().unwrap().clone();
                trace.lock().unwrap().clear();
                let forward = game.movement_dispatch_fixture(fire);
                (
                    canceled,
                    cancel_trace,
                    areas,
                    collect_trace,
                    forward,
                    trace.lock().unwrap().clone(),
                )
            };
            let old = run(false);
            assert!(old.0);
            assert_eq!(old.1, [0, 1]);
            assert_eq!(old.3, [0, 1, 2]);
            assert_eq!(old.4, 4.0);
            assert_eq!(old, run(true));
        });
    }

    /// With no window to lay a screen out in there is no slot under the pointer.
    #[test]
    #[ignore = "daily"]
    fn no_stack_is_hovered_without_a_window() {
        crate::game::in_own_process(
            "game::native_hooks::tests::no_stack_is_hovered_without_a_window",
            || {
                use super::super::{Audio, Game, Options, Progress, Settings, load_resources};

                let root =
                    std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
                let settings = Settings::default();
                let loaded = load_resources(
                    sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
                    Vec::new(),
                    None,
                    &root,
                    &settings,
                    std::time::Instant::now(),
                    false,
                    crate::game::execution_role::ExecutionRole::IntegratedServer,
                    &Progress::new(),
                    &mut |_, _| unreachable!("no menu handoff"),
                )
                .unwrap();
                let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
                options.resources = root.clone();
                let game = Game::new(
                    &loaded.resources,
                    loaded.pack,
                    loaded.gui.unwrap(),
                    options,
                    settings,
                    Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                        root.to_path_buf(),
                    )),
                    None,
                )
                .unwrap();
                assert_eq!(game.screen_hovered_stack(), None);
            },
        );
    }
}
