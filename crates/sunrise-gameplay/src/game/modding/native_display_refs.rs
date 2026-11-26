//! Display handles for native server mods: block, item and text displays.
//!
//! Sunrise keeps a display unsimulated, as the compound vanilla saves
//! (`Display.addAdditionalSaveData`). Reads decode that compound the way the
//! game builds the entity data it sends (so a handle reads what clients see),
//! and a write changes the compound: the next tracking pass sends the changed
//! entries as `SetEntityData`, and the save holds them.
use super::commands::Target;
use mod_api::{
    Billboard, DisplayKind, DisplayRef, EntityRef, ItemDisplayContext, TextAlignment,
    Transformation,
};
use sunrise_game_data::{
    scoreboard::{comp_text, json_to_tag, tag_to_json},
    text::Text,
};
use sunrise_items::items::ItemStack;
use sunrise_protocol::entity_data::Value;
use sunrise_sim::save::item_from_nbt;
use sunrise_sim::text_display_data::{display_data, text_display_data};
use {
    sunrise_core::nbt::{Compound, Tag},
    sunrise_world::registry::BlockStateId,
};

/// The entity data entry `id` of a display's compound: what it sets, else the default.
fn entry(kind: DisplayKind, tag: &Tag, id: u8) -> Option<Value> {
    let find = |tag: &Tag, all: bool| {
        let mut data = display_data(tag, all);
        if kind == DisplayKind::Text {
            data.extend(text_display_data(tag, all));
        }
        data.into_iter().find(|e| e.0 == id).map(|e| e.2)
    };
    find(tag, false).or_else(|| find(&Tag::Compound(Compound::new()), true))
}

fn floats(values: &[f32]) -> Tag {
    Tag::List(values.iter().map(|x| Tag::Float(x.to_bits())).collect())
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|x| x.is_finite())
}

/// A quaternion `Transformation.EXTENDED_CODEC` can normalize.
fn rotation(q: &[f32; 4]) -> bool {
    finite(q) && q.iter().map(|x| x * x).sum::<f32>() > 1.0e-12
}

impl crate::game::ServerGame<'_> {
    /// The display's kind and entity id, or None for a stale handle, a client,
    /// or an entity that isn't a display.
    fn display_entity(&self, display: DisplayRef) -> Option<(DisplayKind, u32)> {
        let Target::Other(id) = self.native_entity_target(display.entity())? else {
            return None;
        };
        let kind = self
            .server
            .others
            .0
            .iter()
            .find(|o| o.id == id)?
            .kind
            .as_str();
        Some((DisplayKind::from_entity_type(kind)?, id))
    }

    /// The display's compound, if it is a display of kind `only` (any when None).
    fn display_tag(
        &self,
        display: DisplayRef,
        only: Option<DisplayKind>,
    ) -> Option<(DisplayKind, Tag)> {
        let (kind, id) = self.display_entity(display)?;
        if only.is_some_and(|only| only != kind) {
            return None;
        }
        let tag = self.common.persist.raw.get(&id).cloned();
        Some((kind, tag.unwrap_or_else(|| Tag::Compound(Compound::new()))))
    }

    fn display_value(
        &self,
        display: DisplayRef,
        only: Option<DisplayKind>,
        id: u8,
    ) -> Option<Value> {
        let (kind, tag) = self.display_tag(display, only)?;
        entry(kind, &tag, id)
    }

    fn display_float(&self, display: DisplayRef, id: u8) -> Option<f32> {
        match self.display_value(display, None, id)? {
            Value::Float(v) => Some(v),
            _ => None,
        }
    }

    fn display_int(&self, display: DisplayRef, only: Option<DisplayKind>, id: u8) -> Option<i32> {
        match self.display_value(display, only, id)? {
            Value::Int(v) => Some(v),
            _ => None,
        }
    }

    fn display_byte(&self, display: DisplayRef, only: DisplayKind, id: u8) -> Option<i8> {
        match self.display_value(display, Some(only), id)? {
            Value::Byte(v) => Some(v),
            _ => None,
        }
    }

    /// Edits the compound of a display of kind `only` (any when None).
    fn display_edit(
        &mut self,
        display: DisplayRef,
        only: Option<DisplayKind>,
        edit: impl FnOnce(&mut Compound),
    ) -> bool {
        let Some((kind, id)) = self.display_entity(display) else {
            return false;
        };
        if only.is_some_and(|only| only != kind) {
            return false;
        }
        let tag = self
            .common.persist
            .raw
            .entry(id)
            .or_insert_with(|| Tag::Compound(Compound::new()));
        let Tag::Compound(c) = tag else {
            return false;
        };
        edit(c);
        true
    }

    fn display_put(
        &mut self,
        display: DisplayRef,
        only: Option<DisplayKind>,
        key: &str,
        value: Tag,
    ) -> bool {
        self.display_edit(display, only, |c| {
            c.insert(key.into(), value);
        })
    }

    fn display_put_float(&mut self, display: DisplayRef, key: &str, value: f32) -> bool {
        value.is_finite() && self.display_put(display, None, key, Tag::Float(value.to_bits()))
    }

    /// Narrows an entity to a display, or None for another kind, a stale
    /// handle or a client.
    pub fn display(&self, entity: EntityRef) -> Option<DisplayRef> {
        let display = DisplayRef::new(entity);
        self.display_entity(display).map(|_| display)
    }

    /// Which `Display` subclass the entity is.
    pub(in crate::game) fn display_kind(&self, display: DisplayRef) -> Option<DisplayKind> {
        Some(self.display_entity(display)?.0)
    }

    /// `Display.createTransformation`: the transformation as shown.
    pub(in crate::game) fn display_transformation(
        &self,
        display: DisplayRef,
    ) -> Option<Transformation> {
        let (kind, tag) = self.display_tag(display, None)?;
        let part = |id| match entry(kind, &tag, id)? {
            Value::Floats(v) => Some(v),
            _ => None,
        };
        Some(Transformation {
            translation: part(11)?.try_into().ok()?,
            scale: part(12)?.try_into().ok()?,
            left_rotation: part(13)?.try_into().ok()?,
            right_rotation: part(14)?.try_into().ok()?,
        })
    }

    /// Sets the transformation. False for a value that isn't finite or a
    /// zero quaternion. Clients ease to it when the start delay is set
    /// (`set_start_interpolation`).
    pub fn display_set_transformation(
        &mut self,
        display: DisplayRef,
        transformation: Transformation,
    ) -> bool {
        let t = transformation;
        if !finite(&t.translation)
            || !finite(&t.scale)
            || !rotation(&t.left_rotation)
            || !rotation(&t.right_rotation)
        {
            return false;
        }
        let tag = Tag::compound([
            ("translation", floats(&t.translation)),
            ("left_rotation", floats(&t.left_rotation)),
            ("scale", floats(&t.scale)),
            ("right_rotation", floats(&t.right_rotation)),
        ]);
        if !self.display_put(display, None, "transformation", tag) {
            return false;
        }
        let index = self
            .server
            .others
            .0
            .iter()
            .position(|o| o.id == display.entity().id())
            .unwrap();
        self.server.others.0.update(index, |o| {
            if o.kind == "minecraft:item_display" {
                o.item_transformation = Some(t.into());
            }
        });
        true
    }

    /// `Display.getBillboardConstraints`.
    pub(in crate::game) fn display_billboard(&self, display: DisplayRef) -> Option<Billboard> {
        match self.display_value(display, None, 15)? {
            Value::Byte(v) => Billboard::ALL.get(usize::try_from(v).ok()?).copied(),
            _ => None,
        }
    }

    /// `Display.setBillboardConstraints`.
    pub(in crate::game) fn display_set_billboard(
        &mut self,
        display: DisplayRef,
        billboard: Billboard,
    ) -> bool {
        let name = Tag::String(billboard.name().into());
        self.display_put(display, None, "billboard", name)
    }

    /// `Display.getBrightnessOverride`: block and sky light, 0 to 15 each, or None for no override.
    pub(in crate::game) fn display_brightness(&self, display: DisplayRef) -> Option<(u8, u8)> {
        let packed = self.display_int(display, None, 16)?;
        (packed != -1).then_some((((packed >> 4) & 15) as u8, ((packed >> 20) & 15) as u8))
    }

    /// `Display.setBrightnessOverride`; None clears the override. False for a
    /// light level above 15.
    pub(in crate::game) fn display_set_brightness(
        &mut self,
        display: DisplayRef,
        brightness: Option<(u8, u8)>,
    ) -> bool {
        if brightness.is_some_and(|(block, sky)| block > 15 || sky > 15) {
            return false;
        }
        self.display_edit(display, None, |c| match brightness {
            Some((block, sky)) => {
                let light = Tag::compound([
                    ("block", Tag::Int(i32::from(block))),
                    ("sky", Tag::Int(i32::from(sky))),
                ]);
                c.insert("brightness".into(), light);
            }
            None => {
                c.remove(&"brightness".into());
            }
        })
    }

    /// `Display.getViewRange`.
    pub(in crate::game) fn display_view_range(&self, display: DisplayRef) -> Option<f32> {
        self.display_float(display, 17)
    }

    /// `Display.setViewRange`. False for a value that isn't finite.
    pub(in crate::game) fn display_set_view_range(
        &mut self,
        display: DisplayRef,
        range: f32,
    ) -> bool {
        self.display_put_float(display, "view_range", range)
    }

    /// `Display.getShadowRadius`.
    pub(in crate::game) fn display_shadow_radius(&self, display: DisplayRef) -> Option<f32> {
        self.display_float(display, 18)
    }

    /// `Display.setShadowRadius`. False for a value that isn't finite.
    pub fn display_set_shadow_radius(
        &mut self,
        display: DisplayRef,
        radius: f32,
    ) -> bool {
        self.display_put_float(display, "shadow_radius", radius)
    }

    /// `Display.getShadowStrength`.
    pub(in crate::game) fn display_shadow_strength(&self, display: DisplayRef) -> Option<f32> {
        self.display_float(display, 19)
    }

    /// `Display.setShadowStrength`. False for a value that isn't finite.
    pub(in crate::game) fn display_set_shadow_strength(
        &mut self,
        display: DisplayRef,
        strength: f32,
    ) -> bool {
        self.display_put_float(display, "shadow_strength", strength)
    }

    /// `Display.getWidth`: the culling box's width, 0 for none.
    pub(in crate::game) fn display_width(&self, display: DisplayRef) -> Option<f32> {
        self.display_float(display, 20)
    }

    /// `Display.setWidth`. False for a value that isn't finite.
    pub(in crate::game) fn display_set_width(&mut self, display: DisplayRef, width: f32) -> bool {
        self.display_put_float(display, "width", width)
    }

    /// `Display.getHeight`: the culling box's height, 0 for none.
    pub(in crate::game) fn display_height(&self, display: DisplayRef) -> Option<f32> {
        self.display_float(display, 21)
    }

    /// `Display.setHeight`. False for a value that isn't finite.
    pub(in crate::game) fn display_set_height(&mut self, display: DisplayRef, height: f32) -> bool {
        self.display_put_float(display, "height", height)
    }

    /// `Display.getGlowColorOverride`: an ARGB color, or -1 for none.
    pub(in crate::game) fn display_glow_color_override(&self, display: DisplayRef) -> Option<i32> {
        self.display_int(display, None, 22)
    }

    /// `Display.setGlowColorOverride`.
    pub(in crate::game) fn display_set_glow_color_override(
        &mut self,
        display: DisplayRef,
        color: i32,
    ) -> bool {
        self.display_put(display, None, "glow_color_override", Tag::Int(color))
    }

    /// `Display.getTransformationInterpolationDuration`, in ticks.
    pub(in crate::game) fn display_interpolation_duration(
        &self,
        display: DisplayRef,
    ) -> Option<i32> {
        self.display_int(display, None, 9)
    }

    /// `Display.setTransformationInterpolationDuration`.
    pub fn display_set_interpolation_duration(
        &mut self,
        display: DisplayRef,
        ticks: i32,
    ) -> bool {
        self.display_put(display, None, "interpolation_duration", Tag::Int(ticks))
    }

    /// `Display.getTransformationInterpolationDelay`: ticks before an
    /// interpolation starts.
    pub(in crate::game) fn display_start_interpolation(&self, display: DisplayRef) -> Option<i32> {
        self.display_int(display, None, 8)
    }

    /// `Display.setTransformationInterpolationDelay`. Like vanilla, every
    /// call is sent to the clients that track the display, even with the
    /// delay it already has, and each client restarts its interpolation.
    pub fn display_set_start_interpolation(
        &mut self,
        display: DisplayRef,
        ticks: i32,
    ) -> bool {
        let Some((_, id)) = self.display_entity(display) else {
            return false;
        };
        // SynchedEntityData.set(DATA_..._START_DELTA_TICKS_ID, ticks, true).
        self.common.persist.force_meta(id, 8);
        self.display_put(display, None, "start_interpolation", Tag::Int(ticks))
    }

    /// `Display.getPosRotInterpolationDuration`: ticks a move eases over, 0 to 59.
    pub(in crate::game) fn display_teleport_duration(&self, display: DisplayRef) -> Option<i32> {
        self.display_int(display, None, 10)
    }

    /// `Display.setPosRotInterpolationDuration`, kept from 0 to 59 as
    /// vanilla's load keeps it.
    pub fn display_set_teleport_duration(
        &mut self,
        display: DisplayRef,
        ticks: i32,
    ) -> bool {
        let ticks = Tag::Int(ticks.clamp(0, 59));
        self.display_put(display, None, "teleport_duration", ticks)
    }

    /// `TextDisplay.getText`; None for another kind of display.
    pub(in crate::game) fn display_text(&self, display: DisplayRef) -> Option<Text> {
        match self.display_value(display, Some(DisplayKind::Text), 23)? {
            Value::Text(tag) => Some(comp_text(&tag_to_json(&tag))),
            _ => None,
        }
    }

    /// `TextDisplay.setText`; false for another kind of display.
    pub(in crate::game) fn display_set_text(&mut self, display: DisplayRef, text: Text) -> bool {
        let text = json_to_tag(&text.component());
        self.display_put(display, Some(DisplayKind::Text), "text", text)
    }

    /// `TextDisplay.getLineWidth`.
    pub(in crate::game) fn display_line_width(&self, display: DisplayRef) -> Option<i32> {
        self.display_int(display, Some(DisplayKind::Text), 24)
    }

    /// `TextDisplay.setLineWidth`; false for another kind of display.
    pub(in crate::game) fn display_set_line_width(
        &mut self,
        display: DisplayRef,
        width: i32,
    ) -> bool {
        self.display_put(
            display,
            Some(DisplayKind::Text),
            "line_width",
            Tag::Int(width),
        )
    }

    /// `TextDisplay.getBackgroundColor`: ARGB.
    pub(in crate::game) fn display_background(&self, display: DisplayRef) -> Option<i32> {
        self.display_int(display, Some(DisplayKind::Text), 25)
    }

    /// `TextDisplay.setBackgroundColor`; false for another kind of display.
    pub(in crate::game) fn display_set_background(
        &mut self,
        display: DisplayRef,
        argb: i32,
    ) -> bool {
        self.display_put(
            display,
            Some(DisplayKind::Text),
            "background",
            Tag::Int(argb),
        )
    }

    /// `TextDisplay.getTextOpacity`: -1 is opaque, else 0 to 127 then -128 to -2.
    pub(in crate::game) fn display_text_opacity(&self, display: DisplayRef) -> Option<i8> {
        self.display_byte(display, DisplayKind::Text, 26)
    }

    /// `TextDisplay.setTextOpacity`; false for another kind of display.
    pub(in crate::game) fn display_set_text_opacity(
        &mut self,
        display: DisplayRef,
        opacity: i8,
    ) -> bool {
        self.display_put(
            display,
            Some(DisplayKind::Text),
            "text_opacity",
            Tag::Byte(opacity),
        )
    }

    fn display_text_flag(&self, display: DisplayRef, bit: i8) -> Option<bool> {
        Some(self.display_byte(display, DisplayKind::Text, 27)? & bit != 0)
    }

    fn display_set_text_flag(&mut self, display: DisplayRef, key: &str, on: bool) -> bool {
        let value = Tag::Byte(i8::from(on));
        self.display_put(display, Some(DisplayKind::Text), key, value)
    }

    /// `TextDisplay.FLAG_SHADOW`.
    pub(in crate::game) fn display_text_shadow(&self, display: DisplayRef) -> Option<bool> {
        self.display_text_flag(display, 1)
    }

    /// Sets the shadow flag; false for another kind of display.
    pub(in crate::game) fn display_set_text_shadow(
        &mut self,
        display: DisplayRef,
        on: bool,
    ) -> bool {
        self.display_set_text_flag(display, "shadow", on)
    }

    /// `TextDisplay.FLAG_SEE_THROUGH`.
    pub(in crate::game) fn display_see_through(&self, display: DisplayRef) -> Option<bool> {
        self.display_text_flag(display, 2)
    }

    /// Sets the see-through flag; false for another kind of display.
    pub(in crate::game) fn display_set_see_through(
        &mut self,
        display: DisplayRef,
        on: bool,
    ) -> bool {
        self.display_set_text_flag(display, "see_through", on)
    }

    /// `TextDisplay.FLAG_USE_DEFAULT_BACKGROUND`.
    pub(in crate::game) fn display_default_background(&self, display: DisplayRef) -> Option<bool> {
        self.display_text_flag(display, 4)
    }

    /// Sets the default-background flag; false for another kind of display.
    pub(in crate::game) fn display_set_default_background(
        &mut self,
        display: DisplayRef,
        on: bool,
    ) -> bool {
        self.display_set_text_flag(display, "default_background", on)
    }

    /// `TextDisplay.getAlign`, from the left and right flags.
    pub(in crate::game) fn display_alignment(&self, display: DisplayRef) -> Option<TextAlignment> {
        let flags = self.display_byte(display, DisplayKind::Text, 27)?;
        Some(match (flags & 8 != 0, flags & 16 != 0) {
            (true, _) => TextAlignment::Left,
            (_, true) => TextAlignment::Right,
            _ => TextAlignment::Center,
        })
    }

    /// Sets the alignment; false for another kind of display.
    pub(in crate::game) fn display_set_alignment(
        &mut self,
        display: DisplayRef,
        alignment: TextAlignment,
    ) -> bool {
        let name = Tag::String(alignment.name().into());
        self.display_put(display, Some(DisplayKind::Text), "alignment", name)
    }

    /// `BlockDisplay.getBlockState`; None for another kind of display.
    pub(in crate::game) fn display_block_state(&self, display: DisplayRef) -> Option<BlockStateId> {
        let (_, tag) = self.display_tag(display, Some(DisplayKind::Block))?;
        tag.get("block_state")
            .and_then(|t| self.state_from_nbt(t))
            .or_else(|| self.common.pack.assets.registry.default_state("minecraft:air"))
    }

    /// `BlockDisplay.setBlockState`; false for another kind of display or an unknown state.
    pub(in crate::game) fn display_set_block_state(
        &mut self,
        display: DisplayRef,
        state: BlockStateId,
    ) -> bool {
        if self.common.pack.assets.registry.state(state).is_none() {
            return false;
        }
        let tag = self.common.state_to_nbt(state);
        self.display_put(display, Some(DisplayKind::Block), "block_state", tag)
    }

    /// `ItemDisplay.getItemStack`; None for another kind of display or an empty stack.
    pub(in crate::game) fn display_item(&self, display: DisplayRef) -> Option<ItemStack> {
        let (_, tag) = self.display_tag(display, Some(DisplayKind::Item))?;
        item_from_nbt(tag.get("item")?)
    }

    /// `ItemDisplay.setItemStack`; None empties it. False for another kind of display.
    pub(in crate::game) fn display_set_item(
        &mut self,
        display: DisplayRef,
        stack: Option<ItemStack>,
    ) -> bool {
        let stack = stack.filter(|s| s.count > 0 && &*s.item != "minecraft:air");
        let items = self.common.res.shared_items();
        self.display_edit(display, Some(DisplayKind::Item), |c| match stack {
            Some(stack) => {
                c.insert(
                    "item".into(),
                    sunrise_sim::save::item_to_nbt(&stack, &items),
                );
            }
            None => {
                c.remove(&"item".into());
            }
        })
    }

    /// `ItemDisplay.getItemTransform`: how the item is displayed.
    pub(in crate::game) fn display_item_context(
        &self,
        display: DisplayRef,
    ) -> Option<ItemDisplayContext> {
        let (_, tag) = self.display_tag(display, Some(DisplayKind::Item))?;
        let name = match tag.get("item_display") {
            Some(Tag::String(s)) => s.to_string_lossy(),
            _ => String::new(),
        };
        Some(ItemDisplayContext::from_name(&name).unwrap_or(ItemDisplayContext::None))
    }

    /// `ItemDisplay.setItemTransform`; false for another kind of display.
    pub(in crate::game) fn display_set_item_context(
        &mut self,
        display: DisplayRef,
        context: ItemDisplayContext,
    ) -> bool {
        let name = Tag::String(context.name().into());
        self.display_put(display, Some(DisplayKind::Item), "item_display", name)
    }
}

use sunrise_gameplay_common::game::tables::WorldTablesExt as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt5 as _;
