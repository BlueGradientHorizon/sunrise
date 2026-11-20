//! Immutable component codec values shared by item templates and protocol codecs.
use sunrise_core::nbt::{JavaString, Tag, with_tree_stack};
pub mod source;
pub mod types;
pub use source::from_component_tag;

/// `ComponentSerialization.CODEC` collapses an unstyled literal with no siblings.
/// Borrow the compact form so source and stream traversal share it.
pub fn compact_text_tag(mut value: &Tag) -> &Tag {
    loop {
        match value {
            Tag::List(values) if values.len() == 1 => value = &values[0],
            Tag::Compound(fields) if fields.len() == 1 => match fields.iter().next() {
                Some((key, text @ Tag::String(_))) if key.0.as_slice() == [116, 101, 120, 116] => {
                    return text;
                }
                _ => return value,
            },
            _ => return value,
        }
    }
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Template {
    pub item: String,
    pub count: i32,
    pub patch: Patch,
}
impl Template {
    pub fn new(item: String, count: i32, patch: Patch) -> Self {
        Self { item, count, patch }
    }
}
/// The entries that decide a patch: when a component is named twice the last entry wins,
/// without allocating or changing the retained patch.
pub fn effective_patch(
    patch: &[(String, Option<Value>)],
) -> impl Iterator<Item = &(String, Option<Value>)> {
    patch
        .iter()
        .enumerate()
        .filter(|(index, (name, _))| !patch[index + 1..].iter().any(|(later, _)| later == name))
        .map(|(_, entry)| entry)
}
impl PartialEq for Template {
    fn eq(&self, other: &Self) -> bool {
        self.item == other.item
            && self.count == other.count
            && effective_patch(&self.patch).all(|(key, value)| {
                other
                    .patch
                    .iter()
                    .rev()
                    .find(|(candidate, _)| candidate == key)
                    .is_some_and(|(_, candidate)| candidate == value)
            })
            && other
                .patch
                .iter()
                .all(|(key, _)| self.patch.iter().any(|(candidate, _)| candidate == key))
    }
}
/// A template's own patch, not its prototype-expanded effective components.
pub type Patch = Vec<(String, Option<Value>)>;

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Value {
    Empty,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(u32),
    Double(u64),
    Bool(bool),
    String(Vec<u16>),
    ByteArray(Vec<u8>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
    List(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Set(Vec<Value>),
    Template(Box<Template>),
    Container(Vec<Option<Template>>),
    /// A real stream value for which the persistent codec refuses encoding.
    StreamOnly(Box<Value>),
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        use Value::*;
        match (self, other) {
            (Empty, Empty) => true,
            (Byte(a), Byte(b)) => a == b,
            (Short(a), Short(b)) => a == b,
            (Int(a), Int(b)) => a == b,
            (Long(a), Long(b)) => a == b,
            (Float(a), Float(b)) => {
                a == b || (f32::from_bits(*a).is_nan() && f32::from_bits(*b).is_nan())
            }
            (Double(a), Double(b)) => {
                a == b || (f64::from_bits(*a).is_nan() && f64::from_bits(*b).is_nan())
            }
            (Bool(a), Bool(b)) => a == b,
            (String(a), String(b)) => a == b,
            (ByteArray(a), ByteArray(b)) => a == b,
            (IntArray(a), IntArray(b)) => a == b,
            (LongArray(a), LongArray(b)) => a == b,
            (List(a), List(b)) => a == b,
            (Map(a), Map(b)) => {
                effective_map(a).all(|(key, value)| {
                    b.iter()
                        .rev()
                        .find(|entry| &entry.0 == key)
                        .is_some_and(|entry| &entry.1 == value)
                }) && b
                    .iter()
                    .all(|(key, _)| a.iter().any(|entry| &entry.0 == key))
            }
            (Set(a), Set(b)) => a.iter().all(|v| b.contains(v)) && b.iter().all(|v| a.contains(v)),
            (Template(a), Template(b)) => a == b,
            (Container(a), Container(b)) => a == b,
            (StreamOnly(a), StreamOnly(b)) => a == b,
            _ => false,
        }
    }
}
/// Like an NBT compound, a canonical map retains the last value for a key.
fn effective_map(entries: &[(Value, Value)]) -> impl Iterator<Item = &(Value, Value)> {
    entries.iter().enumerate().filter_map(|(index, entry)| {
        (!entries[index + 1..].iter().any(|later| later.0 == entry.0)).then_some(entry)
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Projection {
    /// Opaque values may contain templates; each outermost template validates its own saved codec.
    Tree,
    /// The owning component or template validates the completed projection once.
    Codec,
    Presentation,
}

impl Value {
    pub fn string(value: &str) -> Self {
        Self::String(value.encode_utf16().collect())
    }
    /// Convert an already materialized tree; binary NBT read limits do not apply here.
    pub fn from_nbt(tag: &Tag, bool_keys: &[&str]) -> Self {
        Self::convert_nbt(tag, bool_keys)
    }
    fn convert_nbt(tag: &Tag, bool_keys: &[&str]) -> Self {
        match tag {
            Tag::End => Self::Empty,
            Tag::Byte(v) => Self::Byte(*v),
            Tag::Short(v) => Self::Short(*v),
            Tag::Int(v) => Self::Int(*v),
            Tag::Long(v) => Self::Long(*v),
            Tag::Float(v) => Self::Float(*v),
            Tag::Double(v) => Self::Double(*v),
            Tag::String(v) => Self::String(v.0.clone()),
            Tag::ByteArray(v) => Self::ByteArray(v.clone()),
            Tag::IntArray(v) => Self::IntArray(v.clone()),
            Tag::LongArray(v) => Self::LongArray(v.clone()),
            Tag::List(v) => with_tree_stack(|| {
                Self::List(v.iter().map(|v| Self::convert_nbt(v, bool_keys)).collect())
            }),
            Tag::Compound(v) => Self::convert_compound(v, bool_keys),
        }
    }
    fn convert_compound(v: &sunrise_core::nbt::Compound, bool_keys: &[&str]) -> Self {
        with_tree_stack(|| {
            Self::Map(
                v.iter()
                    .map(|(k, v)| {
                        let value = match v {
                            Tag::Byte(v)
                                if bool_keys
                                    .iter()
                                    .any(|n| k.0.iter().copied().eq(n.encode_utf16())) =>
                            {
                                Self::Bool(*v != 0)
                            }
                            _ => Self::convert_nbt(v, bool_keys),
                        };
                        (Self::String(k.0.clone()), value)
                    })
                    .collect(),
            )
        })
    }

    /// Release an owned materialized value without recursive container drops.
    pub fn discard(self) {
        let mut pending = vec![self];
        while let Some(value) = pending.pop() {
            match value {
                Self::List(values) | Self::Set(values) => pending.extend(values),
                Self::Map(entries) => {
                    pending.extend(entries.into_iter().flat_map(|(key, value)| [key, value]))
                }
                Self::Template(template) => {
                    pending.extend(template.patch.into_iter().filter_map(|(_, value)| value))
                }
                Self::Container(slots) => {
                    pending.extend(slots.into_iter().flatten().flat_map(|template| {
                        template.patch.into_iter().filter_map(|(_, value)| value)
                    }))
                }
                Self::StreamOnly(value) => pending.push(*value),
                _ => {}
            }
        }
    }

    /// A read-only encoding error leaves the value available to the caller.
    pub fn to_tag(&self) -> Result<Tag, String> {
        self.tag(Projection::Tree)
    }
    /// Project to the named saved codec, refusing stream-only values and tags it cannot decode.
    pub fn to_component_tag(&self, name: &str) -> Result<Tag, String> {
        self.to_component_tag_with(name, |_| false)
    }
    /// As above, with the owning registry's installed native types.
    pub fn to_component_tag_with(
        &self,
        name: &str,
        is_native: fn(&str) -> bool,
    ) -> Result<Tag, String> {
        let tag = self
            .tag(Projection::Codec)
            .map_err(|error| format!("{name}: {error}"))?;
        match source::from_component_tag_with(name, &tag, is_native) {
            Ok(value) => {
                value.discard();
                Ok(tag)
            }
            Err(error) => {
                sunrise_core::nbt::discard_tag(tag);
                Err(error)
            }
        }
    }
    /// Presentation may inspect inline stream holders without claiming they persist.
    pub fn presentation_tag(&self) -> Result<Tag, String> {
        self.tag(Projection::Presentation)
    }
    fn tag(&self, projection: Projection) -> Result<Tag, String> {
        with_tree_stack(|| self.tag_with_stack(projection))
    }
    fn tag_with_stack(&self, projection: Projection) -> Result<Tag, String> {
        Ok(match self {
            Self::Empty => Tag::End,
            Self::Byte(v) => Tag::Byte(*v),
            Self::Short(v) => Tag::Short(*v),
            Self::Int(v) => Tag::Int(*v),
            Self::Long(v) => Tag::Long(*v),
            Self::Float(v) => Tag::Float(*v),
            Self::Double(v) => Tag::Double(*v),
            Self::Bool(v) => Tag::Byte(i8::from(*v)),
            Self::String(v) => Tag::String(JavaString(v.clone())),
            Self::ByteArray(v) => Tag::ByteArray(v.clone()),
            Self::IntArray(v) => Tag::IntArray(v.clone()),
            Self::LongArray(v) => Tag::LongArray(v.clone()),
            Self::List(v) | Self::Set(v) => Tag::List(
                v.iter()
                    .map(|v| v.tag(projection))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Map(v) => Tag::Compound(
                // The compound's string keys identify the effective entries; don't
                // project overwritten values (they need not have a saved form).
                v.iter()
                    .map(|(key, value)| match key {
                        Self::String(key) => Ok((key, value)),
                        _ => Err("component map key is not a string".to_owned()),
                    })
                    .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?
                    .into_iter()
                    .map(|(k, v)| Ok((JavaString(k.clone()), v.tag(projection)?)))
                    .collect::<Result<_, String>>()?,
            ),
            Self::Template(v) => v.tag(projection)?,
            Self::Container(slots) => Tag::List(
                slots
                    .iter()
                    .enumerate()
                    .filter_map(|(slot, item)| {
                        item.as_ref().map(|item| {
                            Ok(Tag::compound([
                                ("slot", Tag::Int(slot as i32)),
                                ("item", item.tag(projection)?),
                            ]))
                        })
                    })
                    .collect::<Result<_, String>>()?,
            ),
            Self::StreamOnly(v) if projection == Projection::Presentation => v.tag(projection)?,
            Self::StreamOnly(_) => return Err("inline holder has no persistent encoding".into()),
        })
    }
}
impl Template {
    /// Project the own patch to its saved form, omitting transients without changing the template.
    pub fn to_tag(&self) -> Result<Tag, String> {
        self.tag(Projection::Tree)
    }
    fn tag(&self, projection: Projection) -> Result<Tag, String> {
        let children = if projection == Projection::Presentation {
            projection
        } else {
            Projection::Codec
        };
        let mut fields = vec![("id".into(), Tag::String(self.item.as_str().into()))];
        if self.count != 1 {
            fields.push(("count".into(), Tag::Int(self.count)));
        }
        let mut patch = Vec::with_capacity(self.patch.len());
        for (name, value) in effective_patch(&self.patch) {
            if projection != Projection::Presentation && !is_persistent(name) {
                continue;
            }
            patch.push(match value {
                Some(value) => (
                    name.as_str().into(),
                    value
                        .tag(children)
                        .map_err(|error| format!("{name}: {error}"))?,
                ),
                None => (
                    format!("!{name}").as_str().into(),
                    Tag::Compound(Default::default()),
                ),
            });
        }
        if !patch.is_empty() {
            fields.push((
                "components".into(),
                Tag::Compound(patch.into_iter().collect()),
            ));
        }
        let tag = Tag::Compound(fields.into_iter().collect());
        if projection == Projection::Tree {
            match source::template_from_tag(&tag) {
                Ok(template) => Value::Template(Box::new(template)).discard(),
                Err(error) => {
                    sunrise_core::nbt::discard_tag(tag);
                    return Err(error);
                }
            }
        }
        Ok(tag)
    }
}
/// The pinned component types that intentionally have no persistent codec.
pub fn is_persistent(name: &str) -> bool {
    if name.contains(':') {
        !crate::item_defaults::is_transient_component(name)
    } else {
        !crate::item_defaults::is_transient_component(&format!("minecraft:{name}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn java_float_and_collection_identity() {
        assert_eq!(Value::Float(0x7fc00001), Value::Float(0x7fc00002));
        assert_ne!(Value::Float(0), Value::Float(0x80000000));
        assert_eq!(
            Value::Set(vec![Value::Int(1), Value::Int(2)]),
            Value::Set(vec![Value::Int(2), Value::Int(1)])
        );
        assert_ne!(
            Value::List(vec![Value::Int(1), Value::Int(2)]),
            Value::List(vec![Value::Int(2), Value::Int(1)])
        );
    }
    #[test]
    fn map_equality_and_projection_use_the_last_entry_for_each_key() {
        let key = Value::string("a");
        let duplicate = Value::Map(vec![
            (key.clone(), Value::Int(0)),
            (key.clone(), Value::Int(1)),
        ]);
        let right = Value::Map(vec![(key.clone(), Value::Int(1))]);
        let other = Value::Map(vec![
            (key, Value::Int(1)),
            (Value::string("b"), Value::Int(2)),
        ]);
        assert_eq!(duplicate, right);
        assert_eq!(right, duplicate);
        assert_ne!(duplicate, other);
        assert_ne!(other, duplicate);
        assert_eq!(
            duplicate.to_tag().unwrap(),
            Tag::compound([("a", Tag::Int(1))])
        );
    }
    #[test]
    fn source_failure_does_not_destroy_stream_value() {
        let value = Value::StreamOnly(Box::new(Value::Int(3)));
        assert!(value.to_tag().is_err());
        assert_eq!(value.presentation_tag().unwrap(), Tag::Int(3));
    }
}
