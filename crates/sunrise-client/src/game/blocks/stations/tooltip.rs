use super::*;
use std::sync::LazyLock;

static DEFAULT_FONT_ID: LazyLock<mod_api::Identifier> = LazyLock::new(|| {
    mod_api::Identifier::from_parts("minecraft", "default")
        .expect("minecraft:default is a valid identifier")
});

/// NeoForge `ClientHooks.gatherTooltipComponentsFromElements` wrapping width calculation.
pub(super) fn wrap_tooltip_width(
    mut tooltip_text_width: i32,
    mouse_x: i32,
    screen_width: i32,
    max_width: i32,
) -> (i32, bool) {
    let mut needs_wrap = false;
    let mut tooltip_x = mouse_x.saturating_add(12);
    if tooltip_x
        .saturating_add(tooltip_text_width)
        .saturating_add(4)
        > screen_width
    {
        tooltip_x = mouse_x
            .saturating_sub(16)
            .saturating_sub(tooltip_text_width);
        if tooltip_x < 4 {
            tooltip_text_width = if mouse_x > screen_width / 2 {
                mouse_x.saturating_sub(12).saturating_sub(8)
            } else {
                screen_width.saturating_sub(16).saturating_sub(mouse_x)
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

/// Emit one resolved text line into `out`, splitting via `Font::split_formatted` when
/// `wrap_width` is set and the text is non-empty (empty text stays as one empty line,
/// matching NeoForge `ClientHooks.splitLine`).
fn push_prepared_text(
    font: &sunrise_client::font::Font,
    resolved: sunrise_client::text::Text,
    color: u32,
    wrap_width: Option<i32>,
    out: &mut Vec<mod_api::PreparedTooltipComponent>,
) {
    if let Some(wrap_width) = wrap_width
        && !resolved.is_empty()
    {
        for line in font.split_formatted(&resolved, wrap_width as f32) {
            if let Ok(api_line) = line.try_into_api() {
                out.push(mod_api::PreparedTooltipComponent::__text(api_line, color));
            }
        }
        return;
    }
    if let Ok(api_text) = resolved.try_into_api() {
        out.push(mod_api::PreparedTooltipComponent::__text(api_text, color));
    }
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
    pub(super) fn resolve_item_tooltip_image(
        &self,
        stack: &ItemStack,
    ) -> Option<mod_api::TooltipComponent> {
        let mut component = None;
        if mod_api::__private::item_listens(mod_api::HookId::TooltipImage, &stack.item) {
            self.native_tooltip_image(stack, &mut component);
            if component.is_some() {
                return component;
            }
        }
        if sunrise_client::item_tooltip::tooltip_display(stack).shows("minecraft:bundle_contents")
            && sunrise_client::bundle::Contents::of(self.items(), stack).is_some()
        {
            return stack
                .try_into_api()
                .ok()
                .map(mod_api::TooltipComponent::bundle);
        }
        None
    }

    /// Prepare a [`mod_api::TooltipComponent`] into a [`mod_api::PreparedTooltipComponent`]
    /// (`ClientTooltipComponent.create`): built-in bundles are prepared directly with `Items`,
    /// custom components go through the registered client factory table.
    pub(super) fn prepare_tooltip_component(
        &self,
        component: &mod_api::TooltipComponent,
    ) -> Option<mod_api::PreparedTooltipComponent> {
        if let Some(api_stack) = component.as_bundle() {
            let engine_stack = crate::game::api_values::stack_value(api_stack).ok()?;
            let contents = sunrise_client::bundle::Contents::of(self.items(), &engine_stack)?;
            let grid_rows = if contents.items().is_empty() {
                0
            } else {
                contents.items().len().min(12).div_ceil(4) as i32
            };
            return Some(mod_api::PreparedTooltipComponent::__bundle(
                api_stack.clone(),
                grid_rows,
            ));
        }
        sunrise_client::tooltip_components::create(component)
    }

    /// `RenderTooltipEvent.GatherComponents` followed by wrapping (`Font::DEFAULT`) and
    /// component preparation (`ClientHooks.gatherTooltipComponentsFromElements`).
    /// Returns `None` only when a listener explicitly cancels Gather.
    pub(super) fn gather_tooltip_components(
        &self,
        stack: &ItemStack,
        lines: Vec<(sunrise_client::text::Text, u32)>,
        mouse_x: i32,
        screen: (f32, f32),
    ) -> Option<Vec<mod_api::PreparedTooltipComponent>> {
        let gather_listened =
            mod_api::__private::listened(mod_api::HookId::RenderTooltipGatherComponents);
        let image_listened =
            mod_api::__private::item_listens(mod_api::HookId::TooltipImage, &stack.item);
        let font = &self.gui.atlas.font;

        if !gather_listened
            && !image_listened
            && sunrise_client::bundle::Contents::of(self.items(), stack).is_none()
        {
            let resolved: Vec<(sunrise_client::text::Text, u32)> = lines
                .into_iter()
                .map(|(text, color)| (text.resolve_legacy(), color))
                .collect();
            let tooltip_text_width = resolved
                .iter()
                .map(|(text, _)| font.width_text(text) as i32)
                .max()
                .unwrap_or(0);
            let (wrap_width, needs_wrap) =
                wrap_tooltip_width(tooltip_text_width, mouse_x, screen.0 as i32, -1);
            let wrap = needs_wrap.then_some(wrap_width);
            let mut out = Vec::with_capacity(resolved.len());
            for (text, color) in resolved {
                push_prepared_text(font, text, color, wrap, &mut out);
            }
            return Some(out);
        }

        let mut elements: Vec<TooltipElement> = lines
            .into_iter()
            .filter_map(|(text, color)| {
                text.try_into_api()
                    .ok()
                    .map(|text| TooltipElement::Text(text, color))
            })
            .collect();
        if let Some(image) = self.resolve_item_tooltip_image(stack) {
            let index = usize::from(!elements.is_empty());
            elements.insert(index, TooltipElement::Component(image));
        }

        let (mut max_width, mut cancel) = (-1, false);
        if gather_listened {
            self.native_render_tooltip_gather_components(
                stack,
                screen.0 as i32,
                screen.1 as i32,
                &mut elements,
                &mut max_width,
                &mut cancel,
            );
            if cancel {
                return None;
            }
        }

        enum DecodedElement {
            Text(sunrise_client::text::Text, u32),
            Component(mod_api::TooltipComponent),
        }

        let mut tooltip_text_width = 0_i32;
        let mut decoded = Vec::with_capacity(elements.len());
        for element in elements {
            match element {
                TooltipElement::Text(api_text, color) => {
                    let Ok(engine_text) = api_text.try_into_engine(&()) else {
                        continue;
                    };
                    let resolved = engine_text.resolve_legacy();
                    tooltip_text_width = tooltip_text_width.max(font.width_text(&resolved) as i32);
                    decoded.push(DecodedElement::Text(resolved, color));
                }
                TooltipElement::Component(comp) => {
                    decoded.push(DecodedElement::Component(comp));
                }
            }
        }

        let (wrap_width, needs_wrap) =
            wrap_tooltip_width(tooltip_text_width, mouse_x, screen.0 as i32, max_width);
        let wrap = needs_wrap.then_some(wrap_width);
        let mut out = Vec::with_capacity(decoded.len());
        for element in decoded {
            match element {
                DecodedElement::Text(resolved, color) => {
                    push_prepared_text(font, resolved, color, wrap, &mut out);
                }
                DecodedElement::Component(comp) => {
                    if let Some(prepared) = self.prepare_tooltip_component(&comp) {
                        out.push(prepared);
                    }
                }
            }
        }
        Some(out)
    }

    /// `RenderTooltipEvent.Pre` for prepared `components`, hanging from `at`: returns the
    /// (possibly moved) anchor and chosen font identifier, or `None` when a listener cancelled
    /// drawing (custom Pre draws stay in `batch`).
    pub(in crate::game) fn tooltip_pre_components(
        &self,
        batch: &mut HudBatch,
        stack: Option<&ItemStack>,
        components: &[mod_api::PreparedTooltipComponent],
        at: (f32, f32),
        screen: (f32, f32),
    ) -> Option<((f32, f32), mod_api::Identifier)> {
        let mut font = DEFAULT_FONT_ID.clone();
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
        let mut place = mod_api::TooltipPlace {
            x: at.0 as i32,
            y: at.1 as i32,
        };
        let before = place;
        let mut cancel = false;
        let mut draw = crate::game::host::draw::GameDrawContext::new(self, batch);
        self.native_render_tooltip_pre(mod_api::__private::RenderTooltipPreCall {
            stack: api_stack.as_ref(),
            components,
            place: &mut place,
            screen: [screen.0, screen.1],
            positioner: &mod_api::DefaultTooltipPositioner,
            font: &mut font,
            draw: &mut draw,
            cancel: &mut cancel,
        });
        if cancel {
            return None;
        }
        let moved = if place == before {
            at
        } else {
            (place.x as f32, place.y as f32)
        };
        let final_at = if moved.0.is_finite() && moved.1.is_finite() {
            moved
        } else {
            at
        };
        Some((final_at, font))
    }

    /// `GuiGraphicsExtractor.tooltip`: run Pre, measure in Pre's chosen font without rewrapping,
    /// position with `DefaultTooltipPositioner`, draw background (`render_tooltip_texture`), then
    /// run Pass 1 (`draw_text`) and Pass 2 (`draw_image`). Returns the drawn content rect
    /// `[x, y, w, h]` when drawn.
    pub(in crate::game) fn draw_prepared_tooltip(
        &self,
        batch: &mut HudBatch,
        stack: Option<&ItemStack>,
        components: &[mod_api::PreparedTooltipComponent],
        at: (f32, f32),
        extra_space_after_first_line: bool,
    ) -> Option<[f32; 4]> {
        if components.is_empty() {
            return None;
        }
        let screen = (batch.width, batch.height);
        let (at, chosen_font) =
            self.tooltip_pre_components(batch, stack, components, at, screen)?;

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
        for component in components {
            let cw = component.width(&tooltip_font);
            if cw > w {
                w = cw;
            }
            let ch = component.height(&tooltip_font);
            heights.push(ch);
            h = h.saturating_add(ch);
        }

        let [x, y] = mod_api::ClientTooltipPositioner::position_tooltip(
            &mod_api::DefaultTooltipPositioner,
            [screen.0 as i32, screen.1 as i32],
            [at.0.round() as i32, at.1.round() as i32],
            [w, h],
        );

        self.tooltip_box(batch, stack, x as f32, y as f32, w as f32, h as f32);

        let mut local_y = y;
        for (i, (component, &ch)) in components.iter().zip(&heights).enumerate() {
            if let Some((api_text, color)) = component.as_text() {
                if let Ok(engine_text) = api_text.try_into_engine(&()) {
                    batch.text_styled_with_font(
                        &self.gui.atlas,
                        &engine_text,
                        x as f32,
                        local_y as f32,
                        color,
                        true,
                        &chosen_font_str,
                    );
                }
            } else if component.as_bundle().is_none() {
                let paint = batch.paint_checkpoint();
                let ok = {
                    let mut draw = crate::game::host::draw::GameDrawContext::new(self, batch);
                    component.__draw_text(&tooltip_font, x, local_y, &mut draw)
                };
                if !ok {
                    paint.discard(batch);
                }
            }
            local_y = local_y.saturating_add(ch).saturating_add(
                if extra_space_after_first_line && i == 0 {
                    2
                } else {
                    0
                },
            );
        }

        let mut local_y = y;
        for (i, (component, &ch)) in components.iter().zip(&heights).enumerate() {
            if let Some(api_stack) = component.as_bundle() {
                if let Ok(engine_stack) = crate::game::api_values::stack_value(api_stack) {
                    let description_lines =
                        if sunrise_client::bundle::Contents::of(self.items(), &engine_stack)
                            .is_some_and(|c| c.items().is_empty())
                        {
                            self.bundle_description_lines(&chosen_font_str)
                        } else {
                            0
                        };
                    if let Some(layout) = sunrise_client::bundle::tooltip_layout(
                        self.items(),
                        &engine_stack,
                        x as f32,
                        local_y as f32,
                        w as f32,
                        description_lines,
                    ) {
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
                }
            } else if component.as_text().is_none() {
                let paint = batch.paint_checkpoint();
                let ok = {
                    let mut draw = crate::game::host::draw::GameDrawContext::new(self, batch);
                    component.__draw_image(&tooltip_font, x, local_y, w, h, &mut draw)
                };
                if !ok {
                    paint.discard(batch);
                }
            }
            local_y = local_y.saturating_add(ch).saturating_add(
                if extra_space_after_first_line && i == 0 {
                    2
                } else {
                    0
                },
            );
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
        let Some(components) = self.gather_tooltip_components(
            stack,
            lines,
            x.round() as i32,
            (batch.width, batch.height),
        ) else {
            return;
        };
        self.draw_prepared_tooltip(batch, Some(stack), &components, (x, y), true);
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
        crate::game::api_values::install_parts(&mods).unwrap();
        mod_platform::testing::with_mods(mods, || {
            with_test_factories(factories, || {
                crate::game::native_entity_refs::tests::with_client_game(|game| {
                    TEXT_DRAWN.store(false, Ordering::Relaxed);
                    IMAGE_DRAWN.store(false, Ordering::Relaxed);
                    game.draw_stack_tooltip(
                        &mut HudBatch::scratch([320, 240]),
                        &ItemStack::new("minecraft:stick", 1),
                        10.0,
                        20.0,
                    );
                    assert!(TEXT_DRAWN.load(Ordering::Relaxed));
                    assert!(IMAGE_DRAWN.load(Ordering::Relaxed));
                });
            });
        });
    }

    #[test]
    fn wrap_tooltip_width_matches_neoforge_client_hooks() {
        // Fits on the right side without wrapping.
        assert_eq!(wrap_tooltip_width(60, 10, 320, -1), (60, false));
        // Overflows right, flips left cleanly without wrapping.
        assert_eq!(wrap_tooltip_width(80, 260, 320, -1), (80, false));
        // Overflows both sides; cursor on right half wraps to mouse_x - 12 - 8.
        assert_eq!(wrap_tooltip_width(180, 180, 320, -1), (160, true));
        // Overflows both sides; cursor on left half wraps to screen_width - 16 - mouse_x.
        assert_eq!(wrap_tooltip_width(180, 100, 320, -1), (204, true));
        // Explicit positive max_width narrows and enables wrapping.
        assert_eq!(wrap_tooltip_width(100, 10, 320, 40), (40, true));
        // Zero or negative max_width leaves wrapping disabled when tooltip fits.
        assert_eq!(wrap_tooltip_width(100, 10, 320, 0), (100, false));
        // Extreme values never overflow.
        let _ = wrap_tooltip_width(i32::MAX, i32::MAX, i32::MAX, i32::MAX);
        let _ = wrap_tooltip_width(i32::MIN, i32::MIN, i32::MIN, i32::MIN);
    }
}

#[cfg(test)]
#[path = "tooltip_bundle_tests.rs"]
mod bundle_tests;
