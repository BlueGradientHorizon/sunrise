//! Stock layers of living entity renderers (`StockLayer`) and the callback surface for drawing a
//! hidden stock layer from a mod's [`crate::EntityRenderLayer`] (`StockLayerDraw`).

use crate::Hand;

/// One of the four armour slots drawn by vanilla's `HumanoidArmorLayer`, distinct from the item's
/// equipment slot.
// no wire: client-local stock layer selector, not a wire payload.
// exhaustive: these are the four armour slots drawn by HumanoidArmorLayer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArmorLayerSlot {
    /// The helmet layer.
    Head,
    /// The chestplate layer.
    Chest,
    /// The leggings layer.
    Legs,
    /// The boots layer.
    Feet,
}

/// One of the independent passes of a worn humanoid armour piece (`HumanoidArmorLayer` /
/// `EquipmentLayerRenderer`).
// no wire: client-local stock layer selector, not a wire payload.
// exhaustive: base material, enchantment glint and armour trim are the stock armour passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArmorLayerPass {
    /// The armour material, including its base texture layers.
    Base,
    /// The enchantment foil, whether carried by the base material or by the trim.
    Glint,
    /// The armour-trim material.
    Trim,
}

/// A stock layer built into a living entity renderer. Mod-added layers are not stock layers.
///
/// Each variant names the vanilla 26.3 `RenderLayer` class whose step it corresponds to.
/// [`crate::EntityLayers::stock_layers`] lists the variants a target's renderer supports,
/// [`crate::EntityLayers::hide`] removes one from that renderer for the current resource load,
/// and [`StockLayerDraw::draw_stock_layer`] draws a hidden stock layer conditionally from a mod's
/// own [`crate::EntityRenderLayer`].
// no wire: client-local stock layer selector, not a wire payload.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StockLayer {
    /// One pass of a worn humanoid armour piece (`HumanoidArmorLayer` / `EquipmentLayerRenderer`).
    Armor(ArmorLayerSlot, ArmorLayerPass),
    /// Elytra wings worn on the back (`WingsLayer`).
    Wings,
    /// A player's cape (`CapeLayer`).
    Cape,
    /// A non-armour item worn on the head, such as a skull, pumpkin or raid captain banner
    /// (`CustomHeadLayer`).
    CustomHead,
    /// An item held in a hand (`ItemInHandLayer` / `PlayerItemInHandLayer`).
    HeldItem(Hand),
    /// The carved pumpkin worn on a snow golem's head (`SnowGolemHeadLayer`).
    SnowGolemHead,
    /// Parrots perched on a player's shoulders (`ParrotOnShoulderLayer`).
    ParrotOnShoulder,
    /// Arrows stuck in a player's body (`ArrowLayer`).
    Arrow,
    /// Bee stingers stuck in a player's body (`BeeStingerLayer`).
    BeeStinger,
    /// The riptide spin-attack swirl around a player (`SpinAttackEffectLayer`).
    SpinAttackEffect,
    /// The extra ears drawn for the `deadmau5` player name (`Deadmau5EarsLayer`).
    Deadmau5Ears,
}

impl StockLayer {
    const fn bit(self) -> u32 {
        1 << match self {
            Self::Armor(slot, pass) => 3 * slot as u32 + pass as u32,
            Self::Wings => 12,
            Self::Cape => 13,
            Self::CustomHead => 14,
            Self::HeldItem(Hand::Main) => 15,
            Self::HeldItem(Hand::Off) => 16,
            Self::SnowGolemHead => 17,
            Self::ParrotOnShoulder => 18,
            Self::Arrow => 19,
            Self::BeeStinger => 20,
            Self::SpinAttackEffect => 21,
            Self::Deadmau5Ears => 22,
        }
    }
}

/// Draws a hidden stock layer for the entity currently submitted to
/// [`crate::EntityRenderLayer::submit`].
///
/// This is the Rust counterpart of subclassing a vanilla `RenderLayer` and calling `super.render`
/// per entity: hide the stock layer on the target in `register_entity_layers`, then call
/// [`Self::draw_stock_layer`] from your own layer for the entities that should still show it.
pub trait StockLayerDraw {
    /// Submits `layer` for the entity in hand with the same pose, lighting and visibility
    /// conditions the stock draw would have used this frame.
    ///
    /// An empty equipment slot or unmet draw condition draws nothing and returns `Ok(())`.
    /// Returns `Err` if the target's renderer lacks `layer` (including when a mod replaced the
    /// renderer) or if `layer` was not hidden on this target in `register_entity_layers`, since an
    /// unhidden layer has already drawn. Anything drawn by this call rolls back with the rest of
    /// `submit` if the layer returns `Err` or panics.
    fn draw_stock_layer(&mut self, layer: StockLayer) -> Result<(), String>;
}

// hidden: the renderer's fixed-size bitmask of stock layers.
#[doc(hidden)]
// no wire: internal fixed-size bitmask for the client renderer.
// exhaustive: private bitmask field, constructed only through its methods.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StockLayerMask(u32);

impl StockLayerMask {
    /// The empty mask: no stock layers selected.
    pub const EMPTY: Self = Self(0);

    /// Every `StockLayer::Armor(slot, pass)` bit set.
    pub const ALL_ARMOR: Self = Self((1 << 12) - 1);

    /// Whether no bits are set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Inserts `layer` into this mask.
    pub fn insert(&mut self, layer: StockLayer) {
        self.0 |= layer.bit();
    }

    /// Returns a copy of this mask with `layer` set.
    pub const fn with(self, layer: StockLayer) -> Self {
        Self(self.0 | layer.bit())
    }

    /// Returns a copy of this mask with `layer` cleared.
    pub const fn without(self, layer: StockLayer) -> Self {
        Self(self.0 & !layer.bit())
    }

    /// Returns a mask containing only `layer`.
    pub const fn only(layer: StockLayer) -> Self {
        Self(layer.bit())
    }

    /// Returns the union of two masks.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether `layer` is set in this mask.
    pub const fn contains(self, layer: StockLayer) -> bool {
        self.0 & layer.bit() != 0
    }
}
