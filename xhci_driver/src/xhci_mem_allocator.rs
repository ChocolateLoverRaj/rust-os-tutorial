// This module is just for having a Rust library. This alloc request and response is not actually part of the xHCI spec.
use core::num::NonZero;

/// # Safety
/// You must allocate physical memory and make sure it is mapped as Strong Uncacheable (UC).
pub unsafe trait XhciMemAllocator {
    fn alloc(&mut self, request: AllocRequest) -> AllocResponse;
}

/// Requirements for kernel-specific allocation.
/// Note that on x86_64 you should have the memory type be WB (write-back)
#[derive(Debug, Clone, Copy)]
pub struct AllocRequest {
    /// Cannot be greater than the boundary.
    /// Does **not** have to be a multiple of align.
    pub size: NonZero<u64>,
    /// Must be a power of 2.
    /// Cannot be greater than the boundary.
    pub align: NonZero<u64>,
    /// Must be a power of 2.
    pub boundary: NonZero<u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct AllocResponse {
    pub phys_addr: u64,
    pub virt_addr: NonZero<usize>,
}
