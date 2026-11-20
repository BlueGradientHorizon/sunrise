//! Small UTF-16 storage, shared by the plain and hashed string constructors.

pub(crate) const INLINE_CAPACITY: usize = 8;

#[derive(Clone)]
pub(crate) enum Units {
    Inline {
        len: u8,
        units: [u16; INLINE_CAPACITY - 1],
    },
    // The discriminant supplies the length when all eight units are in use.
    Full([u16; INLINE_CAPACITY]),
    Long(Long),
}

// Keep heap and static slices in a single outer case. A flat four-case match
// made the lookup projection use an indirect jump, even for a heap key.
#[derive(Clone)]
pub(crate) enum Long {
    Heap(Vec<u16>),
    Static(&'static [u16]),
}

impl Units {
    pub(crate) const EMPTY: Self = Self::from_static(&[]);

    pub(crate) const fn from_static(units: &'static [u16]) -> Self {
        Self::Long(Long::Static(units))
    }

    pub(crate) fn heap(units: Vec<u16>) -> Self {
        Self::Long(Long::Heap(units))
    }

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
            Self::Long(Long::Static(units)) => units,
            Self::Inline { len, units } => &units[..usize::from(*len)],
            Self::Full(units) => units,
            Self::Long(Long::Heap(units)) => units,
        }
    }

    pub(crate) fn as_vec_mut(&mut self) -> &mut Vec<u16> {
        if !matches!(self, Self::Long(Long::Heap(_))) {
            *self = Self::heap(self.as_slice().to_vec());
        }
        let Self::Long(Long::Heap(units)) = self else {
            unreachable!()
        };
        units
    }

    pub(crate) fn into_vec(self) -> Vec<u16> {
        match self {
            Self::Long(Long::Heap(units)) => units,
            units => units.as_slice().to_vec(),
        }
    }

    pub(crate) fn heap_capacity(&self) -> usize {
        match self {
            Self::Long(Long::Heap(units)) => units.capacity(),
            _ => 0,
        }
    }
}

impl From<Vec<u16>> for Units {
    fn from(value: Vec<u16>) -> Self {
        if value.len() > INLINE_CAPACITY {
            return Self::heap(value);
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
            return Self::heap(iter.collect());
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
            Some(next) => Self::heap(units.into_iter().chain([next]).chain(iter).collect()),
        }
    }
}
