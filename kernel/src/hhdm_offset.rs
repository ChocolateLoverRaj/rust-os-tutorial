use core::fmt::Debug;

use limine::response::HhdmResponse;
use x86_64::VirtAddr;

use crate::limine_requests::HHDM_REQUEST;

/// A wrapper around u64 that represents the actual HHDM offset, and cannot be accidentally made.
/// Remember though that even though this wraps unsafeness in safeness, it is only safe if the assumption that all available memory is mapped in the current Cr3 value according to the HHDM offset (and cache is not invalid)
#[derive(Clone, Copy)]
pub struct HhdmOffset(u64);

impl HhdmOffset {
    /// Get the HHDM offset from the Limine response
    pub fn get_from_response() -> Self {
        HHDM_REQUEST.get_response().unwrap().into()
    }
}

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

impl From<HhdmOffset> for VirtAddr {
    fn from(value: HhdmOffset) -> Self {
        VirtAddr::new(value.0)
    }
}
