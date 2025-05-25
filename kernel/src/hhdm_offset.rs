use core::fmt::Debug;

use limine::response::HhdmResponse;

/// A wrapper around u64 that represents the actual HHDM offset, and cannot be accidentally made.
/// Remember though that even though this wraps unsafeness in safeness, it is only safe if the assumption that all available memory is mapped in the current Cr3 value according to the HHDM offset (and cache is not invalid)
#[derive(Clone, Copy)]
pub struct HhdmOffset(u64);

impl Debug for HhdmOffset {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "HhdmOffset(0x{:X})", self.0)
    }
}

impl From<&'static HhdmResponse> for HhdmOffset {
    fn from(value: &'static HhdmResponse) -> Self {
        Self(value.offset())
    }
}

impl From<HhdmOffset> for u64 {
    fn from(value: HhdmOffset) -> Self {
        value.0
    }
}
