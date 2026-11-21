#!/usr/bin/env python3
"""Thread the existing special-type lookup through all item model decoding paths.

Run on the debt 63 step 1 baseline. This commit preserves the original slot lookup;
the follow-up replaces it at the boundary with the opaque registered model loader.
"""

from pathlib import Path


path = Path("crates/sunrise-assets/src/item_model.rs")
text = path.read_text()


def replace_once(text: str, old: str, new: str) -> str:
    if text.count(old) != 1:
        raise SystemExit(f"expected one match for {old!r}")
    return text.replace(old, new)


# The same call also appears in SwitchCase, where the caller's lookup must be forwarded.
text = replace_once(
    text,
    '''    pub fn decode(value: &Value) -> Result<Self> {
        let m = object(value)?;
        Ok(Self {
            model: ItemModelNode::decode(required(m, "model")?)?,''',
    '''    pub fn decode(value: &Value) -> Result<Self> {
        let m = object(value)?;
        Ok(Self {
            model: ItemModelNode::decode_with(required(m, "model")?, registered_special_type)?,''',
)
text = replace_once(
    text,
    '''impl ItemModelNode {
    pub fn decode(v: &Value) -> Result<Self> {
        let m = object(v)?;''',
    '''type SpecialTypeLookup = fn(&Value) -> Result<Option<SpecialModelSlot>>;

impl ItemModelNode {
    pub fn decode(v: &Value) -> Result<Self> {
        Self::decode_with(v, registered_special_type)
    }

    fn decode_with(v: &Value, lookup: SpecialTypeLookup) -> Result<Self> {
        let decode = |node| Self::decode_with(node, lookup);
        let m = object(v)?;''',
)
start = text.index("    fn decode_with(v:")
end = text.index("    /// Every model resource id", start)
body = text[start:end]
body = body.replace("Self::decode(", "decode(").replace(".map(Self::decode)", ".map(decode)")
body = replace_once(body, "SwitchCase::decode(&property, c)", "SwitchCase::decode(&property, c, lookup)")
body = replace_once(body, "match registered_special_type(model)?", "match lookup(model)?")
text = text[:start] + body + text[end:]
text = replace_once(
    text,
    "    fn decode(property: &SelectProperty, v: &Value) -> Result<Self> {",
    "    fn decode(property: &SelectProperty, v: &Value, lookup: SpecialTypeLookup) -> Result<Self> {",
)
text = replace_once(
    text,
    '            model: ItemModelNode::decode(required(m, "model")?)?,',
    '            model: ItemModelNode::decode_with(required(m, "model")?, lookup)?,',
)
path.write_text(text)
