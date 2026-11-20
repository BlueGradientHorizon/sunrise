//! Small UTF-16 storage, shared by the plain and hashed string constructors.

pub(crate) const INLINE_CAPACITY: usize = 8;

#[derive(Clone)]
pub(crate) enum Units {
    Static(&'static [u16]),
    Inline {
        len: u8,
        units: [u16; INLINE_CAPACITY - 1],
    },
    // The discriminant supplies the length when all eight units are in use.
    Full([u16; INLINE_CAPACITY]),
    Heap(Vec<u16>),
}

impl Units {
    pub(crate) const EMPTY: Self = Self::Static(&[]);

    pub(crate) fn inline(units: [u16; INLINE_CAPACITY], len: usize) -> Self {
        match len {
            0 => Self::EMPTY,
            INLINE_CAPACITY => Self::Full(units),
            _ => {
                let [units @ .., _] = units;
                Self::Inline {
                    len: len as u8,
                    units,
                }
            }
        }
    }

    pub(crate) fn as_slice(&self) -> &[u16] {
        match self {
            Self::Static(units) => units,
            Self::Inline { len, units } => &units[..usize::from(*len)],
            Self::Full(units) => units,
            Self::Heap(units) => units,
        }
    }
}

impl From<Vec<u16>> for Units {
    fn from(value: Vec<u16>) -> Self {
        if value.len() > INLINE_CAPACITY {
            return Self::Heap(value);
        }
        let mut units = [0; INLINE_CAPACITY];
        units[..value.len()].copy_from_slice(&value);
        Self::inline(units, value.len())
    }
}

impl FromIterator<u16> for Units {
    fn from_iter<T: IntoIterator<Item = u16>>(iter: T) -> Self {
        let mut iter = iter.into_iter();
        if iter.size_hint().0 > INLINE_CAPACITY {
            return Self::Heap(iter.collect());
        }
        let mut units = [0; INLINE_CAPACITY];
        for (i, unit) in units.iter_mut().enumerate() {
            let Some(next) = iter.next() else {
                return Self::inline(units, i);
            };
            *unit = next;
        }
        match iter.next() {
            None => Self::inline(units, INLINE_CAPACITY),
            Some(next) => Self::Heap(units.into_iter().chain([next]).chain(iter).collect()),
        }
    }
}
