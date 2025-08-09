use core::ops::RangeInclusive;

use common::{HIGHER_HALF_START, PageSize};
use nodit::{Interval, NoditSet, interval::iu};
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
};

use crate::{ConfigurableFlags, Frame, MANAGED_PAT, ManagedL4PageTable, MapPageError2, Page};

pub struct VirtualMemory {
    pub(super) set: NoditSet<u64, Interval<u64>>,
    pub(super) l4: ManagedL4PageTable,
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

    /// # Safety
    /// You must "own" the frame (nothing else can reference it)
    pub unsafe fn new_user_page_table(&mut self, frame: PhysFrame) -> ManagedL4PageTable {
        unsafe { ManagedL4PageTable::new_user(&mut self.l4, frame, MANAGED_PAT) }
    }
}
pub struct AllocatedPages<'a> {
    virtual_memory: &'a mut VirtualMemory,
    range: RangeInclusive<u64>,
    page_size: PageSize,
}

#[derive(Debug)]
pub enum MapToError {
    SizeMismatch,
    OutsideOfRange,
    OutOfPhysMem,
}

impl AllocatedPages<'_> {
    pub fn start_addr(&self) -> VirtAddr {
        VirtAddr::new(*self.range.start())
    }

    pub fn start_page(&self) -> Page {
        Page::new(self.start_addr(), self.page_size).unwrap()
    }

    pub fn range(&self) -> &RangeInclusive<u64> {
        &self.range
    }

    /// # Safety
    /// See the safety for [`x86_64::structures::paging::mapper::Mapper::map_to`]
    pub unsafe fn map_to(
        &mut self,
        page: Page,
        frame: Frame,
        flags: ConfigurableFlags,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<(), MapToError> {
        if page.size() != self.page_size || frame.size() != self.page_size {
            return Err(MapToError::SizeMismatch);
        }
        if !self.range.contains(&page.start_addr().as_u64()) {
            return Err(MapToError::OutsideOfRange);
        }
        let result = unsafe {
            self.virtual_memory
                .l4
                .map_page(page, frame, flags, frame_allocator)
        };
        if let Err(MapPageError2::FrameAllocationFailed) = result {
            return Err(MapToError::OutOfPhysMem);
        } else {
            result.unwrap();
        }
        Ok(())
    }

    /// All pages must be mapped
    pub fn unmap_and_deallocate(self) {
        let first_page = Page::new(VirtAddr::new(*self.range.start()), self.page_size).unwrap();
        let pages_len = (self.range.end() - self.range.start() + 1) / self.page_size.byte_len_u64();
        for i in 0..pages_len {
            let page = first_page.offset(i).unwrap();
            unsafe { self.virtual_memory.l4.unmap_page(page) }.unwrap();
        }
        let _ = self.virtual_memory.set.cut(Interval::from(self.range));
    }
}
