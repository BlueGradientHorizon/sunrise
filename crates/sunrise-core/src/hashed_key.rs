//! A key with its Java hash, so lookups and copies need not hash it again.
//!
//! For strings this is Java `String.hashCode`, computed on input or at compile time for a literal.
//! An identifier already stores the Java hash that combines its namespace and path. Only this crate
//! can pair a key with a raw hash without computing it or taking it from a known key.

use crate::{
    identifier::Identifier,
    java_string::{java_string_hash, java_string_hash_str, java_string_hash_units_const},
    nbt::JavaString,
};

/// A key with its Java hash, for a map keyed by `JavaString`, `String` or `Identifier`.
#[derive(Clone, Copy, Debug)]
pub struct HashedKey<S> {
    hash: i32,
    text: S,
}

/// A borrowed UTF-16 key: what a `Compound` is looked up by.
pub type UnitsKey<'a> = HashedKey<&'a [u16]>;
/// A borrowed `&str` key, for a map keyed by `String`.
pub type StrKey<'a> = HashedKey<&'a str>;

impl<S> HashedKey<S> {
    /// `hash` must be the Java hash of `text`.
    pub(crate) const fn from_parts(hash: i32, text: S) -> Self {
        Self { hash, text }
    }
    pub const fn hash(&self) -> i32 {
        self.hash
    }
    pub const fn text(&self) -> &S {
        &self.text
    }
    pub fn into_text(self) -> S {
        self.text
    }
    /// The same key by reference: the hash is copied, nothing is rehashed.
    pub const fn borrowed(&self) -> HashedKey<&S> {
        HashedKey {
            hash: self.hash,
            text: &self.text,
        }
    }
}

/// What a compound is looked up by: a key with its hash. Strings are not keys; make the key where the string
/// enters, or with `nbt_key!` for a literal.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a hashed key",
    label = "a compound is looked up by a `HashedKey`, not by a string",
    note = "use nbt_key!(\"x\") for a literal key, or HashedKey::new(..) where the string enters"
)]
pub trait LookupKey {
    fn units_key(&self) -> UnitsKey<'_>;
}
impl LookupKey for UnitsKey<'_> {
    fn units_key(&self) -> UnitsKey<'_> {
        *self
    }
}
impl LookupKey for HashedKey<JavaString> {
    fn units_key(&self) -> UnitsKey<'_> {
        HashedKey::from_parts(self.hash, self.text.units())
    }
}

/// What a compound is inserted by: anything that becomes an owned key without hashing.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a hashed key",
    label = "a compound is inserted by a `HashedKey`, not by a string",
    note = "use nbt_key!(\"x\") for a literal key, or HashedKey::new(..) where the string enters"
)]
pub trait InsertKey {
    fn into_key(self) -> HashedKey<JavaString>;
}
impl InsertKey for HashedKey<JavaString> {
    fn into_key(self) -> HashedKey<JavaString> {
        self
    }
}
impl InsertKey for &HashedKey<JavaString> {
    fn into_key(self) -> HashedKey<JavaString> {
        self.clone()
    }
}
impl InsertKey for &UnitsKey<'_> {
    fn into_key(self) -> HashedKey<JavaString> {
        self.into()
    }
}
impl InsertKey for HashedKey<&JavaString> {
    fn into_key(self) -> HashedKey<JavaString> {
        self.into()
    }
}

impl HashedKey<JavaString> {
    /// The key of `text`, encoded to UTF-16 once. Not an `Into`: hashing is always asked for by name.
    pub fn from_text(text: &str) -> Self {
        let (text, hash) = JavaString::with_hash(text);
        Self::from_parts(hash, text)
    }
    pub fn new(units: Vec<u16>) -> Self {
        Self {
            hash: java_string_hash(&units),
            text: JavaString::from(units),
        }
    }

    /// An owned key backed by static units; both copying it and computing its hash in a constant are free.
    #[doc(hidden)]
    pub const fn from_static(units: &'static [u16]) -> Self {
        Self::from_parts(
            java_string_hash_units_const(units),
            JavaString::from_static(units),
        )
    }
}
/// Copies the units and keeps the hash: nothing is rehashed.
impl From<&UnitsKey<'_>> for HashedKey<JavaString> {
    fn from(key: &UnitsKey<'_>) -> Self {
        Self::from_parts(key.hash(), key.text().iter().copied().collect())
    }
}
/// A key read from another compound, copied with its hash.
impl From<HashedKey<&JavaString>> for HashedKey<JavaString> {
    fn from(key: HashedKey<&JavaString>) -> Self {
        Self::from_parts(key.hash(), (*key.text()).clone())
    }
}

impl HashedKey<String> {
    pub fn new(text: String) -> Self {
        Self {
            hash: java_string_hash_str(&text),
            text,
        }
    }
}

impl HashedKey<Identifier> {
    /// Reuse the identifier's Java hash (31 times the namespace hash plus the path hash, wrapping).
    pub fn new(id: Identifier) -> Self {
        Self {
            hash: id.hash_code(),
            text: id,
        }
    }
    /// [`Identifier::from_parts`] as a key: the hash it computed in validating comes with it.
    pub fn from_namespace_path(
        namespace: &str,
        path: &str,
    ) -> Result<Self, crate::identifier::IdentifierError> {
        Identifier::from_parts(namespace, path).map(Self::new)
    }
    /// [`Identifier::parse`] as a key.
    pub fn parse(text: &str) -> Result<Self, crate::identifier::IdentifierError> {
        Identifier::parse(text).map(Self::new)
    }
}

impl<'a> HashedKey<&'a [u16]> {
    pub fn new(units: &'a [u16]) -> Self {
        Self {
            hash: java_string_hash(units),
            text: units,
        }
    }
    /// For constants: the hash is computed here, at compile time.
    pub const fn new_const(units: &'a [u16]) -> Self {
        Self {
            hash: java_string_hash_units_const(units),
            text: units,
        }
    }
}

impl<'a> HashedKey<&'a str> {
    pub fn new(text: &'a str) -> Self {
        Self {
            hash: java_string_hash_str(text),
            text,
        }
    }
}

impl<'a> HashedKey<&'a Identifier> {
    pub fn new(id: &'a Identifier) -> Self {
        Self {
            hash: id.hash_code(),
            text: id,
        }
    }
}

/// `nbt_key!("Count")`: a `&'static HashedKey<JavaString>` whose units and hash are made at compile
/// time. Looking it up neither encodes nor hashes anything, and inserting it never copies its units.
#[macro_export]
macro_rules! nbt_key {
    ($text:literal) => {{
        const LEN: usize = $crate::java_string::utf16_len_const($text);
        const UNITS: [u16; LEN] = $crate::java_string::utf16_units_const::<LEN>($text);
        static KEY: $crate::hashed_key::HashedKey<$crate::nbt::JavaString> =
            $crate::hashed_key::HashedKey::<$crate::nbt::JavaString>::from_static(&UNITS);
        &KEY
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_maker_computes_the_java_hash() {
        let text = "caf\u{e9}\u{1F600}";
        let units: Vec<u16> = text.encode_utf16().collect();
        let expected = java_string_hash(&units);
        assert_eq!(HashedKey::<JavaString>::new(units.clone()).hash(), expected);
        assert_eq!(HashedKey::<JavaString>::from_text(text).hash(), expected);
        assert_eq!(HashedKey::<String>::new(text.to_owned()).hash(), expected);
        assert_eq!(HashedKey::<&[u16]>::new(&units).hash(), expected);
        assert_eq!(HashedKey::<&[u16]>::new_const(&units).hash(), expected);
        assert_eq!(HashedKey::<&str>::new(text).hash(), expected);
        assert_eq!(crate::nbt_key!("caf\u{e9}\u{1F600}").hash(), expected);
        let id = Identifier::parse("minecraft:stone").unwrap();
        assert_eq!(HashedKey::<Identifier>::new(id.clone()).hash(), 182_203_480);
        assert_eq!(HashedKey::<&Identifier>::new(&id).hash(), 182_203_480);
    }
}
