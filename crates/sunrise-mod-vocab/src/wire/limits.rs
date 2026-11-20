//! The optional shared allocation budget of a typed payload, not a second codec.
use super::WireError;
use std::alloc::Layout;

/// The deepest a typed payload may nest in Wire.
pub const PAYLOAD_MAX_DEPTH: u16 = 64;
/// The largest UTF-8 string in a typed payload, in bytes.
pub const PAYLOAD_MAX_STRING_BYTES: usize = 4096;
/// The largest variable-length collection in a typed payload, in entries.
pub const PAYLOAD_MAX_LIST_ENTRIES: usize = 4096;
/// The allocation budget for a typed payload value, and its maximum encoded body size.
/// Owned collection storage is charged at 64-bit sizes, even on a 32-bit target.
pub const PAYLOAD_DECODE_BUDGET: usize = 8_388_608;

pub(super) struct Budget(u64);

impl Budget {
    pub(super) fn payload() -> Self {
        Self(PAYLOAD_DECODE_BUDGET as u64)
    }

    pub(super) fn charge<T>(&mut self, count: usize) -> Result<(), WireError> {
        // On a 32-bit target this conservatively scales every field by the pointer ratio.
        let width = (std::mem::size_of::<T>() as u64) * (8 / std::mem::size_of::<usize>() as u64);
        self.bytes((count as u64).checked_mul(width).ok_or(WireError::Budget)?)
    }

    pub(super) fn arc<T>(&mut self, count: usize) -> Result<(), WireError> {
        self.bytes(arc_bytes::<T>(count)?)
    }

    pub(super) fn arc_str_overhead(&mut self, len: usize) -> Result<(), WireError> {
        self.bytes(arc_bytes::<u8>(len)? - len as u64)
    }

    pub(super) fn bytes(&mut self, bytes: u64) -> Result<(), WireError> {
        self.0 = self.0.checked_sub(bytes).ok_or(WireError::Budget)?;
        Ok(())
    }
}

fn arc_bytes<T>(count: usize) -> Result<u64, WireError> {
    // An Arc allocation holds both counts and the aligned value, even for a ZST.
    let values = Layout::array::<T>(count).map_err(|_| WireError::Budget)?;
    let (layout, _) = Layout::new::<[usize; 2]>()
        .extend(values)
        .map_err(|_| WireError::Budget)?;
    (layout.pad_to_align().size() as u64)
        .checked_mul(8 / std::mem::size_of::<usize>() as u64)
        .ok_or(WireError::Budget)
}
