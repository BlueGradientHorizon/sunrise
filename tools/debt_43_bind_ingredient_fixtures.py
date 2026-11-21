#!/usr/bin/env python3
"""Rewrite the repeated ingredient hook signatures and marker fixture for debt 43."""
from pathlib import Path
import re

CLIENT = Path("crates/sunrise-client/src")
for relative, expected in (
    ("crafting/custom_data_tests.rs", 2),
    ("crafting/ingredient/matching_tests.rs", 1),
    ("game/network/lan_crafting.rs", 1),
    ("lan/recipes.rs", 2),
):
    path = CLIENT / relative
    source, count = re.subn(
        r"(?m)^( +)(event|_): &mut mod_api::IngredientEvent<'_>,\n",
        r"\1kind: &str,\n\1data: &mod_api::Compound,\n",
        path.read_text(),
    )
    assert count == expected, (relative, count)
    # All of these hook bodies are adapted individually after this migration.
    source = re.sub(
        r"(?m)( +data: &mod_api::Compound,\n +)\) \{",
        r"\1) -> Option<mod_api::IngredientDefinition> {",
        source,
    )
    path.write_text(source.replace("event.kind", "kind").replace("event.data", "data"))

path = CLIENT / "crafting/ingredient/builtin_kinds_tests/families/rest.rs"
source = path.read_text()
old = '    custom.custom = Some(("fixture:custom".into(), Default::default()));'
assert source.count(old) == 1
path.write_text(source.replace(old, '''    custom.custom = Some(
        mod_api::__private::bind_ingredient_predicate(
            "fixture",
            0,
            mod_api::IngredientDefinition::new(vec![], |_, _| false),
        )
        .1,
    );'''))
