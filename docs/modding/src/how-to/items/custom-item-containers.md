# Custom bundle-like item containers

Build a bundle-like item container (such as a rune pouch or quiver) by combining a registered
data component with `BundleConfig`, `BundleItems::with_config`, and `ItemPart` hooks. Everything
runs inside the engine's transactional item and menu paths, reusing the shared exact-fraction
weight, nesting, bar, and rollback rules.

## Vanilla rules vs. Sunrise-specific extensions

The weight model follows vanilla 26.3's `BundleContents` and `BundleItem` rules:

- Exact rational occupancy (`1 / max_stack_size`, non-empty `minecraft:bees` = `1`, nested
  vanilla bundles and nested containers sharing the same component = `contents + 1/16`, evaluated
  iteratively with no artificial nesting depth cutoff).
- Front-first insertion and extraction order, fullness bar color/width (`0x7087FF` blue while
  partial, `0xFF5454` red at width `13` when full, hidden when empty), and tooltip grid layout.

The following behaviours are **Sunrise-specific extensions** beyond vanilla 26.3 (where
`BundleContents` only has capacity `1`):

- **Capacity multipliers (`capacity_multiplier > 1`)**: scale total weight capacity by a positive
  whole number (`1` = 64 normal items, `4` = 256, `8` = 512, etc.). `0` clamps to `1`.
- **Stack splitting at `max_stack`**: when inserting or merging into a container with
  `capacity_multiplier > 1`, an existing matching partial stack is topped up to the item's
  `max_stack` and any remaining inserted items are split into new front entries each `<= max_stack`.

## Storage schema, overfull, and malformed contents

- **Single strict storage format**: custom bundle components must store a `Tag::List` of strict
  `ItemStack.CODEC` compounds (`id` non-air identifier registered in the active item catalog,
  optional integer `count` in `1..=99`, optional persistent `components` compound — matching
  vanilla `BundleContents.CODEC` and registry-backed `Item.CODEC`). Unmentioned extra keys on an
  entry compound are accepted on decode and dropped on re-encode, matching vanilla
  `ItemStack.MAP_CODEC` and Sunrise's vanilla bundle path (`item_data::template_from` /
  `template_tag`). Any non-list tag shape (such as a `Tag::Compound` or scalar tag) **or** any
  malformed or unknown-item element (e.g. `mod:removed_item`) inside the list refuses the entire
  payload with `Err(BundleReadError::UnsupportedFormat)` across all five `BundleConfig` helpers
  (`slots`, `room_for`, `try_insert`, `extract`, `metrics`) and is never overwritten by
  `BundleItems`, `CustomBundleContents::store`, or component destruction.
- **Malformed nested containers**: if a stored item (at any nesting depth, including inside a
  nested vanilla bundle) or an offered stack carries the same custom component with an unsupported
  or malformed payload, weight evaluation propagates `Err(BundleReadError::UnsupportedFormat)`
  instead of weighing the malformed container as an ordinary leaf item.
- **Unavailable host**: calling any of the five `BundleConfig` helpers on a `Host` without an
  active world item registry returns `Err(BundleReadError::Unavailable)`.
- **Overfull stored contents**: if a saved or command-created stack already exceeds capacity
  (e.g. from lowering `capacity_multiplier` or `/give`), `room_for` returns `Ok(0)`, `try_insert`
  inserts `0`, `metrics.is_full` is `true`, `metrics.fullness > 1.0`, and `metrics.bar` clamps to
  `Some((13, 0xFF5454))`. `extract` still removes items from slot `0`, preserving any remainder at
  the front.
- **Memoized hot draw path**: decoded contents and exact rational weight are memoized per component
  value in a bounded host cache (invalidated whenever the item catalog reloads, mirroring vanilla
  `BundleContents`'s memoized weight), so repeated `metrics` / `slots` calls during `item_bar`,
  `tooltip_image`, and `item_model_range_property` avoid re-decoding NBT or re-walking nested
  container trees on every frame.

## 1. Register the component and default

Register your component id in `Hooks::component_types` before content loads, and give your item
an empty default list (`[]`) in its item definition so every fresh stack carries the component:

```rust,ignore
#[sunrise_mod]
impl Hooks for MyMod {
    fn component_types(&self, _: &RegistrationContext, event: &mut ComponentTypesEvent<'_>) {
        event.register("mymod:pouch_contents".into());
    }
}
```

Define one shared `BundleConfig` for the container's component id, capacity multiplier (`1` =
64 normal items, `4` = 256, `8` = 512, etc.), and optional item tag filter:

```rust,ignore
fn pouch_config() -> BundleConfig {
    BundleConfig::custom("mymod:pouch_contents", 4).with_filter_tag("mymod:runes")
}
```

## 2. Expose `ITEM_ITEMS` for automation

Return `BundleItems::with_config(place.clone(), pouch_config())` from `r.item_capability`:

```rust,ignore
fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
    let pouch = r.item_key("mymod:rune_pouch");
    r.item_part(pouch, PouchPart);
    r.item_capability(&ITEM_ITEMS, "mymod:rune_pouch", |_, place| {
        Some(Box::new(BundleItems::with_config(place.clone(), pouch_config())))
    });
}
```

Hoppers, capabilities, and nested `ItemPlace` writes now insert at the front, merge matching
stacks up to each item's `max_stack`, respect `filter_tag` and `can_fit_inside_container_items`,
account for nested vanilla bundles and nested containers sharing the same component, and roll
back atomically if the caller's transaction aborts.

## 3. Handle menu stacking and dropped-item destruction

Implement `ItemPart::item_stacked_on_other` for inventory/container clicks (running in vanilla
`AbstractContainerMenu.tryItemClickBehaviourOverride` order inside the click transaction) and
`ItemPart::item_destroyed` to spill contents when the dropped item entity is destroyed:

```rust,ignore
#[sunrise_part]
impl ItemPart for PouchPart {
    fn item_stacked_on_other(
        &self,
        host: HostSide<'_>,
        event: &mut ItemStackedOnOtherEvent<'_>,
    ) -> Flow {
        let config = pouch_config();
        let host_ref: &dyn Host = match &host {
            HostSide::Server(h) => &**h,
            HostSide::Client(h) => &**h,
        };
        match event.event.direction {
            StackedOnOtherDirection::CarriedOnOther => {
                let Some(carried) = event.event.carried.as_ref() else {
                    return Flow::Continue;
                };
                if event.event.primary {
                    let Some(clicked) = event.event.clicked.as_ref() else {
                        return Flow::Continue;
                    };
                    let Ok(Some((moved, updated))) = config.try_insert(host_ref, carried, clicked)
                    else {
                        return Flow::Continue;
                    };
                    if moved > 0 {
                        event.result.carried = StackResult::Set(Some(updated));
                        event.result.clicked =
                            ClickedStackResult::Set(clicked.clone().shrink(moved));
                        event.result.handled = true;
                        return Flow::Cancel;
                    }
                } else if event.event.clicked.is_none()
                    && let Ok(Some((Some(out), updated))) =
                        config.extract(host_ref, carried, u32::MAX)
                {
                    event.result.carried = StackResult::Set(Some(updated));
                    event.result.clicked = ClickedStackResult::Set(Some(out));
                    event.result.handled = true;
                    return Flow::Cancel;
                }
            }
            StackedOnOtherDirection::OtherStackedOnMe => {
                let Some(clicked) = event.event.clicked.as_ref() else {
                    return Flow::Continue;
                };
                if event.event.primary
                    && let Some(carried) = event.event.carried.as_ref()
                    && let Ok(Some((moved, updated))) =
                        config.try_insert(host_ref, clicked, carried)
                    && moved > 0
                {
                    event.result.clicked = ClickedStackResult::Set(Some(updated));
                    event.result.carried = StackResult::Set(carried.clone().shrink(moved));
                    event.result.handled = true;
                    return Flow::Cancel;
                }
            }
            StackedOnOtherDirection::Event => {}
        }
        Flow::Continue
    }

    fn item_destroyed(&self, host: &mut dyn ServerHost, event: &mut ItemDestroyedEvent<'_>) {
        if let Ok(Some(inside)) = pouch_config().slots(host, event.stack) {
            event.drops.extend(inside);
        }
    }
}
```

## 4. Add fullness bar, custom tooltip, right-click use, and model properties

Reuse the existing Item Part and model hooks over the same `BundleConfig`:

- **Fullness bar (`ItemPart::item_bar`)**: read `pouch_config().metrics(host, event.stack)` and
  set `*event.bar = metrics.bar` (blue until full, red at width 13 when full, hidden when empty).
- **Tooltip image (`ItemPart::tooltip_image` and a client tooltip component)**: in
  `tooltip_image`, read `config.slots(host, event.stack)` and `config.metrics(host, event.stack)`
  and return `Some(TooltipComponent::custom(data))`, where `data` is your own
  `TooltipComponentData` type carrying the stacks to show, `metrics.fullness` and
  `metrics.is_full` (vanilla's `BundleItem.getTooltipImage` hands a `BundleTooltip` its contents
  the same way). Register a `ClientTooltipComponent` for that type in
  `Hooks::register_client_tooltip_component_factories`; it measures itself (`width`, `height`)
  and draws the stored stacks and the progress bar in `draw_image` from the data alone. See
  [Custom tooltip components](../client/client-effects.md#custom-tooltip-components-and-render-hooks).
- **Right-click use (`ItemPart::use_item`)**: extract from the held pouch with `config.extract`
  and rewrite the held stack through `ItemPlace::PlayerHand` inside a transaction.
- **Nesting refusal (`ItemPart::can_fit_inside_container_items`)**: set `*event.fits = false` if
  your pouch should not fit inside shulker boxes or other bundles.
- **Filled model variant (`register_range_select_item_model_property` / `item_model_range_property`)**:
  answer `metrics.fullness` so a `minecraft:range_dispatch` model switches between empty, partial,
  and full textures.
