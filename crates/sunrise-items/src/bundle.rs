//! `BundleContents.Mutable`: what a bundle takes in and gives back, by
//! weight. The menus (`menu`, `lan::menu`) run `BundleItem`'s click
//! overrides on it.
use crate::item_data::BundleSelected;
use crate::items::{ItemStack, Items};
use std::collections::{HashMap, VecDeque, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use sunrise_core::nbt::Tag;

/// A bundle weight: vanilla's `Fraction`, exact and kept reduced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Weight {
    n: i128,
    d: i128,
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

impl Weight {
    const ZERO: Self = Self { n: 0, d: 1 };
    const ONE: Self = Self { n: 1, d: 1 };

    fn new(n: i128, d: i128) -> Self {
        let g = gcd(n, d);
        Self { n: n / g, d: d / g }
    }

    fn add(self, o: Self) -> Self {
        let g = gcd(self.d, o.d);
        let d1 = self.d / g;
        let d2 = o.d / g;
        let n = self
            .n
            .saturating_mul(d2)
            .saturating_add(o.n.saturating_mul(d1));
        let d = self.d.saturating_mul(d2).max(1);
        Self::new(n, d)
    }

    fn sub(self, o: Self) -> Self {
        self.add(Self { n: -o.n, d: o.d })
    }

    fn times(self, count: u32) -> Self {
        self.times_u128(u128::from(count))
    }

    fn times_u128(self, count: u128) -> Self {
        let c = i128::try_from(count).unwrap_or(i128::MAX);
        let g = gcd(c, self.d);
        Self::new(self.n.saturating_mul(c / g), (self.d / g).max(1))
    }

    /// `this.divideBy(item).intValue()` (truncated), at least 0.
    fn how_many(self, item: Self) -> u32 {
        if self.n <= 0 || item.n <= 0 {
            return 0;
        }
        let num = self.n.saturating_mul(item.d);
        let den = self.d.saturating_mul(item.n).max(1);
        u32::try_from((num / den).max(0)).unwrap_or(u32::MAX)
    }
}

/// The sounds of `BundleItem` (`Entity.playSound` at the player): insert
/// and remove one at volume 0.8 and pitch `0.8 + random * 0.4`, a failed
/// insert at 1, 1; the drop of its contents is played at the player's block
/// like remove one.
pub const INSERT: &str = "minecraft:item.bundle.insert";
pub const INSERT_FAIL: &str = "minecraft:item.bundle.insert_fail";
pub const REMOVE_ONE: &str = "minecraft:item.bundle.remove_one";
pub const DROP_CONTENTS: &str = "minecraft:item.bundle.drop_contents";

pub(crate) fn max_stack(items: &Items, stack: &ItemStack) -> u32 {
    let info = items.get(&stack.item);
    if !stack.carries_component(info, "minecraft:max_stack_size") {
        return 1;
    }
    stack
        .data()
        .and_then(|data| data.other("minecraft:max_stack_size"))
        .and_then(crate::properties::component_int)
        .unwrap_or_else(|| info.map_or(1, |item| item.max_stack as i32))
        .max(1) as u32
}

/// Identifies a bundle-like item component and its weight capacity multiplier.
///
/// Vanilla bundles use [`BundleSpec::VANILLA`] (`"minecraft:bundle_contents"`, multiplier `1`).
/// Custom containers name a registered data component and a positive whole-number multiplier
/// relative to a vanilla bundle (`1` = 64 normal items, `4` = 256, etc.).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BundleSpec<'a> {
    /// Namespaced data component id storing the container's items.
    pub component: &'a str,
    /// Whole-number capacity multiplier relative to a vanilla bundle (clamped >= 1).
    pub capacity_multiplier: u32,
}

impl<'a> BundleSpec<'a> {
    /// Vanilla `minecraft:bundle_contents` with multiplier `1`.
    pub const VANILLA: Self = Self {
        component: "minecraft:bundle_contents",
        capacity_multiplier: 1,
    };

    /// A bundle-like component with capacity `capacity_multiplier.max(1)`.
    pub const fn custom(component: &'a str, capacity_multiplier: u32) -> Self {
        Self {
            component,
            capacity_multiplier: if capacity_multiplier == 0 {
                1
            } else {
                capacity_multiplier
            },
        }
    }

    /// Whether this spec targets vanilla's `minecraft:bundle_contents` storage.
    pub fn is_vanilla(&self) -> bool {
        self.component == "minecraft:bundle_contents"
    }

    fn multiplier(&self) -> u32 {
        self.capacity_multiplier.max(1)
    }

    fn capacity(&self) -> Weight {
        Weight::new(i128::from(self.multiplier()), 1)
    }
}

/// Why a custom bundle-like component on an item stack could not be decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomBundleError {
    /// The stack (or a nested container carrying the same custom component) does not conform to
    /// `Tag::List` of strict `ItemStack.CODEC` entries.
    UnsupportedFormat,
}

impl std::fmt::Display for CustomBundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedFormat => {
                f.write_str("custom bundle component value is not a list of valid item stacks")
            }
        }
    }
}

impl std::error::Error for CustomBundleError {}

/// The stack's `bundle_contents`, when it has the component.
fn contents_of<'a>(items: &Items, stack: &'a ItemStack) -> Option<&'a [ItemStack]> {
    let inside = stack.data().map_or(&[][..], |d| &d.bundle_contents[..]);
    stack
        .carries_component(items.get(&stack.item), "minecraft:bundle_contents")
        .then_some(inside)
}

/// Strictly decode one custom bundle entry matching vanilla 26.3 `ItemStack.CODEC`:
/// a `Tag::Compound` carrying `id` (required valid non-air identifier present in `items`,
/// matching registry-backed `Item.CODEC`), optional integer `count` in `1..=99` (default `1`),
/// and optional `Tag::Compound` `components` passing persistent component validation (no
/// transient components).
///
/// Extra unmentioned keys on the entry compound (e.g. `{id:"minecraft:stone", count:1, tier:3}`)
/// are accepted and ignored on decode — and dropped on re-encode via `item_nbt::item_to_nbt` —
/// matching vanilla `ItemStack.MAP_CODEC` (`RecordCodecBuilder`) as well as Sunrise's vanilla
/// `minecraft:bundle_contents` path (`item_data::template_from` / `item_data::template_tag`) and
/// `item_nbt::item_from_persistent_nbt` / `item_nbt::item_to_nbt`.
///
/// Any malformed entry, unknown/removed item id, out-of-range or non-integer `count`, or invalid
/// `components` map refuses the entire payload with [`CustomBundleError::UnsupportedFormat`].
fn try_decode_custom_stack(items: &Items, entry: &Tag) -> Result<ItemStack, CustomBundleError> {
    if !matches!(entry, Tag::Compound(_)) {
        return Err(CustomBundleError::UnsupportedFormat);
    }
    let Some(Tag::String(raw_id)) = entry.get("id") else {
        return Err(CustomBundleError::UnsupportedFormat);
    };
    let raw_id = raw_id
        .to_string()
        .map_err(|_| CustomBundleError::UnsupportedFormat)?;
    let (namespace, path) =
        sunrise_world::datafix::identifier(&raw_id).ok_or(CustomBundleError::UnsupportedFormat)?;
    if namespace == "minecraft" && path == "air" {
        return Err(CustomBundleError::UnsupportedFormat);
    }
    let id = format!("{namespace}:{path}");
    if items.get(&id).is_none() {
        return Err(CustomBundleError::UnsupportedFormat);
    }
    let count = match entry.get("count") {
        None => 1u32,
        Some(value) => {
            let n = match value {
                Tag::Byte(n) => i32::from(*n),
                Tag::Short(n) => i32::from(*n),
                Tag::Int(n) => *n,
                Tag::Long(n) => {
                    i32::try_from(*n).map_err(|_| CustomBundleError::UnsupportedFormat)?
                }
                _ => return Err(CustomBundleError::UnsupportedFormat),
            };
            if !(1..=99).contains(&n) {
                return Err(CustomBundleError::UnsupportedFormat);
            }
            n as u32
        }
    };
    let components = match entry.get("components") {
        None => None,
        Some(value) => {
            if !matches!(value, Tag::Compound(_)) {
                return Err(CustomBundleError::UnsupportedFormat);
            }
            sunrise_world::item_defaults::validate_component_map_tag(value)
                .map_err(|_| CustomBundleError::UnsupportedFormat)?;
            Some(value)
        }
    };
    let (damage, extra) = crate::item_data::decode_components(components);
    Ok(ItemStack {
        item: Arc::from(id.as_str()),
        count,
        damage,
        extra,
    })
}

/// Decode a custom bundle-like component tag.
///
/// The single supported custom storage format is `Tag::List` of strict `ItemStack.CODEC` entries
/// (matching vanilla `BundleContents.CODEC`). Any non-list tag or any malformed or unknown-item
/// entry inside the list refuses the entire payload with [`CustomBundleError::UnsupportedFormat`]
/// rather than silently dropping entries on rewrite.
fn try_decode_custom_tag(items: &Items, tag: &Tag) -> Result<Vec<ItemStack>, CustomBundleError> {
    let Tag::List(entries) = tag else {
        return Err(CustomBundleError::UnsupportedFormat);
    };
    entries
        .iter()
        .map(|entry| try_decode_custom_stack(items, entry))
        .collect()
}

/// The stack's decoded custom component contents when it carries `component`.
///
/// Returns `Ok(None)` when `stack` does not carry `component`, `Ok(Some(items))` when the
/// component is a `Tag::List` whose every element strictly decodes as `ItemStack.CODEC` against
/// `items`, and `Err(CustomBundleError::UnsupportedFormat)` otherwise.
fn try_custom_contents_of(
    items: &Items,
    stack: &ItemStack,
    component: &str,
) -> Result<Option<Vec<ItemStack>>, CustomBundleError> {
    let info = items.get(&stack.item);
    if !stack.carries_component(info, component) {
        return Ok(None);
    }
    let tag = stack
        .data()
        .and_then(|d| d.other(component))
        .or_else(|| info.and_then(|i| i.custom_components.get(component)));
    match tag {
        Some(tag) => try_decode_custom_tag(items, tag).map(Some),
        None => Ok(Some(Vec::new())),
    }
}

enum WeightEntry<'a> {
    Borrowed(&'a ItemStack, u128),
    Owned(ItemStack, u128),
}

/// Exact iterative evaluation of `BundleContents.getWeight`, generalized to recognize both nested
/// vanilla bundles and nested containers sharing `custom_component`: a nested bundle weighs its
/// contents plus 1/16, a beehive with bees 1, the rest 1 / max stack size.
///
/// Iterative traversal avoids call-stack limits and evaluates arbitrary NBT nesting depth without
/// an artificial depth cutoff. Any nested stack carrying `custom_component` with a malformed
/// payload propagates [`CustomBundleError::UnsupportedFormat`] rather than falling back to leaf
/// weight.
fn accumulate_weight(
    items: &Items,
    work: &mut Vec<WeightEntry<'_>>,
    custom_component: Option<&str>,
) -> Result<Weight, CustomBundleError> {
    let mut total = Weight::ZERO;
    while let Some(entry) = work.pop() {
        match entry {
            WeightEntry::Borrowed(stack, mult) => {
                if mult == 0 {
                    continue;
                }
                if let Some(inside) = contents_of(items, stack) {
                    total = total.add(Weight::new(1, 16).times_u128(mult));
                    for child in inside {
                        work.push(WeightEntry::Borrowed(
                            child,
                            mult.saturating_mul(u128::from(child.count)),
                        ));
                    }
                    continue;
                }
                if let Some(component) = custom_component
                    && component != "minecraft:bundle_contents"
                    && let Some(inside) = try_custom_contents_of(items, stack, component)?
                {
                    total = total.add(Weight::new(1, 16).times_u128(mult));
                    for child in inside {
                        let child_mult = mult.saturating_mul(u128::from(child.count));
                        work.push(WeightEntry::Owned(child, child_mult));
                    }
                    continue;
                }
                total = total.add(leaf_weight(items, stack).times_u128(mult));
            }
            WeightEntry::Owned(stack, mult) => {
                if mult == 0 {
                    continue;
                }
                if let Some(inside) = contents_of(items, &stack) {
                    total = total.add(Weight::new(1, 16).times_u128(mult));
                    for child in inside {
                        let child_mult = mult.saturating_mul(u128::from(child.count));
                        work.push(WeightEntry::Owned(child.clone(), child_mult));
                    }
                    continue;
                }
                if let Some(component) = custom_component
                    && component != "minecraft:bundle_contents"
                    && let Some(inside) = try_custom_contents_of(items, &stack, component)?
                {
                    total = total.add(Weight::new(1, 16).times_u128(mult));
                    for child in inside {
                        let child_mult = mult.saturating_mul(u128::from(child.count));
                        work.push(WeightEntry::Owned(child, child_mult));
                    }
                    continue;
                }
                total = total.add(leaf_weight(items, &stack).times_u128(mult));
            }
        }
    }
    Ok(total)
}

fn leaf_weight(items: &Items, stack: &ItemStack) -> Weight {
    let bees = stack
        .data()
        .and_then(|d| d.other("minecraft:bees"))
        .is_some_and(|b| matches!(b, Tag::List(l) if !l.is_empty()));
    if bees {
        Weight::ONE
    } else {
        Weight::new(1, i128::from(max_stack(items, stack)))
    }
}

fn item_weight_for(
    items: &Items,
    stack: &ItemStack,
    custom_component: Option<&str>,
) -> Result<Weight, CustomBundleError> {
    let mut work = vec![WeightEntry::Borrowed(stack, 1)];
    accumulate_weight(items, &mut work, custom_component)
}

fn contents_weight_for(
    items: &Items,
    inside: &[ItemStack],
    custom_component: Option<&str>,
) -> Result<Weight, CustomBundleError> {
    let mut work: Vec<WeightEntry<'_>> = inside
        .iter()
        .map(|s| WeightEntry::Borrowed(s, u128::from(s.count)))
        .collect();
    accumulate_weight(items, &mut work, custom_component)
}

/// Vanilla `BundleContents.computeContentWeight`: infallible because vanilla bundles only inspect
/// `minecraft:bundle_contents`, `minecraft:bees`, and `max_stack_size`.
fn contents_weight(items: &Items, inside: &[ItemStack]) -> Weight {
    contents_weight_for(items, inside, None)
        .expect("vanilla bundle contents cannot fail custom format validation")
}

fn weight_fraction_for(weight: Weight, capacity_multiplier: u32) -> f32 {
    let cap_d = weight.d * i128::from(capacity_multiplier.max(1));
    weight.n as f32 / cap_d as f32
}

fn is_full_for(weight: Weight, capacity_multiplier: u32) -> bool {
    weight.n >= weight.d * i128::from(capacity_multiplier.max(1))
}

fn bar_for(weight: Weight, capacity_multiplier: u32) -> Option<(i32, u32)> {
    if weight.n <= 0 {
        return None;
    }
    let cap_d = weight.d * i128::from(capacity_multiplier.max(1));
    let width = (1 + weight.n * 12 / cap_d).min(13) as i32;
    let color = if is_full_for(weight, capacity_multiplier) {
        0xFF5454
    } else {
        0x7087FF
    };
    Some((width, color))
}

/// `BundleItem.onDestroyed` (`ItemStack.onDestroyed` for a bundle): the stack
/// is emptied and every stack it held comes back as its own item copy, which
/// `ItemUtils.onContainerDestroyed` drops where the entity lay.
pub fn destroyed(stack: &mut ItemStack) -> Vec<ItemStack> {
    let Some(data) = stack.extra.as_deref_mut() else {
        return Vec::new();
    };
    if data.bundle_contents.is_empty() {
        return Vec::new();
    }
    // `set(BUNDLE_CONTENTS, BundleContents.EMPTY)`: no contents and no
    // selected index left.
    let spilled = std::mem::take(&mut data.bundle_contents);
    data.bundle_selected = BundleSelected(-1);
    stack.tidy();
    spilled
}

/// Generalized `BundleItem.onDestroyed` for any bundle-like component id: empties `component`
/// on `stack` and returns the stored stacks for dropping. Delegates `"minecraft:bundle_contents"`
/// to [`destroyed`]. Leaves stacks without `component` or carrying an unsupported non-list
/// component payload untouched and returns an empty `Vec`.
pub fn destroyed_component(
    items: &Items,
    stack: &mut ItemStack,
    component: &str,
) -> Vec<ItemStack> {
    if component == "minecraft:bundle_contents" {
        return destroyed(stack);
    }
    let Ok(Some(mut contents)) =
        CustomBundleContents::try_of(items, stack, BundleSpec::custom(component, 1))
    else {
        return Vec::new();
    };
    if contents.items.is_empty() {
        return Vec::new();
    }
    let spilled = std::mem::take(&mut contents.items);
    contents.weight = Weight::ZERO;
    contents.selected = -1;
    contents.store(items, stack);
    spilled
}

/// Generalized `BundleContents.Mutable` parameterized by component id and capacity multiplier.
///
/// Vanilla [`Contents`] is a thin wrapper over `CustomBundleContents` with
/// [`BundleSpec::VANILLA`], sharing the exact fraction weight, front-insert, merge, removal,
/// bar, and tooltip geometry logic. Capacity multipliers above `1` and splitting merged/inserted
/// stacks at each item's `max_stack` are Sunrise-specific extensions beyond vanilla 26.3's
/// capacity-`1` `BundleContents`.
pub struct CustomBundleContents {
    component: String,
    capacity_multiplier: u32,
    items: Vec<ItemStack>,
    weight: Weight,
    /// `selectedItem`: an index into `items`, -1 for none.
    selected: i32,
}

impl CustomBundleContents {
    fn custom_component(&self) -> Option<&str> {
        (self.component != "minecraft:bundle_contents").then_some(self.component.as_str())
    }

    fn capacity(&self) -> Weight {
        Weight::new(i128::from(self.capacity_multiplier), 1)
    }

    /// Build contents from an explicit item list under `spec`, validating any nested containers
    /// sharing `spec.component`.
    pub fn try_from_items(
        items: &Items,
        spec: BundleSpec<'_>,
        inside: Vec<ItemStack>,
    ) -> Result<Self, CustomBundleError> {
        let custom = (!spec.is_vanilla()).then_some(spec.component);
        let weight = contents_weight_for(items, &inside, custom)?;
        Ok(Self {
            component: spec.component.to_owned(),
            capacity_multiplier: spec.multiplier(),
            items: inside,
            weight,
            selected: -1,
        })
    }

    /// Read the container's contents from `stack` under `spec`.
    ///
    /// Returns `Ok(None)` when `stack` does not carry `spec.component`, or
    /// `Err(CustomBundleError::UnsupportedFormat)` when a custom component (on `stack` or any
    /// nested container carrying `spec.component`) is not a `Tag::List` of strict
    /// `ItemStack.CODEC` entries.
    pub fn try_of(
        items: &Items,
        stack: &ItemStack,
        spec: BundleSpec<'_>,
    ) -> Result<Option<Self>, CustomBundleError> {
        if spec.is_vanilla() {
            let Some(inside) = contents_of(items, stack) else {
                return Ok(None);
            };
            let inside = inside.to_vec();
            let weight = contents_weight(items, &inside);
            let selected = stack.data().map_or(-1, |d| d.bundle_selected.0);
            return Ok(Some(Self {
                component: spec.component.to_owned(),
                capacity_multiplier: spec.multiplier(),
                items: inside,
                weight,
                selected,
            }));
        }
        let Some(inside) = try_custom_contents_of(items, stack, spec.component)? else {
            return Ok(None);
        };
        let weight = contents_weight_for(items, &inside, Some(spec.component))?;
        Ok(Some(Self {
            component: spec.component.to_owned(),
            capacity_multiplier: spec.multiplier(),
            items: inside,
            weight,
            selected: -1,
        }))
    }

    /// Read the container's contents from `stack` under `spec`; `None` when `stack` does not
    /// carry `spec.component` or carries an unsupported payload.
    pub fn of(items: &Items, stack: &ItemStack, spec: BundleSpec<'_>) -> Option<Self> {
        Self::try_of(items, stack, spec).ok().flatten()
    }

    /// Remaining capacity for `offered` in `bundle` under `spec`, distinguishing missing
    /// components (`Ok(None)`) from unsupported custom payloads (either in `bundle` or nested
    /// inside `offered`, returning `Err(CustomBundleError::UnsupportedFormat)`).
    pub fn try_room_in_stack(
        items: &Items,
        bundle: &ItemStack,
        spec: BundleSpec<'_>,
        offered: &ItemStack,
    ) -> Result<Option<u32>, CustomBundleError> {
        if spec.is_vanilla() {
            let Some(inside) = contents_of(items, bundle) else {
                return Ok(None);
            };
            return Ok(Some(
                spec.capacity()
                    .sub(contents_weight(items, inside))
                    .how_many(item_weight_for(items, offered, None)?),
            ));
        }
        let Some(inside) = try_custom_contents_of(items, bundle, spec.component)? else {
            return Ok(None);
        };
        Ok(Some(
            spec.capacity()
                .sub(contents_weight_for(items, &inside, Some(spec.component))?)
                .how_many(item_weight_for(items, offered, Some(spec.component))?),
        ))
    }

    /// Remaining capacity for `offered` in `bundle` under `spec`, avoiding cloning contents for
    /// vanilla stacks.
    pub fn room_in_stack(
        items: &Items,
        bundle: &ItemStack,
        spec: BundleSpec<'_>,
        offered: &ItemStack,
    ) -> Option<u32> {
        Self::try_room_in_stack(items, bundle, spec, offered)
            .ok()
            .flatten()
    }

    /// The component id backing these contents.
    pub fn component(&self) -> &str {
        &self.component
    }

    /// The capacity multiplier relative to a vanilla bundle (`>= 1`).
    pub fn capacity_multiplier(&self) -> u32 {
        self.capacity_multiplier
    }

    /// Fallible `getMaxAmountToAdd` for `stack`'s kind of item, propagating
    /// [`CustomBundleError::UnsupportedFormat`] if `stack` carries (or contains) a malformed
    /// container of the same custom component.
    pub fn try_room_for(&self, items: &Items, stack: &ItemStack) -> Result<u32, CustomBundleError> {
        let weight = item_weight_for(items, stack, self.custom_component())?;
        Ok(self.capacity().sub(self.weight).how_many(weight))
    }

    /// `getMaxAmountToAdd` for `stack`'s kind of item; returns `0` when `stack` carries a
    /// malformed container of the same custom component.
    pub fn room_for(&self, items: &Items, stack: &ItemStack) -> u32 {
        self.try_room_for(items, stack).unwrap_or(0)
    }

    /// Fallible `tryInsert`: what fits moves to the front, merged with its kind up to each stored
    /// stack's `max_stack` (splitting any overflow into new front entries when
    /// `capacity_multiplier > 1`). Returns `Err(CustomBundleError::UnsupportedFormat)` without
    /// modifying `self` or `stack` if `stack` carries (or contains) a malformed container of the
    /// same custom component.
    pub fn try_insert_checked(
        &mut self,
        items: &Items,
        stack: &mut ItemStack,
        fits: bool,
    ) -> Result<u32, CustomBundleError> {
        if stack.count == 0 {
            return Ok(0);
        }
        if !fits {
            if self.custom_component().is_some() {
                let _ = item_weight_for(items, stack, self.custom_component())?;
            }
            return Ok(0);
        }
        let weight = item_weight_for(items, stack, self.custom_component())?;
        let amount = stack
            .count
            .min(self.capacity().sub(self.weight).how_many(weight));
        if amount == 0 {
            return Ok(0);
        }
        self.weight = self.weight.add(weight.times(amount));
        // ItemStack.isStackable: a damaged stack is damageable.
        let max = max_stack(items, stack);
        let stackable = max > 1 && stack.damage == 0;
        let mut remaining = amount;
        if stackable
            && let Some(i) = self
                .items
                .iter()
                .position(|s| s.count < max && s.same_item_same_components(stack))
        {
            let mut merged = self.items.remove(i);
            let add = remaining.min(max - merged.count);
            merged.count += add;
            remaining -= add;
            self.items.insert(0, merged);
        }
        while remaining > 0 {
            let chunk = remaining.min(max);
            let mut part = stack.clone();
            part.count = chunk;
            remaining -= chunk;
            self.items.insert(0, part);
        }
        stack.count -= amount;
        Ok(amount)
    }

    /// `tryInsert`: what fits moves to the front, merged with its kind up to each stored stack's
    /// `max_stack` (splitting any overflow into new front entries when `capacity_multiplier > 1`).
    /// `fits` is the caller's container-fit and filter check; an empty or malformed nested stack
    /// never goes in.
    pub fn try_insert(&mut self, items: &Items, stack: &mut ItemStack, fits: bool) -> u32 {
        self.try_insert_checked(items, stack, fits).unwrap_or(0)
    }

    /// `removeOne`: the selected stack, else the first; nothing is selected after.
    pub fn remove_one(&mut self, items: &Items) -> Option<ItemStack> {
        if self.items.is_empty() {
            return None;
        }
        let at =
            usize::try_from(self.selected).map_or(0, |i| if i < self.items.len() { i } else { 0 });
        let unit = item_weight_for(items, &self.items[at], self.custom_component()).ok()?;
        let stack = self.items.remove(at);
        self.weight = self.weight.sub(unit.times(stack.count));
        self.selected = -1;
        Some(stack)
    }

    /// Fallible front extraction (slot 0), ignoring UI selection and keeping any remainder at the
    /// front even for overfull contents.
    pub fn try_extract_front(
        &mut self,
        items: &Items,
        max: u32,
    ) -> Result<Option<ItemStack>, CustomBundleError> {
        if max == 0 || self.items.is_empty() {
            return Ok(None);
        }
        let unit = item_weight_for(items, &self.items[0], self.custom_component())?;
        let take = self.items[0].count.min(max);
        let out = if take < self.items[0].count {
            let mut taken = self.items[0].clone();
            taken.count = take;
            self.items[0].count -= take;
            taken
        } else {
            self.items.remove(0)
        };
        self.weight = self.weight.sub(unit.times(take));
        self.selected = -1;
        Ok(Some(out))
    }

    /// Extract up to `max` items from the front stack (slot 0), ignoring UI selection and
    /// keeping any remainder at the front even for overfull contents.
    pub fn extract_front(&mut self, items: &Items, max: u32) -> Option<ItemStack> {
        self.try_extract_front(items, max).ok().flatten()
    }

    /// `getSelectedItemIndex`.
    pub fn selected(&self) -> i32 {
        self.selected
    }

    /// The stacks inside, in display order (the first is the next out).
    pub fn items(&self) -> &[ItemStack] {
        &self.items
    }

    /// `getNumberOfItemsToShow`: a full grid shows 11 and a `+N`, so the last
    /// row keeps its fourth cell for the count.
    pub fn shown_count(&self) -> usize {
        let count = self.items.len();
        let available: usize = if count > 12 { 11 } else { 12 };
        let on_last_row = count % 4;
        let empty = if on_last_row == 0 { 0 } else { 4 - on_last_row };
        count.min(available.saturating_sub(empty))
    }

    /// The items the tooltip shows, and how many sit beyond them.
    pub fn shown(&self) -> (&[ItemStack], u32) {
        let n = self.shown_count();
        let hidden = self.items[n..].iter().map(|s| s.count).sum();
        (&self.items[..n], hidden)
    }

    /// Stored weight divided by capacity (`weight / capacity_multiplier`), for bars and tooltips.
    pub fn weight_fraction(&self) -> f32 {
        weight_fraction_for(self.weight, self.capacity_multiplier)
    }

    /// Whether stored weight has reached or exceeded capacity (`capacity_multiplier`).
    pub fn is_full(&self) -> bool {
        is_full_for(self.weight, self.capacity_multiplier)
    }

    /// `BundleItem.isBarVisible`, `getBarWidth` and `getBarColor`: the fullness bar
    /// (width, RGB), blue until the container is full, then red.
    pub fn bar(&self) -> Option<(i32, u32)> {
        bar_for(self.weight, self.capacity_multiplier)
    }

    /// `ClientBundleTooltip.getHeight`: height of this container's tooltip component in GUI
    /// pixels. `description_lines` is the empty-container description line count in the active
    /// font (ignored when non-empty); `line_height` is `Font.lineHeight` (9 in vanilla).
    pub fn tooltip_height(&self, description_lines: usize, line_height: i32) -> i32 {
        if self.items.is_empty() {
            (description_lines as i32)
                .wrapping_mul(line_height)
                .wrapping_add(13 + 8)
        } else {
            let rows = self.items.len().min(12).div_ceil(4) as i32;
            rows.wrapping_mul(24).wrapping_add(13 + 8)
        }
    }

    /// `ClientBundleTooltip` geometry from these already-decoded contents in O(1) without
    /// re-decoding NBT or re-walking nested weights.
    pub fn tooltip_layout(
        &self,
        x: f32,
        y: f32,
        box_width: f32,
        description_lines: usize,
    ) -> TooltipLayout {
        let offset = ((box_width - 96.0) / 2.0).floor();
        if self.items.is_empty() {
            // The description takes the top; the bar follows it. Its lines are
            // the caller's font's (`getEmptyBundleDescriptionTextHeight`).
            let description_height = 9.0 * description_lines as f32;
            return TooltipLayout {
                width: 96.0,
                height: self.tooltip_height(description_lines, 9) as f32,
                slots: Vec::new(),
                selected: None,
                overflow: None,
                bar: [x + offset, y + description_height + 4.0],
                fill: 0.0,
                full: false,
                label: Some("item.minecraft.bundle.empty"),
                empty_description: Some([x + offset, y]),
            };
        }
        let (shown, hidden) = self.shown();
        // `gridSizeY` counts `slotCount()` (at most twelve), not the slots the
        // overflow count leaves room for.
        let rows = self.items.len().min(12).div_ceil(4).max(1);
        let grid_height = rows as f32 * 24.0;
        let x_start = x + offset + 96.0;
        let y_start = y + grid_height;
        let mut slots = Vec::with_capacity(shown.len());
        let mut selected = None;
        let mut overflow = None;
        // Rows fill bottom-up and columns right-to-left, as vanilla draws them.
        let mut slot = 0;
        for row in 1..=rows {
            for column in 1..=4 {
                let at = [x_start - column as f32 * 24.0, y_start - row as f32 * 24.0];
                if hidden > 0 && row == 1 && column == 1 {
                    overflow = Some((at, hidden));
                } else if slot < shown.len() {
                    // `extractSlot`: the cell of slot number n holds item `size - n`.
                    let index = shown.len() - 1 - slot;
                    if usize::try_from(self.selected) == Ok(index) {
                        selected = Some(slots.len());
                    }
                    slots.push((at, shown[index].clone()));
                    slot += 1;
                }
            }
        }
        let weight = self.weight_fraction();
        TooltipLayout {
            width: 96.0,
            height: self.tooltip_height(description_lines, 9) as f32,
            slots,
            selected,
            overflow,
            bar: [x + offset, y + grid_height + 4.0],
            // `Mth.mulAndTruncate(weight, 94)`, kept within the bar.
            fill: (weight * 94.0).trunc().clamp(0.0, 94.0),
            full: self.is_full(),
            label: if self.is_full() {
                Some("item.minecraft.bundle.full")
            } else {
                None
            },
            empty_description: None,
        }
    }

    /// `toImmutable`, written back into the container's component as a `Tag::List` of
    /// `ItemStack.CODEC` entries. Leaves any existing unsupported custom payload (non-list,
    /// malformed list entry, or malformed nested container) untouched.
    pub fn store(self, items: &Items, bundle: &mut ItemStack) {
        if self.component == "minecraft:bundle_contents" {
            let data = bundle.data_mut();
            data.bundle_contents = self.items;
            data.bundle_selected = BundleSelected(self.selected);
            bundle.tidy();
            return;
        }
        if Self::try_of(
            items,
            bundle,
            BundleSpec::custom(&self.component, self.capacity_multiplier),
        )
        .is_err()
        {
            return;
        }
        let info = items.get(&bundle.item);
        let tag = Tag::List(
            self.items
                .iter()
                .map(|s| crate::item_nbt::item_to_nbt(s, items))
                .collect(),
        );
        let default_matches = info
            .and_then(|i| i.custom_components.get(&self.component))
            .is_some_and(|default| default == &tag);
        if default_matches {
            bundle.data_mut().set_other(&self.component, None);
        } else {
            bundle.data_mut().set_other(&self.component, Some(tag));
        }
        bundle.tidy();
    }
}

/// Summary metrics derived in O(1) from a cached `(decoded_items, exact_weight)` entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BundleSummary {
    pub weight_fraction: f32,
    pub is_full: bool,
    pub bar: Option<(i32, u32)>,
    pub stack_count: u32,
}

#[derive(Clone, Debug)]
struct CachedBundle {
    items: Arc<[ItemStack]>,
    weight: Weight,
}

enum EvictionKey {
    Vanilla(u64, u64),
    Prototype(u64, u64),
    Custom(u64, u64),
}

struct VanillaCacheEntry {
    seq: u64,
    key: Vec<ItemStack>,
    value: Arc<CachedBundle>,
}

struct PrototypeCacheEntry {
    seq: u64,
    component: String,
    item: Arc<str>,
    value: Result<Arc<CachedBundle>, CustomBundleError>,
}

struct CustomCacheEntry {
    seq: u64,
    component: String,
    tag: Tag,
    value: Result<Arc<CachedBundle>, CustomBundleError>,
}

/// Bounded per-game memoization cache for decoded bundle-like contents and exact rational weight,
/// matching vanilla `BundleContents`'s memoized weight so hot draw paths (`item_bar`,
/// `tooltip_image`, `item_model_range_property`) avoid re-decoding NBT and re-walking nested
/// container trees on every frame.
///
/// Entries are keyed by exact component value (or `(component, item_id)` for prototype defaults)
/// and invalidated automatically whenever the loaded [`Items`] catalog changes.
#[derive(Default)]
pub struct BundleMetricsCache {
    catalog_id: u64,
    next_seq: u64,
    entry_count: usize,
    computations: u64,
    vanilla: HashMap<u64, Vec<VanillaCacheEntry>>,
    prototypes: HashMap<u64, Vec<PrototypeCacheEntry>>,
    custom: HashMap<u64, Vec<CustomCacheEntry>>,
    eviction: VecDeque<EvictionKey>,
}

impl BundleMetricsCache {
    const MAX_ENTRIES: usize = 512;

    /// Empty cache with no retained entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all cached entries while keeping the cumulative computation counter intact.
    pub fn clear(&mut self) {
        self.vanilla.clear();
        self.prototypes.clear();
        self.custom.clear();
        self.eviction.clear();
        self.entry_count = 0;
    }

    /// Total number of cache misses that decoded contents and evaluated exact weight.
    pub fn computations(&self) -> u64 {
        self.computations
    }

    fn sync_catalog(&mut self, items: &Items) {
        let id = items.bundle_cache_id();
        if self.catalog_id != id {
            self.clear();
            self.catalog_id = id;
        }
    }

    fn evict_if_full(&mut self) {
        while self.entry_count >= Self::MAX_ENTRIES {
            let Some(oldest) = self.eviction.pop_front() else {
                self.clear();
                break;
            };
            let removed = match oldest {
                EvictionKey::Vanilla(hash, seq) => {
                    Self::remove_bucket(&mut self.vanilla, hash, |e| e.seq == seq)
                }
                EvictionKey::Prototype(hash, seq) => {
                    Self::remove_bucket(&mut self.prototypes, hash, |e| e.seq == seq)
                }
                EvictionKey::Custom(hash, seq) => {
                    Self::remove_bucket(&mut self.custom, hash, |e| e.seq == seq)
                }
            };
            if removed {
                self.entry_count -= 1;
            }
        }
    }

    fn remove_bucket<T>(
        map: &mut HashMap<u64, Vec<T>>,
        hash: u64,
        pred: impl Fn(&T) -> bool,
    ) -> bool {
        let Some(bucket) = map.get_mut(&hash) else {
            return false;
        };
        let Some(pos) = bucket.iter().position(pred) else {
            return false;
        };
        bucket.swap_remove(pos);
        if bucket.is_empty() {
            map.remove(&hash);
        }
        true
    }

    fn empty_bundle() -> Arc<CachedBundle> {
        Arc::new(CachedBundle {
            items: Arc::from([]),
            weight: Weight::ZERO,
        })
    }

    fn hash_vanilla(inside: &[ItemStack]) -> u64 {
        let mut hasher = DefaultHasher::new();
        inside.hash(&mut hasher);
        hasher.finish()
    }

    fn hash_prototype(component: &str, item: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        component.hash(&mut hasher);
        item.hash(&mut hasher);
        hasher.finish()
    }

    fn hash_custom(component: &str, tag: &Tag) -> u64 {
        let mut hasher = DefaultHasher::new();
        component.hash(&mut hasher);
        tag.hash(&mut hasher);
        hasher.finish()
    }

    fn resolve(
        &mut self,
        items: &Items,
        stack: &ItemStack,
        spec: BundleSpec<'_>,
    ) -> Result<Option<Arc<CachedBundle>>, CustomBundleError> {
        self.sync_catalog(items);
        if spec.is_vanilla() {
            let Some(inside) = contents_of(items, stack) else {
                return Ok(None);
            };
            if inside.is_empty() {
                return Ok(Some(Self::empty_bundle()));
            }
            let hash = Self::hash_vanilla(inside);
            if let Some(bucket) = self.vanilla.get(&hash)
                && let Some(hit) = bucket.iter().find(|entry| entry.key.as_slice() == inside)
            {
                return Ok(Some(Arc::clone(&hit.value)));
            }
            self.computations += 1;
            let weight = contents_weight(items, inside);
            let value = Arc::new(CachedBundle {
                items: Arc::from(inside.to_vec()),
                weight,
            });
            self.evict_if_full();
            self.next_seq += 1;
            let seq = self.next_seq;
            self.vanilla
                .entry(hash)
                .or_default()
                .push(VanillaCacheEntry {
                    seq,
                    key: inside.to_vec(),
                    value: Arc::clone(&value),
                });
            self.eviction.push_back(EvictionKey::Vanilla(hash, seq));
            self.entry_count += 1;
            return Ok(Some(value));
        }

        let info = items.get(&stack.item);
        if !stack.carries_component(info, spec.component) {
            return Ok(None);
        }
        let override_tag = stack.data().and_then(|d| d.other(spec.component));
        let Some(tag) = override_tag else {
            let default_tag = info.and_then(|i| i.custom_components.get(spec.component));
            match default_tag {
                None => return Ok(Some(Self::empty_bundle())),
                Some(Tag::List(entries)) if entries.is_empty() => {
                    return Ok(Some(Self::empty_bundle()));
                }
                Some(default_tag) => {
                    let hash = Self::hash_prototype(spec.component, &stack.item);
                    if let Some(bucket) = self.prototypes.get(&hash)
                        && let Some(hit) = bucket
                            .iter()
                            .find(|e| e.component == spec.component && e.item == stack.item)
                    {
                        return hit.value.clone().map(Some);
                    }
                    self.computations += 1;
                    let computed = try_decode_custom_tag(items, default_tag).and_then(|decoded| {
                        let weight = contents_weight_for(items, &decoded, Some(spec.component))?;
                        Ok(Arc::new(CachedBundle {
                            items: Arc::from(decoded),
                            weight,
                        }))
                    });
                    self.evict_if_full();
                    self.next_seq += 1;
                    let seq = self.next_seq;
                    self.prototypes
                        .entry(hash)
                        .or_default()
                        .push(PrototypeCacheEntry {
                            seq,
                            component: spec.component.to_owned(),
                            item: Arc::clone(&stack.item),
                            value: computed.clone(),
                        });
                    self.eviction.push_back(EvictionKey::Prototype(hash, seq));
                    self.entry_count += 1;
                    return computed.map(Some);
                }
            }
        };

        if matches!(tag, Tag::List(entries) if entries.is_empty()) {
            return Ok(Some(Self::empty_bundle()));
        }
        let hash = Self::hash_custom(spec.component, tag);
        if let Some(bucket) = self.custom.get(&hash)
            && let Some(hit) = bucket
                .iter()
                .find(|entry| entry.component == spec.component && entry.tag == *tag)
        {
            return hit.value.clone().map(Some);
        }
        self.computations += 1;
        let computed = try_decode_custom_tag(items, tag).and_then(|decoded| {
            let weight = contents_weight_for(items, &decoded, Some(spec.component))?;
            Ok(Arc::new(CachedBundle {
                items: Arc::from(decoded),
                weight,
            }))
        });
        self.evict_if_full();
        self.next_seq += 1;
        let seq = self.next_seq;
        self.custom.entry(hash).or_default().push(CustomCacheEntry {
            seq,
            component: spec.component.to_owned(),
            tag: tag.clone(),
            value: computed.clone(),
        });
        self.eviction.push_back(EvictionKey::Custom(hash, seq));
        self.entry_count += 1;
        computed.map(Some)
    }

    /// Cached fullness, full flag, vanilla-style bar `(width, rgb)`, and stored stack count for
    /// `stack` under `spec`. Repeated calls for the same component value (even under different
    /// capacity multipliers) reuse the cached exact weight without re-decoding NBT.
    pub fn metrics_for(
        &mut self,
        items: &Items,
        stack: &ItemStack,
        spec: BundleSpec<'_>,
    ) -> Result<Option<BundleSummary>, CustomBundleError> {
        let Some(cached) = self.resolve(items, stack, spec)? else {
            return Ok(None);
        };
        let mult = spec.multiplier();
        Ok(Some(BundleSummary {
            weight_fraction: weight_fraction_for(cached.weight, mult),
            is_full: is_full_for(cached.weight, mult),
            bar: bar_for(cached.weight, mult),
            stack_count: cached.items.len() as u32,
        }))
    }

    /// Cached front-first stored stacks of `stack` under `spec`.
    pub fn slots_for(
        &mut self,
        items: &Items,
        stack: &ItemStack,
        spec: BundleSpec<'_>,
    ) -> Result<Option<Arc<[ItemStack]>>, CustomBundleError> {
        Ok(self
            .resolve(items, stack, spec)?
            .map(|c| Arc::clone(&c.items)))
    }

    /// Remaining capacity in `bundle` for `offered` under `spec`, reusing `bundle`'s cached exact
    /// weight while still validating any nested custom container inside `offered`.
    pub fn room_for(
        &mut self,
        items: &Items,
        bundle: &ItemStack,
        spec: BundleSpec<'_>,
        offered: &ItemStack,
    ) -> Result<Option<u32>, CustomBundleError> {
        let Some(cached) = self.resolve(items, bundle, spec)? else {
            return Ok(None);
        };
        let custom = (!spec.is_vanilla()).then_some(spec.component);
        let offered_weight = item_weight_for(items, offered, custom)?;
        Ok(Some(
            spec.capacity().sub(cached.weight).how_many(offered_weight),
        ))
    }
}

/// `BundleContents.Mutable`: vanilla `minecraft:bundle_contents` with capacity multiplier `1`.
pub struct Contents(CustomBundleContents);

impl Contents {
    /// Width of a bundle tooltip component in GUI pixels (`ClientBundleTooltip.GRID_WIDTH`).
    pub const TOOLTIP_WIDTH: i32 = 96;

    /// Whether `stack` carries `minecraft:bundle_contents`, without cloning its items or
    /// recomputing their weight.
    pub fn is_bundle(items: &Items, stack: &ItemStack) -> bool {
        contents_of(items, stack).is_some()
    }

    /// `ClientBundleTooltip.getHeight`: height of this bundle tooltip component in GUI pixels.
    /// `description_lines` is the empty-bundle description line count in the active font (ignored
    /// when non-empty); `line_height` is `Font.lineHeight` (9 in vanilla).
    pub fn tooltip_height(&self, description_lines: usize, line_height: i32) -> i32 {
        self.0.tooltip_height(description_lines, line_height)
    }

    /// Lay out this prepared bundle's tooltip (`ClientBundleTooltip`) at `(x, y)` centred inside
    /// `box_width`, reusing these retained items and weight without re-reading the stack.
    pub fn tooltip_layout(
        &self,
        x: f32,
        y: f32,
        box_width: f32,
        description_lines: usize,
    ) -> TooltipLayout {
        self.0.tooltip_layout(x, y, box_width, description_lines)
    }

    /// A component collection may belong to any item, including an item with
    /// no previous bundle_contents component. Reuse the same weight model.
    pub fn from_items(items: &Items, inside: Vec<ItemStack>) -> Self {
        Self(
            CustomBundleContents::try_from_items(items, BundleSpec::VANILLA, inside)
                .expect("vanilla bundle contents cannot fail custom format validation"),
        )
    }
    /// The bundle's contents; None when the stack is no bundle.
    pub fn of(items: &Items, stack: &ItemStack) -> Option<Self> {
        CustomBundleContents::of(items, stack, BundleSpec::VANILLA).map(Self)
    }

    /// Remaining capacity for `offered` in a stack's bundle component, without cloning contents.
    pub fn room_in_stack(items: &Items, bundle: &ItemStack, offered: &ItemStack) -> Option<u32> {
        CustomBundleContents::room_in_stack(items, bundle, BundleSpec::VANILLA, offered)
    }

    /// `getMaxAmountToAdd` for `stack`'s kind of item.
    pub fn room_for(&self, items: &Items, stack: &ItemStack) -> u32 {
        self.0.room_for(items, stack)
    }

    /// `tryInsert`: what fits moves to the front, merged with its kind. `fits` is the stack's
    /// `canFitInsideContainerItems` answer, which `canItemBeInBundle` asks and the caller
    /// gives, since the parts of the stack's item may change it; an empty stack never goes in.
    pub fn try_insert(&mut self, items: &Items, stack: &mut ItemStack, fits: bool) -> u32 {
        self.0.try_insert(items, stack, fits)
    }

    /// `removeOne`: the selected stack, else the first; nothing is selected after.
    pub fn remove_one(&mut self, items: &Items) -> Option<ItemStack> {
        self.0.remove_one(items)
    }

    /// `getSelectedItemIndex`.
    pub fn selected(&self) -> i32 {
        self.0.selected()
    }

    /// The stacks inside, in display order (the first is the next out).
    pub fn items(&self) -> &[ItemStack] {
        self.0.items()
    }

    /// `getNumberOfItemsToShow`: a full grid shows 11 and a `+N`, so the last
    /// row keeps its fourth cell for the count.
    pub fn shown_count(&self) -> usize {
        self.0.shown_count()
    }

    /// The items the tooltip shows, and how many sit beyond them.
    pub fn shown(&self) -> (&[ItemStack], u32) {
        self.0.shown()
    }

    /// The `Fraction` weight as the float the progress bar reads.
    pub fn weight_fraction(&self) -> f32 {
        self.0.weight_fraction()
    }

    /// `Fraction.ONE`: a full bundle draws the full bar and says so.
    pub fn is_full(&self) -> bool {
        self.0.is_full()
    }

    /// `BundleItem.isBarVisible`, `getBarWidth` and `getBarColor`: the fullness bar
    /// (width, RGB), blue until the bundle is full, then red.
    pub fn bar(&self) -> Option<(i32, u32)> {
        self.0.bar()
    }

    /// `toImmutable`, written back into the bundle's component.
    pub fn store(self, bundle: &mut ItemStack) {
        let data = bundle.data_mut();
        data.bundle_contents = self.0.items;
        data.bundle_selected = BundleSelected(self.0.selected);
        bundle.tidy();
    }
}

/// `BundleItem.toggleSelectedItem`: select `index`, or nothing when it is the
/// selected one already or outside the contents.
pub fn toggle_selected(bundle: &mut ItemStack, index: i32) {
    let Some(data) = bundle.extra.as_deref_mut() else {
        return;
    };
    let inside = (0..data.bundle_contents.len() as i32).contains(&index);
    let selected = if data.bundle_selected.0 != index && inside {
        index
    } else {
        -1
    };
    data.bundle_selected = BundleSelected(selected);
}

#[cfg(test)]
mod tests {
    use super::Weight;

    #[test]
    fn weights_add_up_exactly() {
        let w = Weight::new(1, 64)
            .times(32)
            .add(Weight::new(1, 16).times(4));
        assert_eq!(w, Weight::new(3, 4));
        assert_eq!(Weight::ONE.sub(w).how_many(Weight::new(1, 64)), 16);
        assert_eq!(Weight::ONE.sub(Weight::ONE).how_many(Weight::ONE), 0);
        // An over-full bundle (made by a command) has no room.
        let over = Weight::ONE.sub(Weight::new(5, 4));
        assert_eq!(over.how_many(Weight::new(1, 64)), 0);
    }
}

// ------------------------------------------------------------- the tooltip

/// `ClientBundleTooltip`'s geometry, relative to the tooltip box's top-left:
/// the 96 pixel column of a 4 wide slot grid and the progress bar under it.
#[derive(Clone, Debug, PartialEq)]
pub struct TooltipLayout {
    /// The component's own width (96) and height, the text's excluded.
    pub width: f32,
    pub height: f32,
    /// Each slot's top-left and the stack in it, in drawing order.
    pub slots: Vec<([f32; 2], ItemStack)>,
    /// The position in `slots` of the selected item, which is drawn highlighted.
    pub selected: Option<usize>,
    /// The `+N` of the items past the grid: its top-left and the count.
    pub overflow: Option<([f32; 2], u32)>,
    /// The progress bar's top-left, its fill's width out of 94, and whether
    /// the weight fills it (the full sprite).
    pub bar: [f32; 2],
    pub fill: f32,
    pub full: bool,
    /// `item.minecraft.bundle.empty` / `.full`, centred over the bar.
    pub label: Option<&'static str>,
    /// An empty bundle's description, wrapped at `width` from here.
    pub empty_description: Option<[f32; 2]>,
}

/// The tooltip of a bundle-like container (`ClientBundleTooltip`), or None when
/// the stack does not carry `spec.component`. `x`/`y` are the box's top-left,
/// `box_width` the tooltip's width, which the component is centred in.
pub fn tooltip_layout_for(
    items: &Items,
    stack: &ItemStack,
    spec: BundleSpec<'_>,
    x: f32,
    y: f32,
    box_width: f32,
    description_lines: usize,
) -> Option<TooltipLayout> {
    CustomBundleContents::of(items, stack, spec)
        .map(|contents| contents.tooltip_layout(x, y, box_width, description_lines))
}

/// The tooltip of a bundle's contents (`ClientBundleTooltip`), or None when
/// the stack is not a bundle. `x`/`y` are the box's top-left, `box_width` the
/// tooltip's width, which the component is centred in.
pub fn tooltip_layout(
    items: &Items,
    stack: &ItemStack,
    x: f32,
    y: f32,
    box_width: f32,
    description_lines: usize,
) -> Option<TooltipLayout> {
    tooltip_layout_for(
        items,
        stack,
        BundleSpec::VANILLA,
        x,
        y,
        box_width,
        description_lines,
    )
}

#[cfg(test)]
mod tooltip_tests {
    use super::*;

    /// The prepared resources, when they are on this machine: the tooltip
    /// asks the item registry whether a stack is a bundle.
    fn items() -> Option<Items> {
        crate::items::prepared_items()
    }

    fn bundle_with(counts: &[(&str, u32)]) -> ItemStack {
        let mut bundle = ItemStack::new("minecraft:bundle", 1);
        bundle.data_mut().bundle_contents = counts
            .iter()
            .map(|(item, n)| ItemStack::new(item, *n))
            .collect();
        bundle
    }

    #[test]
    fn a_grid_shows_twelve_stacks_and_counts_the_rest() {
        let Some(items) = items() else { return };
        let twelve: Vec<(&str, u32)> = (0..12).map(|_| ("minecraft:stone", 1)).collect();
        let layout = tooltip_layout(&items, &bundle_with(&twelve), 0.0, 0.0, 96.0, 2).unwrap();
        assert_eq!(layout.slots.len(), 12);
        assert_eq!(layout.overflow, None);
        assert_eq!(layout.height, 3.0 * 24.0 + 13.0 + 8.0);
        // Rows fill bottom-up and columns right-to-left: the twelfth item is in
        // the bottom-right cell, the first in the top-left.
        assert_eq!(layout.slots[0].0, [96.0 - 24.0, 3.0 * 24.0 - 24.0]);
        assert_eq!(&*layout.slots[0].1.item, "minecraft:stone");
        assert_eq!(layout.slots[11].0, [0.0, 0.0]);
        // A wider tooltip centres the 96 pixel column in it.
        let centred = tooltip_layout(&items, &bundle_with(&twelve), 0.0, 0.0, 120.0, 2).unwrap();
        assert_eq!(centred.slots[0].0[0], 12.0 + 96.0 - 24.0);
        // Thirteen stacks: `getNumberOfItemsToShow` leaves the last row room
        // for the count, so 8 slots show and `+6` (four stones and two dirt)
        // takes the first cell.
        let mut counts = twelve;
        counts.push(("minecraft:dirt", 2));
        let layout = tooltip_layout(&items, &bundle_with(&counts), 0.0, 0.0, 96.0, 2).unwrap();
        assert_eq!(layout.slots.len(), 8);
        assert_eq!(layout.overflow, Some(([96.0 - 24.0, 48.0], 6)));
    }

    #[test]
    fn the_bar_fills_with_the_weight_and_an_empty_bundle_explains_itself() {
        let Some(items) = items() else { return };
        let layout = tooltip_layout(&items, &bundle_with(&[]), 4.0, 8.0, 96.0, 2).unwrap();
        assert!(layout.slots.is_empty());
        assert_eq!(layout.fill, 0.0);
        assert_eq!(layout.label, Some("item.minecraft.bundle.empty"));
        assert_eq!(layout.empty_description, Some([4.0, 8.0]));
        assert_eq!(layout.bar, [4.0, 8.0 + 18.0 + 4.0]);
        // A bundle holding one 64 stack is full: the whole bar, the full label.
        let layout = tooltip_layout(
            &items,
            &bundle_with(&[("minecraft:stone", 64)]),
            0.0,
            0.0,
            96.0,
            2,
        )
        .unwrap();
        assert!(layout.full);
        assert_eq!(layout.fill, 94.0);
        assert_eq!(layout.label, Some("item.minecraft.bundle.full"));
        // Something that is not a bundle has no component.
        let dirt = ItemStack::new("minecraft:dirt", 1);
        assert!(tooltip_layout(&items, &dirt, 0.0, 0.0, 96.0, 2).is_none());
    }
    /// `getEmptyBundleBackgroundHeight`: the description's wrapped height (9 a
    /// line) plus the bar's 13 and the tooltip's 8, not a fixed two lines.
    #[test]
    fn an_empty_bundles_height_follows_its_description_lines() {
        let Some(items) = items() else { return };
        let layout = tooltip_layout(&items, &bundle_with(&[]), 4.0, 8.0, 96.0, 3).unwrap();
        assert_eq!(layout.height, 3.0 * 9.0 + 13.0 + 8.0);
        assert_eq!(layout.bar, [4.0, 8.0 + 3.0 * 9.0 + 4.0]);
        assert_eq!(layout.empty_description, Some([4.0, 8.0]));
    }
}

#[cfg(test)]
mod custom_bundle_tests {
    use super::*;
    use std::fs;
    use sunrise_world::loot::PackOverlay;

    fn test_catalog(name: &str) -> Items {
        let root = std::env::temp_dir().join(format!(
            "sunrise-custom-bundle-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("data/minecraft/loot_table")).unwrap();
        let json = r#"{
            "minecraft:stone": {"minecraft:max_stack_size": 64},
            "minecraft:ender_pearl": {"minecraft:max_stack_size": 16},
            "minecraft:diamond_sword": {"minecraft:max_stack_size": 1},
            "minecraft:bundle": {"minecraft:max_stack_size": 1, "minecraft:bundle_contents": []},
            "runes:pouch": {"minecraft:max_stack_size": 1, "runes:pouch_contents": []},
            "runes:compound_pouch": {"minecraft:max_stack_size": 1, "runes:compound_contents": {"items": [], "tier": 2}},
            "runes:entries_pouch": {"minecraft:max_stack_size": 1, "runes:entries_contents": {"entries": [], "tier": 2}}
        }"#;
        fs::write(root.join("items.json"), json).unwrap();
        let defaults = sunrise_world::item_defaults::read(&root).unwrap();
        let catalog = Items::catalog(&root, &PackOverlay::NONE, &defaults).unwrap();
        fs::remove_dir_all(&root).unwrap();
        catalog
    }

    #[test]
    fn custom_component_multipliers_split_stacks_and_scale_bar() {
        let items = test_catalog("multipliers");
        let spec4 = BundleSpec::custom("runes:pouch_contents", 4);
        assert_eq!(
            BundleSpec::custom("runes:pouch_contents", 0).multiplier(),
            1
        );

        let mut pouch = ItemStack::new("runes:pouch", 1);
        let mut contents = CustomBundleContents::of(&items, &pouch, spec4).unwrap();
        assert_eq!(contents.capacity_multiplier(), 4);
        assert_eq!(contents.bar(), None);
        assert!(!contents.is_full());
        assert_eq!(
            contents.room_for(&items, &ItemStack::new("minecraft:stone", 1)),
            256
        );
        assert_eq!(
            contents.room_for(&items, &ItemStack::new("minecraft:ender_pearl", 1)),
            64
        );

        // Insert 16 pearls (weight 1), then 10 pearls, then 10 more pearls:
        // merges 6 into the partial 10-stack (capped at max_stack 16), splits remaining 4 to front.
        let mut p1 = ItemStack::new("minecraft:ender_pearl", 16);
        assert_eq!(contents.try_insert(&items, &mut p1, true), 16);
        assert_eq!(p1.count, 0);
        let mut p2 = ItemStack::new("minecraft:ender_pearl", 10);
        assert_eq!(contents.try_insert(&items, &mut p2, true), 10);
        let mut p3 = ItemStack::new("minecraft:ender_pearl", 10);
        assert_eq!(contents.try_insert(&items, &mut p3, true), 10);
        assert_eq!(
            contents
                .items()
                .iter()
                .map(|s| (&*s.item, s.count))
                .collect::<Vec<_>>(),
            vec![
                ("minecraft:ender_pearl", 4),
                ("minecraft:ender_pearl", 16),
                ("minecraft:ender_pearl", 16),
            ]
        );
        // 36/16 = 9/4 weight out of 4 => 9/16 fullness, partial blue bar.
        assert!((contents.weight_fraction() - 9.0 / 16.0).abs() < 1e-6);
        assert!(!contents.is_full());
        assert_eq!(contents.bar(), Some((1 + (9 * 12) / 16, 0x7087FF)));

        // Fill the remaining 7/4 weight = 112 stones: splits into [48, 64] at front.
        let mut stones = ItemStack::new("minecraft:stone", 150);
        assert_eq!(contents.try_insert(&items, &mut stones, true), 112);
        assert_eq!(stones.count, 38);
        assert!(contents.is_full());
        assert_eq!(contents.bar(), Some((13, 0xFF5454)));
        assert_eq!(contents.items()[0].count, 48);
        assert_eq!(contents.items()[1].count, 64);
        assert!(
            contents
                .items()
                .iter()
                .all(|s| s.count <= max_stack(&items, s))
        );

        // Persist and reload under multipliers 4, 8, and 12.
        contents.store(&items, &mut pouch);
        assert_eq!(
            CustomBundleContents::room_in_stack(
                &items,
                &pouch,
                spec4,
                &ItemStack::new("minecraft:stone", 1)
            ),
            Some(0)
        );
        assert_eq!(
            CustomBundleContents::room_in_stack(
                &items,
                &pouch,
                BundleSpec::custom("runes:pouch_contents", 8),
                &ItemStack::new("minecraft:stone", 1)
            ),
            Some(256)
        );
        assert_eq!(
            CustomBundleContents::room_in_stack(
                &items,
                &pouch,
                BundleSpec::custom("runes:pouch_contents", 12),
                &ItemStack::new("minecraft:stone", 1)
            ),
            Some(512)
        );
    }

    #[test]
    fn custom_component_nested_occupancy_bees_and_removal() {
        let items = test_catalog("nested");
        let spec = BundleSpec::custom("runes:pouch_contents", 4);

        // Nested vanilla bundle with 16 stones weighs 1/4 + 1/16 = 5/16.
        let mut vanilla_inner = ItemStack::new("minecraft:bundle", 1);
        vanilla_inner.data_mut().bundle_contents = vec![ItemStack::new("minecraft:stone", 16)];

        // Nested custom pouch with 8 pearls weighs 1/2 + 1/16 = 9/16.
        let mut custom_inner = ItemStack::new("runes:pouch", 1);
        let mut inner_c = CustomBundleContents::of(&items, &custom_inner, spec).unwrap();
        let mut pearls = ItemStack::new("minecraft:ender_pearl", 8);
        assert_eq!(inner_c.try_insert(&items, &mut pearls, true), 8);
        inner_c.store(&items, &mut custom_inner);

        // Beehive with non-empty bees weighs 1.
        let mut beehive = ItemStack::new("minecraft:stone", 1);
        beehive.data_mut().set_other(
            "minecraft:bees",
            Some(Tag::List(vec![Tag::Compound(Default::default())])),
        );

        let mut outer = ItemStack::new("runes:pouch", 1);
        let mut outer_c = CustomBundleContents::of(&items, &outer, spec).unwrap();
        assert_eq!(outer_c.try_insert(&items, &mut vanilla_inner, true), 1);
        assert_eq!(outer_c.try_insert(&items, &mut custom_inner, true), 1);
        assert_eq!(outer_c.try_insert(&items, &mut beehive, true), 1);
        // Total weight = 5/16 + 9/16 + 1 = 30/16 = 15/8. Remaining = 4 - 15/8 = 17/8 = 136 stones.
        assert_eq!(
            outer_c.room_for(&items, &ItemStack::new("minecraft:stone", 1)),
            136
        );
        outer_c.store(&items, &mut outer);
        assert_eq!(
            CustomBundleContents::room_in_stack(
                &items,
                &outer,
                spec,
                &ItemStack::new("minecraft:stone", 1)
            ),
            Some(136)
        );

        // Component removal (`!runes:pouch_contents`) hides the container.
        let mut removed = ItemStack::new("runes:pouch", 1);
        removed.data_mut().set_other(
            "!runes:pouch_contents",
            Some(Tag::Compound(Default::default())),
        );
        assert!(CustomBundleContents::of(&items, &removed, spec).is_none());
    }

    #[test]
    fn custom_component_extract_destroyed_and_tooltip_layout() {
        let items = test_catalog("extract_destroy_tooltip");
        let list_spec = BundleSpec::custom("runes:pouch_contents", 4);

        // Emptying a default-component pouch restores the default stack cleanly.
        let mut pouch = ItemStack::new("runes:pouch", 1);
        let mut contents = CustomBundleContents::of(&items, &pouch, list_spec).unwrap();
        let mut stones = ItemStack::new("minecraft:stone", 128);
        assert_eq!(contents.try_insert(&items, &mut stones, true), 128);
        contents.store(&items, &mut pouch);

        // Tooltip layout scales bar fill by capacity multiplier: 128/256 = 0.5 -> 47px, not full.
        let layout = tooltip_layout_for(&items, &pouch, list_spec, 0.0, 0.0, 96.0, 2).unwrap();
        assert_eq!(layout.slots.len(), 2);
        assert!(!layout.full);
        assert_eq!(layout.fill, 47.0);
        assert_eq!(layout.label, None);

        // Extract partial from front preserves remainder at index 0.
        let mut contents = CustomBundleContents::of(&items, &pouch, list_spec).unwrap();
        let taken = contents.extract_front(&items, 10).unwrap();
        assert_eq!(taken.count, 10);
        assert_eq!(contents.items()[0].count, 54);
        contents.store(&items, &mut pouch);

        // Destroyed spills remaining stacks and clears the override back to the default stack.
        let spilled = destroyed_component(&items, &mut pouch, "runes:pouch_contents");
        assert_eq!(
            spilled.iter().map(|s| s.count).collect::<Vec<_>>(),
            vec![54, 64]
        );
        assert!(pouch.same_item_same_components(&ItemStack::new("runes:pouch", 1)));
    }

    #[test]
    fn custom_component_deep_nesting_past_32_computes_exact_weight() {
        let items = test_catalog("deep_nesting");
        let spec4 = BundleSpec::custom("runes:pouch_contents", 4);

        // Build 40 nested custom pouches (empty innermost pouch wrapped 39 times).
        // Exact weight is 40 * (1/16) = 5/2 = 2.5 (not truncated at depth 32, which would have
        // returned 32/16 + 1 = 3.0 by falling back to max_stack = 1 at level 32).
        let mut nested = ItemStack::new("runes:pouch", 1);
        for _ in 0..39 {
            let mut outer = ItemStack::new("runes:pouch", 1);
            let mut contents = CustomBundleContents::of(&items, &outer, spec4).unwrap();
            let mut offer = nested;
            assert_eq!(contents.try_insert(&items, &mut offer, true), 1);
            contents.store(&items, &mut outer);
            nested = outer;
        }

        // Refused by capacity-2 pouch (2 < 5/2).
        let spec2 = BundleSpec::custom("runes:pouch_contents", 2);
        let mut cap2 = ItemStack::new("runes:pouch", 1);
        let mut cap2_contents = CustomBundleContents::of(&items, &cap2, spec2).unwrap();
        assert_eq!(cap2_contents.room_for(&items, &nested), 0);
        let mut refused = nested.clone();
        assert_eq!(cap2_contents.try_insert(&items, &mut refused, true), 0);
        assert_eq!(refused.count, 1);
        cap2_contents.store(&items, &mut cap2);

        // Accepted into capacity-3 pouch with exact remaining weight 3 - 5/2 = 1/2 = 32 stones.
        let spec3 = BundleSpec::custom("runes:pouch_contents", 3);
        let mut cap3 = ItemStack::new("runes:pouch", 1);
        let mut cap3_contents = CustomBundleContents::of(&items, &cap3, spec3).unwrap();
        assert_eq!(cap3_contents.room_for(&items, &nested), 1);
        let mut offer = nested;
        assert_eq!(cap3_contents.try_insert(&items, &mut offer, true), 1);
        assert_eq!(
            cap3_contents.room_for(&items, &ItemStack::new("minecraft:stone", 1)),
            32
        );
        cap3_contents.store(&items, &mut cap3);
        assert_eq!(
            CustomBundleContents::try_room_in_stack(
                &items,
                &cap3,
                spec3,
                &ItemStack::new("minecraft:stone", 1),
            ),
            Ok(Some(32))
        );
    }

    #[test]
    fn custom_component_refuses_non_list_formats_without_overwriting() {
        let items = test_catalog("unsupported_formats");

        // 1. Compound without "items" (e.g. {"entries": [], "tier": 2}) in prototype default.
        let entries_spec = BundleSpec::custom("runes:entries_contents", 2);
        let mut entries_pouch = ItemStack::new("runes:entries_pouch", 1);
        let original_entries = entries_pouch.clone();
        assert_eq!(
            CustomBundleContents::try_of(&items, &entries_pouch, entries_spec).err(),
            Some(CustomBundleError::UnsupportedFormat)
        );
        assert_eq!(
            CustomBundleContents::try_room_in_stack(
                &items,
                &entries_pouch,
                entries_spec,
                &ItemStack::new("minecraft:stone", 1),
            ),
            Err(CustomBundleError::UnsupportedFormat)
        );
        assert!(CustomBundleContents::of(&items, &entries_pouch, entries_spec).is_none());
        assert!(
            tooltip_layout_for(&items, &entries_pouch, entries_spec, 0.0, 0.0, 96.0, 2).is_none()
        );
        assert!(
            destroyed_component(&items, &mut entries_pouch, "runes:entries_contents").is_empty()
        );
        assert!(entries_pouch.same_item_same_components(&original_entries));
        // Even direct `store` refuses to overwrite a non-list custom component.
        CustomBundleContents::try_from_items(
            &items,
            entries_spec,
            vec![ItemStack::new("minecraft:stone", 1)],
        )
        .unwrap()
        .store(&items, &mut entries_pouch);
        assert!(entries_pouch.same_item_same_components(&original_entries));

        // 2. Compound WITH "items" ({"items": [], "tier": 2}) is also refused (single List schema).
        let compound_spec = BundleSpec::custom("runes:compound_contents", 2);
        let mut compound_pouch = ItemStack::new("runes:compound_pouch", 1);
        let original_compound = compound_pouch.clone();
        assert_eq!(
            CustomBundleContents::try_of(&items, &compound_pouch, compound_spec).err(),
            Some(CustomBundleError::UnsupportedFormat)
        );
        assert!(
            destroyed_component(&items, &mut compound_pouch, "runes:compound_contents").is_empty()
        );
        assert!(compound_pouch.same_item_same_components(&original_compound));

        // 3. Stack override holding a compound or scalar tag on an otherwise list-backed item.
        let list_spec = BundleSpec::custom("runes:pouch_contents", 2);
        let bad_tag = Tag::compound([("entries", Tag::List(Vec::new())), ("tier", Tag::Int(2))]);
        let mut overridden = ItemStack::new("runes:pouch", 1);
        overridden
            .data_mut()
            .set_other("runes:pouch_contents", Some(bad_tag.clone()));
        assert_eq!(
            CustomBundleContents::try_of(&items, &overridden, list_spec).err(),
            Some(CustomBundleError::UnsupportedFormat)
        );
        assert!(destroyed_component(&items, &mut overridden, "runes:pouch_contents").is_empty());
        assert_eq!(
            overridden
                .data()
                .and_then(|d| d.other("runes:pouch_contents")),
            Some(&bad_tag)
        );

        let mut scalar = ItemStack::new("runes:pouch", 1);
        scalar
            .data_mut()
            .set_other("runes:pouch_contents", Some(Tag::Int(7)));
        assert_eq!(
            CustomBundleContents::try_of(&items, &scalar, list_spec).err(),
            Some(CustomBundleError::UnsupportedFormat)
        );
    }

    #[test]
    fn custom_component_overfull_contents_refuse_insert_clamp_bar_and_allow_extract() {
        let items = test_catalog("overfull");
        let spec4 = BundleSpec::custom("runes:pouch_contents", 4);
        let spec2 = BundleSpec::custom("runes:pouch_contents", 2);

        // Fill a pouch to weight 3 (192 stones) under multiplier 4, then inspect under multiplier 2.
        let mut pouch = ItemStack::new("runes:pouch", 1);
        let mut contents = CustomBundleContents::of(&items, &pouch, spec4).unwrap();
        let mut stones = ItemStack::new("minecraft:stone", 192);
        assert_eq!(contents.try_insert(&items, &mut stones, true), 192);
        contents.store(&items, &mut pouch);

        let mut overfull = CustomBundleContents::try_of(&items, &pouch, spec2)
            .unwrap()
            .unwrap();
        assert!(overfull.is_full());
        assert!((overfull.weight_fraction() - 1.5).abs() < 1e-6);
        assert_eq!(overfull.bar(), Some((13, 0xFF5454)));
        assert_eq!(
            overfull.room_for(&items, &ItemStack::new("minecraft:stone", 1)),
            0
        );
        let mut extra = ItemStack::new("minecraft:stone", 8);
        assert_eq!(overfull.try_insert(&items, &mut extra, true), 0);
        assert_eq!(extra.count, 8);

        let layout = tooltip_layout_for(&items, &pouch, spec2, 0.0, 0.0, 96.0, 2).unwrap();
        assert!(layout.full);
        assert_eq!(layout.fill, 94.0);
        assert_eq!(layout.label, Some("item.minecraft.bundle.full"));

        // Front extraction still works on overfull contents and preserves the remainder at index 0.
        let taken = overfull.extract_front(&items, 10).unwrap();
        assert_eq!(taken.count, 10);
        assert_eq!(overfull.items()[0].count, 54);
        assert!(overfull.is_full());
    }

    #[test]
    fn custom_component_malformed_list_entries_refuse_whole_payload_without_overwriting() {
        let items = test_catalog("malformed_entries");
        let spec = BundleSpec::custom("runes:pouch_contents", 2);
        let valid_stone = Tag::compound([
            ("id", Tag::String("minecraft:stone".into())),
            ("count", Tag::Int(12)),
        ]);

        // Every single invalid ItemStack.CODEC entry shape must refuse the ENTIRE list (even when
        // paired with a valid sibling stack) and leave the stack untouched across all operations.
        let malformed_cases: Vec<(&str, Tag)> = vec![
            ("non_compound_int", Tag::Int(42)),
            (
                "air_stack",
                Tag::compound([
                    ("id", Tag::String("minecraft:air".into())),
                    ("count", Tag::Int(1)),
                ]),
            ),
            (
                "bare_air_stack",
                Tag::compound([("id", Tag::String("air".into())), ("count", Tag::Int(1))]),
            ),
            (
                "invalid_identifier",
                Tag::compound([
                    ("id", Tag::String("NOT A VALID ID".into())),
                    ("count", Tag::Int(5)),
                ]),
            ),
            ("missing_id", Tag::compound([("count", Tag::Int(3))])),
            (
                "count_zero",
                Tag::compound([
                    ("id", Tag::String("minecraft:stone".into())),
                    ("count", Tag::Int(0)),
                ]),
            ),
            (
                "count_above_99",
                Tag::compound([
                    ("id", Tag::String("minecraft:stone".into())),
                    ("count", Tag::Int(200)),
                ]),
            ),
            (
                "count_wrong_type_string",
                Tag::compound([
                    ("id", Tag::String("minecraft:stone".into())),
                    ("count", Tag::String("12".into())),
                ]),
            ),
            (
                "count_wrong_type_float",
                Tag::compound([
                    ("id", Tag::String("minecraft:stone".into())),
                    ("count", Tag::Float(1.5f32.to_bits())),
                ]),
            ),
            (
                "components_non_compound",
                Tag::compound([
                    ("id", Tag::String("minecraft:stone".into())),
                    ("count", Tag::Int(1)),
                    ("components", Tag::String("not-a-compound".into())),
                ]),
            ),
            (
                "transient_component",
                Tag::compound([
                    ("id", Tag::String("minecraft:stone".into())),
                    ("count", Tag::Int(1)),
                    (
                        "components",
                        Tag::compound([(
                            "minecraft:creative_slot_lock",
                            Tag::Compound(Default::default()),
                        )]),
                    ),
                ]),
            ),
            (
                "unknown_registry_id",
                Tag::compound([
                    ("id", Tag::String("mod:removed_item".into())),
                    ("count", Tag::Int(1)),
                ]),
            ),
            (
                "unknown_minecraft_id",
                Tag::compound([
                    ("id", Tag::String("minecraft:nonexistent_item".into())),
                    ("count", Tag::Int(2)),
                ]),
            ),
        ];

        for (case, bad_entry) in malformed_cases {
            let payload = Tag::List(vec![bad_entry, valid_stone.clone()]);
            let mut pouch = ItemStack::new("runes:pouch", 1);
            pouch
                .data_mut()
                .set_other("runes:pouch_contents", Some(payload.clone()));
            let original = pouch.clone();

            assert_eq!(
                CustomBundleContents::try_of(&items, &pouch, spec).err(),
                Some(CustomBundleError::UnsupportedFormat),
                "{case}: try_of must refuse whole payload"
            );
            assert_eq!(
                CustomBundleContents::try_room_in_stack(
                    &items,
                    &pouch,
                    spec,
                    &ItemStack::new("minecraft:stone", 1),
                ),
                Err(CustomBundleError::UnsupportedFormat),
                "{case}: try_room_in_stack must refuse whole payload"
            );
            assert!(
                CustomBundleContents::of(&items, &pouch, spec).is_none(),
                "{case}: of must return None"
            );
            assert!(
                tooltip_layout_for(&items, &pouch, spec, 0.0, 0.0, 96.0, 2).is_none(),
                "{case}: tooltip_layout_for must return None"
            );
            assert!(
                destroyed_component(&items, &mut pouch, "runes:pouch_contents").is_empty(),
                "{case}: destroyed_component must not spill or clear malformed payload"
            );
            assert!(
                pouch.same_item_same_components(&original),
                "{case}: stack must stay untouched after destroyed_component"
            );

            // Even direct `store` refuses to overwrite a list containing a malformed entry.
            CustomBundleContents::try_from_items(
                &items,
                spec,
                vec![ItemStack::new("minecraft:stone", 1)],
            )
            .unwrap()
            .store(&items, &mut pouch);
            assert_eq!(
                pouch.data().and_then(|d| d.other("runes:pouch_contents")),
                Some(&payload),
                "{case}: store must not overwrite malformed list payload"
            );
        }

        // Valid `ItemStack.CODEC` entries (including omitted `count`, which defaults to 1) decode.
        let mut valid_pouch = ItemStack::new("runes:pouch", 1);
        valid_pouch.data_mut().set_other(
            "runes:pouch_contents",
            Some(Tag::List(vec![
                Tag::compound([("id", Tag::String("minecraft:stone".into()))]),
                valid_stone,
            ])),
        );
        let decoded = CustomBundleContents::try_of(&items, &valid_pouch, spec)
            .unwrap()
            .unwrap();
        assert_eq!(
            decoded
                .items()
                .iter()
                .map(|s| (&*s.item, s.count))
                .collect::<Vec<_>>(),
            vec![("minecraft:stone", 1), ("minecraft:stone", 12)]
        );
        assert_eq!(
            decoded
                .try_room_for(&items, &ItemStack::new("minecraft:stone", 1))
                .unwrap(),
            115
        );
    }

    #[test]
    fn custom_component_extra_entry_keys_match_vanilla_decode_and_reencode() {
        let items = test_catalog("extra_keys");
        let spec = BundleSpec::custom("runes:pouch_contents", 2);

        // Vanilla `ItemStack.MAP_CODEC` (`RecordCodecBuilder`) — and Sunrise's vanilla bundle path
        // (`item_data::template_from` / `template_tag`, `item_nbt::item_from_persistent_nbt` /
        // `item_to_nbt`) — accept unmentioned keys on an entry compound and drop them on re-encode.
        // An unmentioned legacy `"Count"` key is ignored as an extra key, so omitted `"count"`
        // defaults to `1`.
        let mut pouch = ItemStack::new("runes:pouch", 1);
        pouch.data_mut().set_other(
            "runes:pouch_contents",
            Some(Tag::List(vec![
                Tag::compound([
                    ("id", Tag::String("minecraft:stone".into())),
                    ("count", Tag::Int(3)),
                    ("tier", Tag::Int(2)),
                    ("custom_meta", Tag::String("ignored".into())),
                ]),
                Tag::compound([("id", Tag::String("stone".into())), ("Count", Tag::Byte(4))]),
            ])),
        );

        let contents = CustomBundleContents::try_of(&items, &pouch, spec)
            .unwrap()
            .unwrap();
        assert_eq!(
            contents
                .items()
                .iter()
                .map(|s| (&*s.item, s.count))
                .collect::<Vec<_>>(),
            vec![("minecraft:stone", 3), ("minecraft:stone", 1)]
        );
        // Weight = 4/64 out of capacity 2 = 128 stones -> 124 remaining.
        assert_eq!(
            contents
                .try_room_for(&items, &ItemStack::new("minecraft:stone", 1))
                .unwrap(),
            124
        );

        // Re-encoding via `store` drops the unmentioned extra keys, matching vanilla re-encode.
        contents.store(&items, &mut pouch);
        let stored = pouch
            .data()
            .and_then(|d| d.other("runes:pouch_contents"))
            .expect("stored list");
        let Tag::List(entries) = stored else {
            panic!("expected Tag::List after store");
        };
        assert_eq!(entries.len(), 2);
        assert!(entries[0].get("tier").is_none());
        assert!(entries[0].get("custom_meta").is_none());
        assert!(entries[1].get("Count").is_none());
        assert_eq!(entries[0].get("count"), Some(&Tag::Int(3)));
        assert_eq!(entries[1].get("count"), Some(&Tag::Int(1)));
    }

    #[test]
    fn custom_component_nested_malformed_payload_refuses_outer_weight_and_insert() {
        let items = test_catalog("nested_malformed");
        let spec = BundleSpec::custom("runes:pouch_contents", 2);

        // Build two malformed inner custom pouches: one non-list compound, one list with a bad element.
        let mut bad_compound_inner = ItemStack::new("runes:pouch", 1);
        bad_compound_inner.data_mut().set_other(
            "runes:pouch_contents",
            Some(Tag::compound([("tier", Tag::Int(2))])),
        );
        let mut bad_list_inner = ItemStack::new("runes:pouch", 1);
        bad_list_inner.data_mut().set_other(
            "runes:pouch_contents",
            Some(Tag::List(vec![
                Tag::Int(42),
                Tag::compound([
                    ("id", Tag::String("minecraft:stone".into())),
                    ("count", Tag::Int(1)),
                ]),
            ])),
        );

        for (label, malformed_inner) in [
            ("compound_inner", bad_compound_inner.clone()),
            ("bad_list_inner", bad_list_inner.clone()),
        ] {
            // 1. Direct nesting: valid outer pouch list whose entry is a malformed inner pouch.
            let mut outer = ItemStack::new("runes:pouch", 1);
            outer.data_mut().set_other(
                "runes:pouch_contents",
                Some(Tag::List(vec![crate::item_nbt::item_to_nbt(
                    &malformed_inner,
                    &items,
                )])),
            );
            let outer_original = outer.clone();

            assert_eq!(
                CustomBundleContents::try_of(&items, &outer, spec).err(),
                Some(CustomBundleError::UnsupportedFormat),
                "{label}: outer try_of must propagate nested error"
            );
            assert_eq!(
                CustomBundleContents::try_room_in_stack(
                    &items,
                    &outer,
                    spec,
                    &ItemStack::new("minecraft:stone", 1),
                ),
                Err(CustomBundleError::UnsupportedFormat),
                "{label}: outer try_room_in_stack must propagate nested error"
            );
            assert!(tooltip_layout_for(&items, &outer, spec, 0.0, 0.0, 96.0, 2).is_none());
            assert!(destroyed_component(&items, &mut outer, "runes:pouch_contents").is_empty());
            assert!(outer.same_item_same_components(&outer_original));
            CustomBundleContents::try_from_items(&items, spec, Vec::new())
                .unwrap()
                .store(&items, &mut outer);
            assert!(outer.same_item_same_components(&outer_original));

            // 2. Indirect nesting: malformed custom pouch inside a vanilla bundle inside outer pouch.
            let mut vanilla_wrapper = ItemStack::new("minecraft:bundle", 1);
            vanilla_wrapper.data_mut().bundle_contents = vec![malformed_inner.clone()];
            let mut outer_via_vanilla = ItemStack::new("runes:pouch", 1);
            outer_via_vanilla.data_mut().set_other(
                "runes:pouch_contents",
                Some(Tag::List(vec![crate::item_nbt::item_to_nbt(
                    &vanilla_wrapper,
                    &items,
                )])),
            );
            assert_eq!(
                CustomBundleContents::try_of(&items, &outer_via_vanilla, spec).err(),
                Some(CustomBundleError::UnsupportedFormat),
                "{label}: malformed custom pouch nested inside vanilla bundle must refuse outer"
            );

            // 3. Offered stack: valid empty outer pouch offered a malformed inner pouch (directly
            //    or wrapped inside a vanilla bundle) must refuse weight, room, and insert.
            let valid_outer = ItemStack::new("runes:pouch", 1);
            assert_eq!(
                CustomBundleContents::try_room_in_stack(
                    &items,
                    &valid_outer,
                    spec,
                    &malformed_inner
                ),
                Err(CustomBundleError::UnsupportedFormat),
                "{label}: offering malformed custom pouch must fail try_room_in_stack"
            );
            assert_eq!(
                CustomBundleContents::try_room_in_stack(
                    &items,
                    &valid_outer,
                    spec,
                    &vanilla_wrapper
                ),
                Err(CustomBundleError::UnsupportedFormat),
                "{label}: offering vanilla bundle holding malformed pouch must fail try_room_in_stack"
            );
            let mut contents = CustomBundleContents::try_of(&items, &valid_outer, spec)
                .unwrap()
                .unwrap();
            assert_eq!(
                contents.try_room_for(&items, &malformed_inner),
                Err(CustomBundleError::UnsupportedFormat)
            );
            assert_eq!(contents.room_for(&items, &malformed_inner), 0);
            let mut offered = malformed_inner.clone();
            assert_eq!(
                contents.try_insert_checked(&items, &mut offered, true),
                Err(CustomBundleError::UnsupportedFormat)
            );
            assert_eq!(
                contents.try_insert_checked(&items, &mut offered, false),
                Err(CustomBundleError::UnsupportedFormat)
            );
            assert_eq!(offered.count, 1);
            assert!(contents.items().is_empty());
        }
    }

    #[test]
    fn custom_component_metrics_cache_computes_once_per_value_and_invalidates_on_catalog_change() {
        let items = test_catalog("metrics_cache");
        let spec4 = BundleSpec::custom("runes:pouch_contents", 4);
        let spec2 = BundleSpec::custom("runes:pouch_contents", 2);

        // Build a 40-deep nested pouch holding 16 stones inside the innermost pouch.
        let mut current = ItemStack::new("runes:pouch", 1);
        let mut contents = CustomBundleContents::of(&items, &current, spec4).unwrap();
        let mut stones = ItemStack::new("minecraft:stone", 16);
        assert_eq!(contents.try_insert(&items, &mut stones, true), 16);
        contents.store(&items, &mut current);
        for _ in 0..40 {
            let mut outer = ItemStack::new("runes:pouch", 1);
            let mut outer_contents = CustomBundleContents::of(&items, &outer, spec4).unwrap();
            let mut inner = current;
            assert_eq!(outer_contents.try_insert(&items, &mut inner, true), 1);
            outer_contents.store(&items, &mut outer);
            current = outer;
        }

        let mut cache = BundleMetricsCache::new();

        // Missing component and empty pouch fast paths do zero weight computations.
        assert_eq!(
            cache
                .metrics_for(&items, &ItemStack::new("minecraft:stone", 1), spec4)
                .unwrap(),
            None
        );
        let empty = ItemStack::new("runes:pouch", 1);
        let empty_summary = cache.metrics_for(&items, &empty, spec4).unwrap().unwrap();
        assert_eq!(empty_summary.weight_fraction, 0.0);
        assert!(!empty_summary.is_full);
        assert_eq!(empty_summary.bar, None);
        assert_eq!(empty_summary.stack_count, 0);
        assert_eq!(cache.computations(), 0);

        // First query decodes and walks the 40-deep nested tree once.
        let expected_m4 = CustomBundleContents::try_of(&items, &current, spec4)
            .unwrap()
            .unwrap();
        let expected_m2 = CustomBundleContents::try_of(&items, &current, spec2)
            .unwrap()
            .unwrap();
        let m4 = cache.metrics_for(&items, &current, spec4).unwrap().unwrap();
        assert_eq!(cache.computations(), 1);
        assert!((m4.weight_fraction - expected_m4.weight_fraction()).abs() < 1e-6);
        assert_eq!(m4.is_full, expected_m4.is_full());
        assert_eq!(m4.bar, expected_m4.bar());
        assert_eq!(m4.stack_count, 1);

        // Repeated draws across multipliers, slots_for, and room_for reuse the single cached entry.
        for _ in 0..20 {
            let repeat_m4 = cache.metrics_for(&items, &current, spec4).unwrap().unwrap();
            assert_eq!(repeat_m4, m4);
            let repeat_m2 = cache.metrics_for(&items, &current, spec2).unwrap().unwrap();
            assert!((repeat_m2.weight_fraction - expected_m2.weight_fraction()).abs() < 1e-6);
            assert_eq!(repeat_m2.is_full, expected_m2.is_full());
            assert_eq!(repeat_m2.bar, expected_m2.bar());
            let slots = cache.slots_for(&items, &current, spec4).unwrap().unwrap();
            assert_eq!(slots.len(), 1);
            let room = cache
                .room_for(
                    &items,
                    &current,
                    spec4,
                    &ItemStack::new("minecraft:stone", 1),
                )
                .unwrap()
                .unwrap();
            assert_eq!(
                room,
                expected_m4.room_for(&items, &ItemStack::new("minecraft:stone", 1))
            );
        }
        assert_eq!(
            cache.computations(),
            1,
            "repeated metrics/slots/room queries on unchanged contents must compute weight only once"
        );

        // Mutating the pouch produces a new component value and computes once more.
        let mut mutated = current.clone();
        let mut mc = CustomBundleContents::try_of(&items, &mutated, spec4)
            .unwrap()
            .unwrap();
        assert_eq!(
            mc.try_insert(&items, &mut ItemStack::new("minecraft:stone", 8), true),
            8
        );
        mc.store(&items, &mut mutated);
        let m4_mutated = cache.metrics_for(&items, &mutated, spec4).unwrap().unwrap();
        assert_eq!(cache.computations(), 2);
        assert!(m4_mutated.weight_fraction > m4.weight_fraction);
        let _ = cache.metrics_for(&items, &mutated, spec4).unwrap().unwrap();
        assert_eq!(cache.computations(), 2);

        // Malformed payloads are also cached as UnsupportedFormat without re-parsing on every draw.
        let mut malformed = ItemStack::new("runes:pouch", 1);
        malformed
            .data_mut()
            .set_other("runes:pouch_contents", Some(Tag::List(vec![Tag::Int(42)])));
        assert_eq!(
            cache.metrics_for(&items, &malformed, spec4),
            Err(CustomBundleError::UnsupportedFormat)
        );
        assert_eq!(cache.computations(), 3);
        assert_eq!(
            cache.metrics_for(&items, &malformed, spec4),
            Err(CustomBundleError::UnsupportedFormat)
        );
        assert_eq!(cache.computations(), 3);

        // Cloning/reloading the Items catalog invalidates the cache and recomputes under new rules.
        let mut items_v2 = items.clone();
        items_v2.items.get_mut("minecraft:stone").unwrap().max_stack = 16;
        let m4_v2 = cache
            .metrics_for(&items_v2, &current, spec4)
            .unwrap()
            .unwrap();
        assert_eq!(cache.computations(), 4);
        assert!(
            m4_v2.weight_fraction > m4.weight_fraction,
            "reloaded catalog with smaller max_stack must recompute heavier weight"
        );
        let _ = cache
            .metrics_for(&items_v2, &current, spec4)
            .unwrap()
            .unwrap();
        assert_eq!(cache.computations(), 4);

        // Removing a stored item id from a reloaded catalog invalidates the cache and refuses the
        // payload with UnsupportedFormat (matching registry-backed `Item.CODEC`).
        let mut items_v3 = items.clone();
        items_v3.items.remove("minecraft:stone");
        assert_eq!(
            cache.metrics_for(&items_v3, &current, spec4),
            Err(CustomBundleError::UnsupportedFormat)
        );
        assert_eq!(cache.computations(), 5);
    }
}
