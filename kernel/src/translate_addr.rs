use ez_paging::{Frame, Page};
use x86_64::{PhysAddr, VirtAddr};

use crate::hhdm_offset;

pub trait OffsetMappedPhysAddr {
    fn offset_mapped(self) -> VirtAddr;
}

impl OffsetMappedPhysAddr for PhysAddr {
    fn offset_mapped(self) -> VirtAddr {
        VirtAddr::new(self.as_u64() + u64::from(hhdm_offset()))
    }
}

pub trait OffsetMappedPhysFrame {
    fn offset_mapped(self) -> Page;
}

impl OffsetMappedPhysFrame for Frame {
    fn offset_mapped(self) -> Page {
        ez_paging::Page::new(self.start_addr().offset_mapped(), self.size()).unwrap()
    }
}
