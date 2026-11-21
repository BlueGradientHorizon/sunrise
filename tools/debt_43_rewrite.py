#!/usr/bin/env python3
"""Convert the shared ingredient walk to return caller-owned custom answers."""

from pathlib import Path
from textwrap import dedent, indent

ROOT = Path(__file__).resolve().parents[1]
CLIENT = "crates/sunrise-client/src/crafting/"
VOCAB = "crates/sunrise-mod-vocab/src/"
API = "crates/sunrise-mod-api/src/"


def block(source, spaces=0):
    return indent(dedent(source).strip("\n"), " " * spaces)


def main():
    pending = {}

    def replace(path, old, new, count=1):
        if path not in pending:
            pending[path] = (ROOT / path).read_text(encoding="utf-8")
        source = pending[path]
        found = source.count(old)
        if found != count:
            raise SystemExit(f"{path}: expected {count} matches, found {found}: {old!r}")
        pending[path] = source.replace(old, new)

    parser = VOCAB + "__private/ingredient_parser.rs"
    replace(parser, block('''
        /// Where `Ingredient::resolve` leaves the hook's answer inside the occurrence
        /// it resolved. Written and read only here; the loads that own the sources
        /// never send or save it.
        const RESOLVED_ITEMS: &str = "sunrise:resolved_items";
    ''') + "\n", "")
    replace(parser, block('''
        /// Takes the answer `resolve` left out of `data`.
        // hidden: the game-owned ingredient adapter reads the answer a mod's `resolve` left, not a mod-facing call.
        #[doc(hidden)]
        pub fn resolved_items(data: &mut sunrise_core::nbt::Compound) -> Option<Vec<String>> {
            let answer = data.remove(&RESOLVED_ITEMS.into())?;
            let sunrise_core::nbt::Tag::List(answer) = answer else {
                return None;
            };
            answer
                .iter()
                .map(|i| match i {
                    sunrise_core::nbt::Tag::String(s) => s.to_string().ok(),
                    _ => None,
                })
                .collect()
        }
    ''') + "\n", "")
    replace(parser, block('''
        /// shape sees its ids like any child's: the item ingredient gives the list
        /// its hook left at load, a fluid one none, which fails the file. Existing
        /// empty tags and custom answers retain their predicate but cannot be placed.
    '''), block('''
        /// shape sees its ids like any child's. A missing answer fails the parse.
        /// Existing empty tags and custom answers retain their predicate but cannot
        /// be placed.
    '''))
    replace(parser, "pub struct WalkedIngredient {", "pub struct WalkedIngredient<A> {")
    replace(parser, block('''
        /// A kind outside the standard shapes: the kind and its NBT compound, as the
        /// file wrote it, without the wrapper's answer.
        pub custom: Option<(String, sunrise_core::nbt::Compound)>,
    ''', 4), block('''
        /// The wrapper's payload for a kind outside the standard shapes.
        pub custom: Option<A>,
    ''', 4))
    replace(parser, block('''
        /// The wrapper's answer for a kind outside the standard shapes: the ids it
        /// matches, taken out of its metadata, or none to fail the file.
    '''), block('''
        /// Answers the original JSON and kind with placement ids and a custom
        /// payload, or none to fail the file.
    '''))
    replace(parser, "pub type CustomIds<'a> = &'a dyn Fn(&mut sunrise_core::nbt::Compound) -> Option<Vec<String>>;",
            "pub type CustomIds<'a, A> = &'a dyn Fn(&Value, &str) -> Option<(Vec<String>, A)>;")
    replace(parser, "impl WalkedIngredient {", "impl<A> WalkedIngredient<A> {")
    replace(parser, "custom: CustomIds<'_>,", "custom: CustomIds<'_, A>,", count=3)
    replace(parser, "Self::walk_object(o, tags, block_tags, custom, convert, resolve)",
            "Self::walk_object(v, o, tags, block_tags, custom, convert, resolve)")
    replace(parser, "    fn walk_object(\n", "    fn walk_object(\n        v: &Value,\n")
    replace(parser, block('''
        nested = Some((Shape::Intersection, subs.clone()));
        subs.first()?
            .order
            .iter()
            .filter(|i| subs[1..].iter().all(|s| s.set.contains(*i)))
            .cloned()
            .collect()
    ''', 16), block('''
        let kept = subs
            .first()?
            .order
            .iter()
            .filter(|i| subs[1..].iter().all(|s| s.set.contains(*i)))
            .cloned()
            .collect();
        nested = Some((Shape::Intersection, subs));
        kept
    ''', 16))
    replace(parser, block('''
        nested = Some((Shape::Difference, vec![base.clone(), gone.clone()]));
        base.order
            .iter()
            .filter(|i| !gone.is_simple() || !gone.set.contains(*i))
            .cloned()
            .collect()
    ''', 16), block('''
        let kept = base
            .order
            .iter()
            .filter(|i| !gone.is_simple() || !gone.set.contains(*i))
            .cloned()
            .collect();
        nested = Some((Shape::Difference, vec![base, gone]));
        kept
    ''', 16))
    replace(parser, block('''
        let mut shown = base.clone();
        shown.rule = None;
        shown.custom = None;
        shown.children = Some((Shape::CustomDisplay, vec![base]));
        shown.display = Some(convert(o.get("display")?.as_object()?)?);
        return Some(shown);
    ''', 16), block('''
        return Some(Self {
            set: base.set.clone(),
            order: base.order.clone(),
            children: Some((Shape::CustomDisplay, vec![base])),
            custom: None,
            rule: None,
            display: Some(convert(o.get("display")?.as_object()?)?),
        });
    ''', 16))
    replace(parser, block('''
        // A kind outside these shapes: the wrapper answers it — the
        // item ingredient asks its hook, a fluid one fails the file.
        k if is_custom(k) => {
            let mut data = convert(o)?;
            let list = custom(&mut data)?;
    ''', 12), block('''
        // The wrapper defines both the candidate ids and the custom payload.
        k if is_custom(k) => {
            let (list, answer) = custom(v, k)?;
    ''', 12))
    replace(parser, "custom: Some((k.to_owned(), data)),", "custom: Some(answer),")

    client = CLIENT + "ingredient.rs"
    replace(client, "use mod_api::__private::{WalkedIngredient, dedup_keep_order, resolved_items};",
            "use mod_api::__private::{WalkedIngredient, dedup_keep_order};")
    replace(client, "            &resolved_items,", block('''
        &|v, kind| {
            use sunrise_core::nbt::Tag;

            let mut data = components::json_patch(v.as_object()?)?;
            let Tag::List(answer) = data.remove(&RESOLVED_ITEMS.into())? else {
                return None;
            };
            let list = answer
                .iter()
                .map(|i| match i {
                    Tag::String(s) => s.to_string().ok(),
                    _ => None,
                })
                .collect::<Option<_>>()?;
            Some((list, (kind.to_owned(), data)))
        },
    ''', 12))
    replace(client, "    fn of_walked(walked: WalkedIngredient, items: &Items) -> Option<Self> {",
            block('''
        fn of_walked(
            walked: WalkedIngredient<(String, sunrise_core::nbt::Compound)>,
            items: &Items,
        ) -> Option<Self> {
    ''', 4))

    for path in ("ingredient/components.rs", "ingredient/builtin_kinds_tests/families/member_4124.rs"):
        replace(CLIENT + path, "WalkedIngredient::walk(", "WalkedIngredient::<()>::walk(")
        replace(CLIENT + path, "&resolved_items,", "&|_, _| None,")

    replace(VOCAB + "__private/ingredient_parser_tests.rs", "Option<WalkedIngredient>",
            "Option<WalkedIngredient<()>>")
    replace(VOCAB + "__private/ingredient_parser_tests.rs", "&|_| None, &|_| None, &|_| None)",
            "&|_| None, &|_, _| None, &|_| None)")
    replace(VOCAB + "exports/crafting.rs", block('''
        pub use crate::__private::ingredient_parser::{
            WalkedIngredient, dedup_keep_order, resolved_items,
        };
    ''', 4), "    pub use crate::__private::ingredient_parser::{WalkedIngredient, dedup_keep_order};")
    fluid = VOCAB + "block/fluid_capability.rs"
    replace(fluid, block('''
        let source =
            WalkedIngredient::walk_resolved(v, &tags, &|_| None, &|_| None, &|_| None, &|id| {
    ''', 8), block('''
        let source = WalkedIngredient::<()>::walk_resolved(
            v,
            &tags,
            &|_| None,
            &|_, _| None,
            &|_| None,
            &|id| {
    ''', 8))
    replace(fluid, "            });\n        fn nonempty_tree(source: &WalkedIngredient)",
            "            },\n        );\n        fn nonempty_tree(source: &WalkedIngredient<()>)")

    fixture = API + "parts/registration_fluid_capability_tests.rs"
    replace(fixture, "// Supply the resolved ids fixture, not a second JSON/NBT converter.",
            "// Supply the resolved ids directly; this fixture only checks enumeration.")
    replace(fixture, block('''
        &crate::__private::ingredient_parser::resolved_items,
        &|data| {
            let ids = data.get("sunrise:resolved_items")?.as_array()?;
            let ids = ids
                .iter()
                .map(|v| sunrise_core::nbt::Tag::String(v.as_str().unwrap().into()))
                .collect();
            Some(sunrise_core::nbt::Compound::from([(
                "sunrise:resolved_items".into(),
                sunrise_core::nbt::Tag::List(ids),
            )]))
        },
    ''', 16), block('''
        &|v, _| {
            let ids = v.get("sunrise:resolved_items")?.as_array()?;
            let ids = ids
                .iter()
                .map(|v| Some(v.as_str()?.to_owned()))
                .collect::<Option<_>>()?;
            Some((ids, ()))
        },
        &|_| None,
    ''', 16))

    for path, source in pending.items():
        (ROOT / path).write_text(source, encoding="utf-8")


if __name__ == "__main__":
    main()
