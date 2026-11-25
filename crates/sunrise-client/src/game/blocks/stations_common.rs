//! Role-neutral workstation state (`Kind`, `Station`, `EnchantBook`, `CyclingSlot`),
//! shared anvil rename-box helpers, and common `Game` station accessors.
use super::Game;
use sunrise_client::{
    items::ItemStack,
    random_source,
    stations::{AnvilResult, EnchantOffers},
};
use sunrise_core::random::LegacyRandom;

/// Slot ids of the open station: inputs from 600, the result at 650.
pub(in crate::game) const INPUT: usize = 600;
pub(in crate::game) const RESULT: usize = 650;

/// `AnvilScreen`: `name.setMaxLength(50)`.
pub(in crate::game) const NAME_CHARS: usize = 50;

/// `AnvilScreen.init`: the unbordered box of 50 at the menu's name position,
/// focused; `slotChanged` makes it editable once the input slot has an item.
pub(in crate::game) fn rename_box() -> sunrise_client::ui::EditBox {
    let mut edit = sunrise_client::ui::EditBox::new(0.0, 0.0, 103.0, 12.0);
    edit.message = super::screen_info::tr("container.repair", &[]);
    edit.bordered = false;
    edit.max_length = NAME_CHARS;
    edit.text_color = u32::MAX;
    // AnvilScreen also sets `setTextColorUneditable(-1)`: white either way.
    edit.text_color_uneditable = u32::MAX;
    // AnvilScreen sets `setInvertHighlightedTextColor(false)`.
    edit.invert_highlighted_text_color = false;
    edit.focused = true;
    edit.editable = false;
    edit
}

/// The book above the enchanting table's offers: `EnchantmentScreen`'s
/// `time`, `flip`, `oFlip`, `flipT`, `flipA`, `open`, `oOpen` and `last`.
pub(in crate::game) struct EnchantBook {
    pub(in crate::game) time: i32,
    pub(in crate::game) flip: f32,
    pub(in crate::game) o_flip: f32,
    pub(in crate::game) flip_t: f32,
    pub(in crate::game) flip_a: f32,
    pub(in crate::game) open: f32,
    pub(in crate::game) o_open: f32,
    pub(in crate::game) last: Option<ItemStack>,
    pub(in crate::game) random: LegacyRandom,
}

impl EnchantBook {
    pub(in crate::game) fn new(random: LegacyRandom) -> Self {
        Self {
            time: 0,
            flip: 0.0,
            o_flip: 0.0,
            flip_t: 0.0,
            flip_a: 0.0,
            open: 0.0,
            o_open: 0.0,
            last: None,
            random,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::game) enum Kind {
    Enchanting,
    Anvil,
    Grindstone,
    Smithing,
    Stonecutter,
    /// Villager trading (game/items/trading.rs).
    Merchant,
    /// The beacon menu (game/blocks/beacon.rs).
    Beacon,
    /// The cartography table (game/blocks/cartography.rs).
    Cartography,
    /// The loom (game/blocks/loom.rs).
    Loom,
}

impl Kind {
    pub(in crate::game) fn from_block(name: &str) -> Option<Self> {
        Some(match name {
            "minecraft:enchanting_table" => Self::Enchanting,
            "minecraft:anvil" | "minecraft:chipped_anvil" | "minecraft:damaged_anvil" => {
                Self::Anvil
            }
            "minecraft:grindstone" => Self::Grindstone,
            "minecraft:smithing_table" => Self::Smithing,
            "minecraft:stonecutter" => Self::Stonecutter,
            "minecraft:beacon" => Self::Beacon,
            "minecraft:cartography_table" => Self::Cartography,
            "minecraft:loom" => Self::Loom,
            _ => return None,
        })
    }

    pub(in crate::game) fn inputs(self) -> usize {
        match self {
            Self::Smithing | Self::Loom => 3,
            Self::Stonecutter | Self::Beacon => 1,
            _ => 2,
        }
    }

    pub(in crate::game) fn title_key(self) -> &'static str {
        match self {
            Self::Enchanting => "container.enchant",
            Self::Anvil => "container.repair",
            Self::Grindstone => "container.grindstone_title",
            Self::Smithing => "container.upgrade",
            Self::Stonecutter => "container.stonecutter",
            Self::Merchant => "merchant.trades",
            Self::Beacon => "container.beacon",
            Self::Cartography => "container.cartography_table",
            Self::Loom => "container.loom",
        }
    }

    /// Whether the menu has a result slot after its inputs.
    pub(in crate::game) fn has_result(self) -> bool {
        !matches!(self, Self::Enchanting | Self::Beacon)
    }
}

/// `CyclingSlotBackground`: the icons an empty slot cycles through, one every
/// 30 ticks, the next fading in over 4 (`CyclingSlotBackground.tick` in
/// `advance`, `CyclingSlotBackground.extractRenderState` in `shown`).
#[derive(Default)]
pub(in crate::game) struct CyclingSlot {
    pub(in crate::game) icons: Vec<&'static str>,
    pub(in crate::game) tick: u32,
    pub(in crate::game) index: usize,
    /// The client tick it was last advanced to.
    pub(in crate::game) at: Option<u64>,
}

impl CyclingSlot {
    pub(in crate::game) const CHANGE_TICKS: u32 = 30;
    pub(in crate::game) const FADE_TICKS: f32 = 4.0;
}

/// `SmithingScreen`'s `EMPTY_SLOT_SMITHING_TEMPLATES`.
pub(in crate::game) const EMPTY_TEMPLATE_ICONS: [&str; 2] = [
    "smithing_template_armor_trim",
    "smithing_template_netherite_upgrade",
];

pub(in crate::game) struct Station {
    pub(in crate::game) kind: Kind,
    pub(in crate::game) pos: [i32; 3],
    pub(in crate::game) inputs: Vec<Option<ItemStack>>,
    pub(in crate::game) result: Option<ItemStack>,
    pub(in crate::game) offers: EnchantOffers,
    pub(in crate::game) bookcases: i32,
    pub(in crate::game) anvil: AnvilResult,
    /// Anvil rename box text (`AnvilMenu.itemName`).
    pub(in crate::game) name: String,
    /// The box itself (`AnvilScreen`'s `EditBox` of 50, unbordered).
    pub(in crate::game) rename: sunrise_client::ui::EditBox,
    /// Stonecutter: the menu's recipe list (sunrise_client::menu), its
    /// `selectedRecipeIndex` and the screen's first shown index.
    pub(in crate::game) stone: sunrise_client::menu::Stonecutter,
    pub(in crate::game) selected: Option<usize>,
    pub(in crate::game) start: usize,
    /// Stonecutter: the screen's `scrollOffs` (0 to 1) and `scrolling`, the
    /// scroller held by the mouse.
    pub(in crate::game) scroll: f32,
    pub(in crate::game) scrolling: bool,
    /// Anvil: the input the rename box was last filled from.
    pub(in crate::game) anvil_last: Option<ItemStack>,
    /// Anvil on a server: the name last sent (the client menu's `itemName`).
    pub(in crate::game) anvil_sent: Option<String>,
    pub(in crate::game) last_sound: u64,
    /// Villager trading state.
    pub(in crate::game) merchant: Option<Box<crate::game::trading::MerchantState>>,
    /// Beacon menu: the selected primary and secondary power (EFFECTS index).
    pub(in crate::game) beacon: [Option<usize>; 2],
    /// A server's beacon menu: the pyramid's levels (its data slot 0).
    pub(in crate::game) beacon_levels: Option<i32>,
    /// The loom's patterns, selection and scroller (game/blocks/loom.rs).
    pub(in crate::game) loom: crate::game::loom::LoomState,
    /// The smithing table's three `CyclingSlotBackground`s: the template, base and additions slots.
    pub(in crate::game) cycling: [CyclingSlot; 3],
    /// The enchanting table's book (`EnchantmentScreen`'s flip and open state).
    pub(in crate::game) book: EnchantBook,
}

impl Station {
    pub(in crate::game) fn new(kind: Kind, pos: [i32; 3]) -> Self {
        Self {
            kind,
            pos,
            inputs: vec![None; kind.inputs()],
            result: None,
            offers: EnchantOffers::default(),
            bookcases: 0,
            anvil: AnvilResult::default(),
            name: String::new(),
            rename: rename_box(),
            stone: Default::default(),
            selected: None,
            start: 0,
            scroll: 0.0,
            scrolling: false,
            anvil_last: None,
            anvil_sent: None,
            last_sound: 0,
            merchant: None,
            beacon: [None; 2],
            beacon_levels: None,
            loom: Default::default(),
            cycling: Default::default(),
            book: EnchantBook::new(random_source::create()),
        }
    }

    pub(in crate::game) fn new_kind(kind: Kind, pos: [i32; 3]) -> Self {
        Self::new(kind, pos)
    }
}

impl Game<'_> {
    /// The anvil name the menu works with (`AnvilScreen.onNameChanged`).
    pub(in crate::game) fn anvil_item_name(&self, s: &Station) -> String {
        match &s.inputs[0] {
            Some(input)
                if input.custom_name().is_none()
                    && s.name
                        == sunrise_client::lang::returned_to_server(|| {
                            self.stack_hover_name_text(input).plain()
                        }) =>
            {
                String::new()
            }
            Some(_) => s.name.clone(),
            None => String::new(),
        }
    }

    /// `slotsChanged` / `createResult` for the open station.
    pub(in crate::game) fn station_changed(&mut self) {
        self.side_station_changed();
    }

    pub(in crate::game) fn max_stack(&self, stack: &ItemStack) -> u32 {
        self.res
            .items()
            .get(&stack.item)
            .map_or(64, |i| i.max_stack)
    }
}
