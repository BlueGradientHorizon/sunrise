use super::*;
use std::borrow::Cow;
#[cfg(test)]
use std::sync::LazyLock;
use sunrise_client::tooltip_components::TooltipComponentPrepareError;

#[cfg(test)]
static DEFAULT_FONT_ID: LazyLock<mod_api::Identifier> = LazyLock::new(|| {
    mod_api::Identifier::from_parts("minecraft", "default")
        .expect("minecraft:default is a valid identifier")
});

/// Single integer anchor quantization shared by Gather wrapping, `render_tooltip_pre`
/// (`TooltipPlace`), and `DefaultTooltipPositioner::position_tooltip`.
pub(in crate::game) fn quantize_tooltip_anchor(at: (f32, f32)) -> [i32; 2] {
    [at.0.round() as i32, at.1.round() as i32]
}

/// Internal prepared tooltip item retaining engine text and prepared bundle contents so the
/// engine never round-trips original text or bundles through the mod-API boundary on render.
pub(in crate::game) enum PreparedTooltipItem {
    Text {
        text: sunrise_client::text::Text,
        color: u32,
    },
    Bundle {
        stack: ItemStack,
        contents: sunrise_client::bundle::Contents,
    },
    Custom(mod_api::PreparedTooltipComponent),
}

impl PreparedTooltipItem {
    #[cfg(test)]
    pub(super) fn as_text(&self) -> Option<(&sunrise_client::text::Text, u32)> {
        match self {
            Self::Text { text, color } => Some((text, *color)),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn as_bundle(&self) -> Option<&ItemStack> {
        match self {
            Self::Bundle { stack, .. } => Some(stack),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn as_custom(&self) -> Option<&mod_api::PreparedTooltipComponent> {
        match self {
            Self::Custom(component) => Some(component),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn downcast_ref<C: mod_api::ClientTooltipComponent + 'static>(&self) -> Option<&C> {
        self.as_custom().and_then(|c| c.downcast_ref::<C>())
    }
}

/// Outcome of gathering and preparing an item's tooltip components before `render_tooltip_pre`.
pub(in crate::game) enum GatheredTooltip {
    /// Gathered and prepared components plus the item's initial tooltip font (`item_tooltip_font`).
    Ready {
        components: Vec<PreparedTooltipItem>,
        font: mod_api::Identifier,
    },
    /// A `render_tooltip_gather_components` listener explicitly returned `Flow::Cancel`.
    Cancelled,
    /// Preparing a gathered component failed (unregistered custom type, failed factory, or invalid
    /// bundle); vanilla throws `IllegalArgumentException` before Pre/Texture/paint, so the tooltip
    /// aborts cleanly with an error log instead of dropping the component or cancelling.
    PrepareError(TooltipComponentPrepareError),
}

#[cfg(test)]
impl GatheredTooltip {
    pub(super) fn unwrap_ready(self) -> Vec<PreparedTooltipItem> {
        match self {
            Self::Ready { components, .. } => components,
            Self::Cancelled => panic!("expected GatheredTooltip::Ready, got Cancelled"),
            Self::PrepareError(err) => {
                panic!("expected GatheredTooltip::Ready, got PrepareError({err})")
            }
        }
    }
}

/// Unprepared image selected by `Item.getTooltipImage`; a vanilla bundle uses the source stack.
enum RawTooltipImage {
    Mod(mod_api::TooltipComponent),
    VanillaBundle,
}

/// NeoForge `ClientHooks.gatherTooltipComponentsFromElements` wrapping width calculation using
/// Java `int` wrapping arithmetic.
pub(super) fn wrap_tooltip_width(
    mut tooltip_text_width: i32,
    mouse_x: i32,
    screen_width: i32,
    max_width: i32,
) -> (i32, bool) {
    let mut needs_wrap = false;
    let mut tooltip_x = mouse_x.wrapping_add(12);
    if tooltip_x.wrapping_add(tooltip_text_width).wrapping_add(4) > screen_width {
        tooltip_x = mouse_x.wrapping_sub(16).wrapping_sub(tooltip_text_width);
        if tooltip_x < 4 {
            tooltip_text_width = if mouse_x > screen_width / 2 {
                mouse_x.wrapping_sub(12).wrapping_sub(8)
            } else {
                screen_width.wrapping_sub(16).wrapping_sub(mouse_x)
            };
            needs_wrap = true;
        }
    }
    if max_width > 0 && tooltip_text_width > max_width {
        tooltip_text_width = max_width;
        needs_wrap = true;
    }
    (tooltip_text_width, needs_wrap)
}

/// Emit one resolved engine text line into `out`, splitting via `Font::split_formatted_with_font`
/// when `wrap_width` is set and the text is non-empty (empty text stays as one empty line,
/// matching NeoForge `ClientHooks.splitLine`).
fn push_prepared_text(
    font: &sunrise_client::font::Font,
    default_font: &str,
    resolved: sunrise_client::text::Text,
    color: u32,
    wrap_width: Option<i32>,
    out: &mut Vec<PreparedTooltipItem>,
) {
    if let Some(wrap_width) = wrap_width
        && !resolved.is_empty()
    {
        for line in font.split_formatted_with_font(&resolved, wrap_width as f32, default_font) {
            out.push(PreparedTooltipItem::Text { text: line, color });
        }
        return;
    }
    out.push(PreparedTooltipItem::Text {
        text: resolved,
        color,
    });
}

/// Report a tooltip component preparation failure without latching or cancelling.
fn report_tooltip_prepare_error(err: &TooltipComponentPrepareError) {
    log::error!("tooltip component preparation failed: {err}");
}

impl Game<'_> {
    /// `getEmptyBundleDescriptionTextHeight`'s lines: the empty bundle's description wrapped at
    /// the grid's width in `default_font`.
    fn bundle_description_lines(&self, default_font: &str) -> usize {
        let text =
            sunrise_client::text::Text::translate("item.minecraft.bundle.empty.description", &[]);
        self.gui
            .atlas
            .font
            .split_formatted_with_font(&text, 96.0, default_font)
            .len()
    }

    /// `Item.getTooltipImage`: asks the item's `ItemPart::tooltip_image` if one listens, falling
    /// back to vanilla's bundle component when `minecraft:tooltip_display` shows
    /// `minecraft:bundle_contents`.
    fn resolve_raw_tooltip_image(&self, stack: &ItemStack) -> Option<RawTooltipImage> {
        let mut component = None;
        if mod_api::__private::item_listens(mod_api::HookId::TooltipImage, &stack.item) {
            self.native_tooltip_image(stack, &mut component);
            if let Some(component) = component {
                return Some(RawTooltipImage::Mod(component));
            }
        }
        sunrise_client::item_tooltip::shows_bundle_tooltip(self.items(), stack)
            .then_some(RawTooltipImage::VanillaBundle)
    }

    fn prepare_bundle_tooltip(
        &self,
        stack: Cow<'_, ItemStack>,
    ) -> Result<PreparedTooltipItem, TooltipComponentPrepareError> {
        let contents = sunrise_client::bundle::Contents::of(self.items(), &stack)
            .ok_or(TooltipComponentPrepareError::InvalidBundle { owner: None })?;
        Ok(PreparedTooltipItem::Bundle {
            stack: stack.into_owned(),
            contents,
        })
    }

    /// Prepare a [`mod_api::TooltipComponent`] into a [`PreparedTooltipItem`]
    /// (`ClientTooltipComponent.create`): built-in bundles are prepared directly with `Items`,
    /// custom components go through the registered client factory table.
    pub(super) fn prepare_tooltip_component(
        &self,
        component: &mod_api::TooltipComponent,
    ) -> Result<PreparedTooltipItem, TooltipComponentPrepareError> {
        if let Some(api_stack) = component.as_bundle() {
            let engine_stack = crate::game::api_values::stack_value(api_stack).map_err(|_| {
                TooltipComponentPrepareError::InvalidBundle {
                    owner: component.__owner_id(),
                }
            })?;
            return self.prepare_bundle_tooltip(Cow::Owned(engine_stack));
        }
        sunrise_client::tooltip_components::create(component).map(PreparedTooltipItem::Custom)
    }

    /// `AbstractContainerScreen.showTooltipWithItemInHand`: whether this stack's tooltip image
    /// shows while carrying another item, avoiding unnecessary `Contents::of` work for ordinary
    /// bundles and returning `false` on preparation errors.
    pub(super) fn tooltip_image_shows_with_carried(&self, stack: &ItemStack) -> bool {
        match self.resolve_raw_tooltip_image(stack) {
            Some(RawTooltipImage::VanillaBundle) => true,
            Some(RawTooltipImage::Mod(component)) => {
                match self.prepare_tooltip_component(&component) {
                    Ok(PreparedTooltipItem::Bundle { .. }) => true,
                    Ok(PreparedTooltipItem::Custom(prepared)) => {
                        prepared.show_tooltip_with_item_in_hand()
                    }
                    Ok(PreparedTooltipItem::Text { .. }) => false,
                    Err(err) => {
                        report_tooltip_prepare_error(&err);
                        false
                    }
                }
            }
            None => false,
        }
    }

    /// Measure one [`PreparedTooltipItem`] in `tooltip_font` (`font_str` matching `tooltip_font.id`).
    fn measure_prepared_item(
        &self,
        item: &PreparedTooltipItem,
        tooltip_font: &crate::game::host::draw::EngineTooltipFont<'_>,
        font_str: &str,
    ) -> Option<(i32, i32)> {
        Some(match item {
            PreparedTooltipItem::Text { text, .. } => (
                self.gui.atlas.font.width_text_with_font(text, font_str) as i32,
                10,
            ),
            PreparedTooltipItem::Bundle { contents, .. } => {
                let desc_lines = if contents.items().is_empty() {
                    self.bundle_description_lines(font_str)
                } else {
                    0
                };
                (
                    sunrise_client::bundle::Contents::TOOLTIP_WIDTH,
                    contents.tooltip_height(desc_lines, 9),
                )
            }
            PreparedTooltipItem::Custom(component) => (
                component.width(tooltip_font)?,
                component.height(tooltip_font)?,
            ),
        })
    }

    /// `RenderTooltipEvent.GatherComponents` followed by wrapping in the item's tooltip font
    /// (`ClientHooks.getTooltipFont` -> `ClientItemExtensions::tooltip_font`) and component
    /// preparation (`ClientHooks.gatherTooltipComponentsFromElements`).
    pub(super) fn gather_tooltip_components(
        &self,
        stack: &ItemStack,
        lines: Vec<(sunrise_client::text::Text, u32)>,
        mouse_x: i32,
        screen: (f32, f32),
    ) -> GatheredTooltip {
        let gather_listened =
            mod_api::__private::listened(mod_api::HookId::RenderTooltipGatherComponents);
        let item_font = self.item_tooltip_font(stack);
        let item_font_str = crate::game::host::draw::font_id_str(&item_font);
        let raw_image = self.resolve_raw_tooltip_image(stack);
        let font = &self.gui.atlas.font;

        // Build an initial mod-API snapshot for Gather only when listeners are attached. If any
        // engine line or vanilla bundle cannot convert across the local API boundary, skip Gather and
        // render the unmodified engine content directly so no line or bundle is silently dropped.
        let mut vanilla_bundle_snapshot = None;
        let api_elements = if gather_listened {
            let mut snapshot = Vec::with_capacity(lines.len() + usize::from(raw_image.is_some()));
            let mut convertible = true;
            for (text, color) in &lines {
                match text.try_into_api() {
                    Ok(api_text) => snapshot.push(TooltipElement::Text(api_text, *color)),
                    Err(_) => {
                        convertible = false;
                        break;
                    }
                }
            }
            if convertible {
                match &raw_image {
                    Some(RawTooltipImage::Mod(component)) => {
                        let index = usize::from(!snapshot.is_empty());
                        snapshot.insert(index, TooltipElement::Component(component.clone()));
                    }
                    Some(RawTooltipImage::VanillaBundle) => match stack.try_into_api() {
                        Ok(api_stack) => {
                            let bundle = mod_api::TooltipComponent::bundle(api_stack);
                            let index = usize::from(!snapshot.is_empty());
                            snapshot.insert(index, TooltipElement::Component(bundle.clone()));
                            vanilla_bundle_snapshot = Some(bundle);
                        }
                        Err(_) => convertible = false,
                    },
                    None => {}
                }
            }
            convertible.then_some(snapshot)
        } else {
            None
        };

        if let Some(mut elements) = api_elements {
            let (mut max_width, mut cancel) = (-1, false);
            self.native_render_tooltip_gather_components(
                stack,
                screen.0 as i32,
                screen.1 as i32,
                &mut elements,
                &mut max_width,
                &mut cancel,
            );
            if cancel {
                return GatheredTooltip::Cancelled;
            }

            enum DecodedElement {
                Text(sunrise_client::text::Text, u32),
                Component(mod_api::TooltipComponent),
            }

            let mut tooltip_text_width = None;
            let mut decoded = Vec::with_capacity(elements.len());
            for element in elements {
                let (element, width) = match element {
                    TooltipElement::Text(api_text, color) => {
                        let Ok(engine_text) = api_text.try_into_engine(&()) else {
                            continue;
                        };
                        let resolved = engine_text.resolve_legacy();
                        let width = font.width_formatted_with_font(&resolved, &item_font_str);
                        (DecodedElement::Text(resolved, color), width)
                    }
                    TooltipElement::Component(comp) => (DecodedElement::Component(comp), 0),
                };
                tooltip_text_width =
                    Some(tooltip_text_width.map_or(width, |max: i32| max.max(width)));
                decoded.push(element);
            }

            let (wrap_width, needs_wrap) = wrap_tooltip_width(
                tooltip_text_width.unwrap_or(0),
                mouse_x,
                screen.0 as i32,
                max_width,
            );
            let wrap = needs_wrap.then_some(wrap_width);
            let original_bundle = vanilla_bundle_snapshot
                .as_ref()
                .and_then(mod_api::TooltipComponent::as_bundle);
            let mut out = Vec::with_capacity(decoded.len());
            for element in decoded {
                match element {
                    DecodedElement::Text(resolved, color) => {
                        push_prepared_text(font, &item_font_str, resolved, color, wrap, &mut out);
                    }
                    DecodedElement::Component(comp) => {
                        // Unchanged bundle elements (including clones/reorders) retain the shared
                        // snapshot. Prepare the original directly instead of decoding its API view.
                        let prepared = if let Some(original) = original_bundle
                            && comp
                                .as_bundle()
                                .is_some_and(|current| std::ptr::eq(original, current))
                        {
                            self.prepare_bundle_tooltip(Cow::Borrowed(stack))
                        } else {
                            self.prepare_tooltip_component(&comp)
                        };
                        match prepared {
                            Ok(prepared) => out.push(prepared),
                            Err(err) => {
                                report_tooltip_prepare_error(&err);
                                return GatheredTooltip::PrepareError(err);
                            }
                        }
                    }
                }
            }
            return GatheredTooltip::Ready {
                components: out,
                font: item_font,
            };
        }

        // Fast / unconvertible-snapshot engine path: no Gather listeners ran, so preserve every
        // engine text line and vanilla bundle directly without API round-trips.
        let resolved: Vec<(sunrise_client::text::Text, u32)> = lines
            .into_iter()
            .map(|(text, color)| (text.resolve_legacy(), color))
            .collect();
        let tooltip_text_width = resolved
            .iter()
            .map(|(text, _)| font.width_formatted_with_font(text, &item_font_str))
            .chain(raw_image.is_some().then_some(0))
            .max()
            .unwrap_or(0);
        let (wrap_width, needs_wrap) =
            wrap_tooltip_width(tooltip_text_width, mouse_x, screen.0 as i32, -1);
        let wrap = needs_wrap.then_some(wrap_width);
        let prepared_image = match raw_image
            .map(|image| match image {
                RawTooltipImage::VanillaBundle => self.prepare_bundle_tooltip(Cow::Borrowed(stack)),
                RawTooltipImage::Mod(component) => self.prepare_tooltip_component(&component),
            })
            .transpose()
        {
            Ok(prepared) => prepared,
            Err(err) => {
                report_tooltip_prepare_error(&err);
                return GatheredTooltip::PrepareError(err);
            }
        };
        let mut out = Vec::with_capacity(resolved.len() + usize::from(prepared_image.is_some()));
        let mut iter = resolved.into_iter();
        if let Some((first_text, first_color)) = iter.next() {
            push_prepared_text(
                font,
                &item_font_str,
                first_text,
                first_color,
                wrap,
                &mut out,
            );
        }
        if let Some(image) = prepared_image {
            out.push(image);
        }
        for (text, color) in iter {
            push_prepared_text(font, &item_font_str, text, color, wrap, &mut out);
        }
        GatheredTooltip::Ready {
            components: out,
            font: item_font,
        }
    }

    /// `RenderTooltipEvent.Pre` for prepared `components`, hanging from quantized integer anchor
    /// `at`, seeded with `initial_font`: returns the (possibly moved) integer anchor and chosen
    /// font identifier, or `None` when a listener cancelled drawing (custom Pre draws stay in
    /// `batch`).
    pub(in crate::game) fn tooltip_pre_components(
        &self,
        batch: &mut HudBatch,
        stack: Option<&ItemStack>,
        components: &mut [PreparedTooltipItem],
        at: [i32; 2],
        screen: (f32, f32),
        initial_font: mod_api::Identifier,
    ) -> Option<([i32; 2], mod_api::Identifier)> {
        let mut font = initial_font;
        if !mod_api::__private::listened(mod_api::HookId::RenderTooltipPre) {
            return Some((at, font));
        }
        let api_stack = match stack {
            Some(s) => match s.try_into_api() {
                Ok(api) => Some(api),
                Err(_) => return Some((at, font)),
            },
            None => None,
        };

        // Build a complete mod-API snapshot to lend to Pre without losing engine-owned text or
        // bundles. If any item cannot convert, skip Pre cleanly and leave the original engine
        // tooltip intact (consistent with unconvertible stacks).
        struct RestoreCustom<'a> {
            components: &'a mut [PreparedTooltipItem],
            lent: Vec<mod_api::PreparedTooltipComponent>,
        }
        impl Drop for RestoreCustom<'_> {
            fn drop(&mut self) {
                for (slot, item) in self.components.iter_mut().zip(self.lent.drain(..)) {
                    if let PreparedTooltipItem::Custom(dest) = slot {
                        *dest = item;
                    }
                }
            }
        }

        let mut lent = Vec::with_capacity(components.len());
        let mut ok = true;
        for item in components.iter_mut() {
            match item {
                PreparedTooltipItem::Text { text, color } => match text.try_into_api() {
                    Ok(api_text) => {
                        lent.push(mod_api::PreparedTooltipComponent::__text(api_text, *color));
                    }
                    Err(_) => {
                        ok = false;
                        break;
                    }
                },
                PreparedTooltipItem::Bundle { stack, contents } => match stack.try_into_api() {
                    Ok(api_stack) => {
                        let non_empty_height =
                            (!contents.items().is_empty()).then(|| contents.tooltip_height(0, 9));
                        lent.push(mod_api::PreparedTooltipComponent::__bundle(
                            api_stack,
                            non_empty_height,
                        ));
                    }
                    Err(_) => {
                        ok = false;
                        break;
                    }
                },
                PreparedTooltipItem::Custom(custom) => {
                    let placeholder =
                        mod_api::PreparedTooltipComponent::__text(mod_api::Text::literal(""), 0);
                    lent.push(std::mem::replace(custom, placeholder));
                }
            }
        }
        let restore = RestoreCustom { components, lent };
        if !ok {
            drop(restore);
            return Some((at, font));
        }

        let mut place = mod_api::TooltipPlace { x: at[0], y: at[1] };
        let mut cancel = false;
        let mut draw = crate::game::host::draw::GameDrawContext::new(self, batch);
        self.native_render_tooltip_pre(mod_api::__private::RenderTooltipPreCall {
            stack: api_stack.as_ref(),
            components: &restore.lent,
            place: &mut place,
            screen: [screen.0, screen.1],
            positioner: &mod_api::DefaultTooltipPositioner,
            font: &mut font,
            draw: &mut draw,
            cancel: &mut cancel,
        });
        drop(restore);
        if cancel {
            return None;
        }
        Some(([place.x, place.y], font))
    }

    /// `GuiGraphicsExtractor.tooltip`: run Pre, measure in Pre's chosen font without rewrapping,
    /// position with `DefaultTooltipPositioner`, draw background (`render_tooltip_texture`), then
    /// run Pass 1 (`draw_text`) and Pass 2 (`draw_image`). Returns the drawn content rect
    /// `[x, y, w, h]` when drawn.
    pub(in crate::game) fn draw_prepared_tooltip(
        &self,
        batch: &mut HudBatch,
        stack: Option<&ItemStack>,
        components: &mut [PreparedTooltipItem],
        at: [i32; 2],
        initial_font: mod_api::Identifier,
        extra_space_after_first_line: bool,
    ) -> Option<[f32; 4]> {
        if components.is_empty() {
            return None;
        }
        let screen = (batch.width, batch.height);
        let (at, chosen_font) =
            self.tooltip_pre_components(batch, stack, components, at, screen, initial_font)?;

        let tooltip_font = crate::game::host::draw::EngineTooltipFont {
            font: &self.gui.atlas.font,
            id: &chosen_font,
        };
        let chosen_font_str = crate::game::host::draw::font_id_str(&chosen_font);

        let mut w = 0_i32;
        let mut h = if extra_space_after_first_line && components.len() != 1 {
            0_i32
        } else {
            -2_i32
        };
        let mut heights = Vec::with_capacity(components.len());
        for component in components.iter() {
            let (cw, ch) =
                self.measure_prepared_item(component, &tooltip_font, &chosen_font_str)?;
            if cw > w {
                w = cw;
            }
            heights.push(ch);
            h = h.wrapping_add(ch);
        }

        let [x, y] = mod_api::ClientTooltipPositioner::position_tooltip(
            &mod_api::DefaultTooltipPositioner,
            [screen.0 as i32, screen.1 as i32],
            at,
            [w, h],
        );

        // A failed component pass discards this tooltip's entire paint, not Pre listener paint.
        let paint = batch.paint_checkpoint();
        self.tooltip_box(batch, stack, x as f32, y as f32, w as f32, h as f32);

        let mut local_y = y;
        for (i, (component, &ch)) in components.iter().zip(&heights).enumerate() {
            match component {
                PreparedTooltipItem::Text { text, color } => {
                    batch.text_styled_with_font(
                        &self.gui.atlas,
                        text,
                        x as f32,
                        local_y as f32,
                        *color,
                        true,
                        &chosen_font_str,
                    );
                }
                PreparedTooltipItem::Bundle { .. } => {}
                PreparedTooltipItem::Custom(custom) => {
                    let mut draw = crate::game::host::draw::GameDrawContext::new(self, batch);
                    if !custom.__draw_text(&tooltip_font, x, local_y, &mut draw) {
                        drop(draw);
                        paint.discard(batch);
                        return None;
                    }
                }
            }
            local_y =
                local_y
                    .wrapping_add(ch)
                    .wrapping_add(if extra_space_after_first_line && i == 0 {
                        2
                    } else {
                        0
                    });
        }

        let mut local_y = y;
        for (i, (component, &ch)) in components.iter().zip(&heights).enumerate() {
            match component {
                PreparedTooltipItem::Text { .. } => {}
                PreparedTooltipItem::Bundle { contents, .. } => {
                    let description_lines = if contents.items().is_empty() {
                        self.bundle_description_lines(&chosen_font_str)
                    } else {
                        0
                    };
                    let layout = contents.tooltip_layout(
                        x as f32,
                        local_y as f32,
                        w as f32,
                        description_lines,
                    );
                    self.draw_bundle_tooltip(batch, &layout, &chosen_font_str);
                    self.draw_bundle_selected_name(
                        batch,
                        &layout,
                        x as f32,
                        local_y as f32,
                        w as f32,
                        &chosen_font_str,
                    );
                }
                PreparedTooltipItem::Custom(custom) => {
                    let mut draw = crate::game::host::draw::GameDrawContext::new(self, batch);
                    if !custom.__draw_image(&tooltip_font, x, local_y, w, h, &mut draw) {
                        drop(draw);
                        paint.discard(batch);
                        return None;
                    }
                }
            }
            local_y =
                local_y
                    .wrapping_add(ch)
                    .wrapping_add(if extra_space_after_first_line && i == 0 {
                        2
                    } else {
                        0
                    });
        }

        Some([x as f32, y as f32, w as f32, h as f32])
    }

    /// A stack's tooltip: its ordered lines and a bundle's contents
    /// (`ClientBundleTooltip`: the 4 wide grid, the `+N` of what it leaves
    /// out, and the weight bar).
    pub(in crate::game) fn draw_stack_tooltip(
        &mut self,
        batch: &mut HudBatch,
        stack: &ItemStack,
        x: f32,
        y: f32,
    ) {
        self.draw_stack_tooltip_with(batch, stack, false, x, y, Vec::new());
    }

    /// [`Self::draw_stack_tooltip`] with `TooltipFlag.isCreative` set when
    /// `creative` (a creative tab's own slot), and `after_name` inserted from
    /// line 1, as the creative screen adds its tab names.
    pub(in crate::game) fn draw_stack_tooltip_with(
        &mut self,
        batch: &mut HudBatch,
        stack: &ItemStack,
        creative: bool,
        x: f32,
        y: f32,
        after_name: Vec<(sunrise_client::text::Text, u32)>,
    ) {
        let mut lines = self.stack_tooltip_lines(stack, false, creative);
        if lines.is_empty()
            && !sunrise_client::item_tooltip::tooltip_display(stack).name_visible(creative)
        {
            return;
        }
        let after_name_index = lines.len().min(1);
        lines.splice(after_name_index..after_name_index, after_name);
        self.draw_stack_tooltip_lines(batch, stack, lines, x, y);
    }

    /// Draw already-built item lines, keeping the stack's image, style and gather hooks.
    pub(in crate::game) fn draw_stack_tooltip_lines(
        &mut self,
        batch: &mut HudBatch,
        stack: &ItemStack,
        lines: Vec<(sunrise_client::text::Text, u32)>,
        x: f32,
        y: f32,
    ) {
        let anchor = quantize_tooltip_anchor((x, y));
        let GatheredTooltip::Ready {
            mut components,
            font,
        } = self.gather_tooltip_components(stack, lines, anchor[0], (batch.width, batch.height))
        else {
            return;
        };
        self.draw_prepared_tooltip(batch, Some(stack), &mut components, anchor, font, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "daily"]
    fn rendered_image_callback_receives_whole_box() {
        use mod_api::{
            ClientHost, ClientTooltipComponent, ClientTooltipComponentFactoriesRegistrar,
            DrawContext, Hooks, ItemPart, ModEntry, Mods, Registrar,
            RenderTooltipGatherComponentsEvent, TooltipComponent, TooltipComponentData,
            TooltipFont, TooltipImageEvent,
        };
        use std::sync::atomic::{AtomicBool, Ordering};
        use sunrise_client::tooltip_components::{TooltipComponentFactories, with_test_factories};

        static TEXT_DRAWN: AtomicBool = AtomicBool::new(false);
        static IMAGE_DRAWN: AtomicBool = AtomicBool::new(false);

        struct BoxData;
        impl TooltipComponentData for BoxData {}

        struct BoxClient;
        impl ClientTooltipComponent for BoxClient {
            fn height(&self, _: &dyn TooltipFont) -> i32 {
                16
            }
            fn width(&self, _: &dyn TooltipFont) -> i32 {
                70
            }
            fn draw_text(&self, _: &dyn TooltipFont, x: i32, y: i32, _: &mut dyn DrawContext) {
                assert_eq!((x, y), (22, 20));
                assert!(!IMAGE_DRAWN.load(Ordering::Relaxed));
                TEXT_DRAWN.store(true, Ordering::Relaxed);
            }
            fn draw_image(
                &self,
                _: &dyn TooltipFont,
                x: i32,
                y: i32,
                width: i32,
                height: i32,
                _: &mut dyn DrawContext,
            ) {
                assert!(TEXT_DRAWN.load(Ordering::Relaxed));
                assert_eq!((x, y), (22, 20));
                assert_eq!((width, height), (70, 46));
                IMAGE_DRAWN.store(true, Ordering::Relaxed);
            }
        }

        #[derive(Clone)]
        struct Image;
        #[mod_api::sunrise_part]
        impl ItemPart for Image {
            fn tooltip_image(
                &self,
                _: &dyn ClientHost,
                _: &mut TooltipImageEvent<'_>,
            ) -> Option<TooltipComponent> {
                Some(TooltipComponent::custom(BoxData))
            }
        }

        struct Mod;
        #[mod_api::__subscriptions]
        impl Hooks for Mod {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                let key = r.item_key("minecraft:stick");
                r.item_part(key, Image);
            }
            fn render_tooltip_gather_components(
                &self,
                _: &dyn ClientHost,
                event: &mut RenderTooltipGatherComponentsEvent<'_>,
            ) -> mod_api::Flow {
                assert!(matches!(event.elements[1], TooltipElement::Component(_)));
                let component = event.elements[1].clone();
                *event.elements = vec![
                    TooltipElement::Text(mod_api::Text::literal("A"), u32::MAX),
                    component,
                    TooltipElement::Text(mod_api::Text::literal("B"), u32::MAX),
                    TooltipElement::Text(mod_api::Text::literal("C"), u32::MAX),
                ];
                mod_api::Flow::Continue
            }
        }

        let mut factories = TooltipComponentFactories::default();
        let registrar: &mut dyn ClientTooltipComponentFactoriesRegistrar = &mut factories;
        registrar.register::<BoxData, BoxClient>(|_| BoxClient);

        let mods: Mods = vec![ModEntry::new("image", Box::new(Mod))].into();
        mod_platform::testing::with_mods(mods.clone(), || {
            mod_platform::__private::part_tests::install_parts_fixture(&mods).unwrap();
            with_test_factories(factories, || {
                crate::game::native_entity_refs::tests::with_client_game(|game| {
                    let draw = |game: &mut Game<'_>, stack: &ItemStack| {
                        TEXT_DRAWN.store(false, Ordering::Relaxed);
                        IMAGE_DRAWN.store(false, Ordering::Relaxed);
                        game.draw_stack_tooltip(
                            &mut HudBatch::scratch([320, 240]),
                            stack,
                            10.0,
                            20.0,
                        );
                        assert!(TEXT_DRAWN.load(Ordering::Relaxed));
                        assert!(IMAGE_DRAWN.load(Ordering::Relaxed));
                    };
                    draw(game, &ItemStack::new("minecraft:stick", 1));
                    // The component also draws for view-only stacks that cannot enter storage.
                    let mut stack = ItemStack::new("minecraft:stick", 65)
                        .try_into_api()
                        .unwrap();
                    for remove_damage in [false, true] {
                        stack
                            .set_components(
                                [(
                                    sunrise_core::nbt::JavaString::from(if remove_damage {
                                        "!minecraft:max_damage"
                                    } else {
                                        "minecraft:max_damage"
                                    }),
                                    if remove_damage {
                                        sunrise_core::nbt::Tag::Compound(Default::default())
                                    } else {
                                        sunrise_core::nbt::Tag::Int(250)
                                    },
                                )]
                                .into(),
                            )
                            .unwrap();
                        stack.set_damage(251);
                        assert!(stack.try_into_engine(game.items()).is_err());
                        draw(game, &crate::game::api_values::stack_value(&stack).unwrap());
                    }
                });
            });
        });
    }

    #[test]
    fn wrap_tooltip_width_matches_neoforge_client_hooks() {
        // A: fits on the right side without wrapping (including exact screen_width boundary).
        assert_eq!(wrap_tooltip_width(60, 10, 320, -1), (60, false));
        assert_eq!(wrap_tooltip_width(100, 100, 216, -1), (100, false));
        // B: overflows right, flips left cleanly without wrapping (including exact tooltip_x == 4).
        assert_eq!(wrap_tooltip_width(80, 260, 320, -1), (80, false));
        assert_eq!(wrap_tooltip_width(80, 100, 190, -1), (80, false));
        // C: overflows both sides; cursor on right half (mouse_x > screen_width / 2).
        assert_eq!(wrap_tooltip_width(180, 180, 320, -1), (160, true));
        // D: overflows both sides; cursor on left half (mouse_x < screen_width / 2).
        assert_eq!(wrap_tooltip_width(210, 100, 320, -1), (204, true));
        // E: exact midpoint (mouse_x == screen_width / 2) uses the left-half formula.
        assert_eq!(wrap_tooltip_width(200, 160, 320, -1), (144, true));
        // F: explicit positive max_width narrows and wraps; >= width or <= 0 leaves wrapping off.
        assert_eq!(wrap_tooltip_width(100, 10, 320, 40), (40, true));
        assert_eq!(wrap_tooltip_width(100, 10, 320, 100), (100, false));
        assert_eq!(wrap_tooltip_width(100, 10, 320, 140), (100, false));
        assert_eq!(wrap_tooltip_width(100, 10, 320, 0), (100, false));
        // G: max_width tighter than screen wrap narrows further.
        assert_eq!(wrap_tooltip_width(210, 180, 320, 100), (100, true));
        // H: max_width wider than screen wrap keeps screen wrap width.
        assert_eq!(wrap_tooltip_width(210, 180, 320, 180), (160, true));
        // Extreme values never overflow.
        let _ = wrap_tooltip_width(i32::MAX, i32::MAX, i32::MAX, i32::MAX);
        let _ = wrap_tooltip_width(i32::MIN, i32::MIN, i32::MIN, i32::MIN);
    }
}

#[cfg(test)]
#[path = "tooltip_bundle_tests.rs"]
mod bundle_tests;

#[cfg(test)]
#[path = "tooltip_component_tests.rs"]
mod component_tests;
