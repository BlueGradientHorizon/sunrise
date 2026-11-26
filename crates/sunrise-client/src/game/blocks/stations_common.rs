//! Role-neutral workstation state (`Kind`, `Station`) and common `Game` station accessors.
use super::Game;
use sunrise_items::items::ItemStack;
use sunrise_sim::stations::{AnvilResult, EnchantOffers};

/// Slot ids of the open station: inputs from 600, the result at 650.
pub(in crate::game) const INPUT: usize = 600;
pub(in crate::game) const RESULT: usize = 650;

/// `AnvilScreen`: `name.setMaxLength(50)`.
pub(in crate::game) const NAME_CHARS: usize = 50;

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
    /// Stonecutter: the menu's recipe list (sunrise_client::menu) and `selectedRecipeIndex`.
    pub(in crate::game) stone: sunrise_sim::menu::Stonecutter,
    pub(in crate::game) selected: Option<usize>,
    #[allow(dead_code)]
    pub(in crate::game) last_sound: u64,
    /// Villager trading state.
    pub(in crate::game) merchant: Option<Box<crate::game::trading::MerchantState>>,
    /// Beacon menu: the selected primary and secondary power (EFFECTS index).
    pub(in crate::game) beacon: [Option<usize>; 2],
    /// The loom's patterns, selection and scroller (game/blocks/loom.rs).
    pub(in crate::game) loom: crate::game::loom::LoomState,
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
            stone: Default::default(),
            selected: None,
            last_sound: 0,
            merchant: None,
            beacon: [None; 2],
            loom: Default::default(),
        }
    }

    pub(in crate::game) fn new_kind(kind: Kind, pos: [i32; 3]) -> Self {
        Self::new(kind, pos)
    }
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// The anvil name the menu works with (`AnvilScreen.onNameChanged`).
    pub(in crate::game) fn anvil_item_name(&self, s: &Station) -> String {
        match &s.inputs[0] {
            Some(input)
                if input.custom_name().is_none()
                    && s.name
                        == sunrise_game_data::lang::returned_to_server(|| {
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
