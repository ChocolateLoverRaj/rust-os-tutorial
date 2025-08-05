use core::ops::RangeInclusive;

use common::{HIGHER_HALF_START, PageSize};
use nodit::{Interval, NoditSet, interval::iu};
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{FrameAllocator, PageTableFlags, PhysFrame, Size4KiB},
};

use crate::{MapPageError, map_page, unmap_page};

pub struct VirtualMemory {
    pub(super) set: NoditSet<u64, Interval<u64>>,
    pub(super) cr3: PhysFrame<Size4KiB>,
}

impl VirtualMemory {
    /// Returns the start page of the allocated range of pages.
    /// Pages are guaranteed not to be mapped.
    pub fn allocate_contiguous_pages(
        &mut self,
        page_size: PageSize,
        n_pages: u64,
    ) -> Option<AllocatedPages> {
        let range = self
            .set
            .gaps_trimmed(iu(HIGHER_HALF_START))
            .find_map(|gap| {
                let aligned_start = gap.start().next_multiple_of(page_size.byte_len_u64());
                let required_end_inclusive =
                    aligned_start + (n_pages * page_size.byte_len_u64() - 1);
                if required_end_inclusive <= gap.end() {
                    Some(aligned_start..=required_end_inclusive)
                } else {
                    None
                }
            })?;
        self.set
            .insert_merge_touching(Interval::from(range.clone()))
            .unwrap();
        Some(AllocatedPages {
            virtual_memory: self,
            range,
            page_size,
        })
    }

    /// # Safety
    /// The pages must have been allocated by [`VirtualMemory`]
    pub unsafe fn already_allocated(
        &mut self,
        page_size: PageSize,
        range: RangeInclusive<u64>,
    ) -> AllocatedPages<'_> {
        AllocatedPages {
            virtual_memory: self,
            range,
            page_size,
        }
    }
}
pub struct AllocatedPages<'a> {
    virtual_memory: &'a mut VirtualMemory,
    range: RangeInclusive<u64>,
    page_size: PageSize,
}

#[derive(Debug)]
pub enum MapToError {
    OutOfPhysMem,
}

impl AllocatedPages<'_> {
    pub fn start_addr(&self) -> VirtAddr {
        VirtAddr::new(*self.range.start())
    }

    pub fn range(&self) -> &RangeInclusive<u64> {
        &self.range
    }

    /// # Safety
    /// See the safety for [`x86_64::structures::paging::mapper::Mapper::map_to`]
    pub unsafe fn map_to(
        &mut self,
        page: VirtAddr,
        frame: PhysAddr,
        flags: PageTableFlags,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<(), MapToError> {
        if self.range.contains(&page.as_u64()) {
            let result = unsafe {
                map_page(
                    self.virtual_memory.cr3,
                    self.page_size,
                    page,
                    frame,
                    flags,
                    frame_allocator,
                )
            };
            if let Err(MapPageError::AllocateFrame) = result {
                return Err(MapToError::OutOfPhysMem);
            } else {
                result.unwrap();
            }
        } else {
            panic!(
                "Tried to map page {page:?}, which is outside of allocated range {:X?}",
                self.range
            )
        }
        Ok(())
    }

    /// All pages must be mapped
    pub fn unmap_and_deallocate(self) {
        let first_page = VirtAddr::new(*self.range.start());
        let pages_len = (self.range.end() - self.range.start() + 1) / self.page_size.byte_len_u64();
        for i in 0..pages_len {
            let page = first_page + i * self.page_size.byte_len_u64();
            unsafe { unmap_page(self.virtual_memory.cr3, self.page_size, page) }.unwrap();
        }
        let _ = self.virtual_memory.set.cut(Interval::from(self.range));
    }
}
