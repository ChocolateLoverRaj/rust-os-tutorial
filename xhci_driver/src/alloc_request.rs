// This module is just for having a Rust library. This alloc request and response is not actually part of the xHCI spec.
use core::num::NonZero;

use alloc::boxed::Box;

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

/// This basically tells the kernel, I want multiple of these memory regions, but they don't have to be contiguous.
#[derive(Debug, Clone, Copy)]
pub struct MultiAllocRequest {
    pub request: AllocRequest,
    pub count: NonZero<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct AllocResponse {
    pub phys_addr: u64,
    pub virt_addr: NonZero<usize>,
}

pub type MultiAllocResponse = Box<[AllocResponse]>;

pub(crate) fn assert_res_matches(req: &AllocRequest, res: &AllocResponse) {
    assert!(
        res.phys_addr.is_multiple_of(req.align.get()),
        "phys addr should be aligned as requested"
    );
    assert!(
        res.virt_addr.get().is_multiple_of(req.align.get() as usize),
        "virt addr should also be aligned as requested, because if it's not, it's an indication that it is not mapped properly"
    );
    // Check that phys addr doesn't cross boundary
    let phys_end_inclusive = res.phys_addr + req.size.get() - 1;
    assert_eq!(
        res.phys_addr / req.boundary,
        phys_end_inclusive / req.boundary,
        "phys region must not cross requested boundary"
    );
}

pub(crate) fn assert_multi_res_matches(
    request: &Option<MultiAllocRequest>,
    response: &MultiAllocResponse,
) {
    if let Some(request) = request {
        assert_eq!(
            request.count.get(),
            response.len(),
            "request count should match response len"
        );
        for res in response {
            assert_res_matches(&request.request, res);
        }
    } else {
        assert!(
            response.is_empty(),
            "no alloc requests, so expected empty response"
        );
    }
}

pub(crate) fn assert_responses_match(
    requests: &[Option<MultiAllocRequest>],
    responses: &[MultiAllocResponse],
) {
    assert_eq!(requests.len(), responses.len());
    for i in 0..requests.len() {
        assert_multi_res_matches(&requests[i], &responses[i]);
    }
}
