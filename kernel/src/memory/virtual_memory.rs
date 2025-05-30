use core::{fmt::Debug, ops::RangeInclusive};

use nodit::{Interval, NoditSet, interval::iu};
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags,
        PhysFrame, Size4KiB,
    },
};

use crate::hhdm_offset::HhdmOffset;

pub struct VirtualMemory {
    pub(super) set: NoditSet<u64, Interval<u64>>,
    pub(super) cr3: PhysFrame<Size4KiB>,
    pub(super) hhdm_offset: HhdmOffset,
}

impl VirtualMemory {
    /// Returns the start page of the allocated range of pages.
    /// Pages are guaranteed not to be mapped.
    pub fn allocate_contiguous_pages<S: PageSize + Debug>(
        &mut self,
        n_pages: u64,
    ) -> Option<AllocatedPages<S>> {
        let start_page = Page::<S>::from_start_address(VirtAddr::new({
            let range = self
                .set
                .gaps_trimmed(iu(0xffff800000000000))
                .find_map(|gap| {
                    let aligned_start = gap.start().next_multiple_of(S::SIZE);
                    let required_end_inclusive = aligned_start + (n_pages * S::SIZE - 1);
                    if required_end_inclusive <= gap.end() {
                        Some(aligned_start..=required_end_inclusive)
                    } else {
                        None
                    }
                })?;
            let start = *range.start();
            self.set
                .insert_merge_touching(Interval::from(range))
                .unwrap();
            start
        }))
        .unwrap();
        Some(AllocatedPages {
            virtual_memory: self,
            range: start_page..=start_page + (n_pages - 1),
        })
    }

    /// # Safety
    /// The pages must have been allocated by [`VirtualMemory`]
    pub unsafe fn already_allocated<S: PageSize>(
        &mut self,
        pages: RangeInclusive<Page<S>>,
    ) -> AllocatedPages<'_, S> {
        AllocatedPages {
            virtual_memory: self,
            range: pages,
        }
    }
}

pub struct AllocatedPages<'a, S: PageSize> {
    virtual_memory: &'a mut VirtualMemory,
    range: RangeInclusive<Page<S>>,
}

impl<S: PageSize> AllocatedPages<'_, S> {
    pub fn range(&self) -> &RangeInclusive<Page<S>> {
        &self.range
    }

    fn get_offset_page_table(&self) -> OffsetPageTable<'_> {
        let level_4_page_table = VirtAddr::new(
            u64::from(self.virtual_memory.hhdm_offset)
                + self.virtual_memory.cr3.start_address().as_u64(),
        )
        .as_mut_ptr::<PageTable>();
        // Safety: We can access it through HHDM
        let level_4_page_table = unsafe { level_4_page_table.as_mut() }.unwrap();
        // Safety: No other code is currently modifying page tables
        unsafe {
            OffsetPageTable::new(
                level_4_page_table,
                VirtAddr::new(self.virtual_memory.hhdm_offset.into()),
            )
        }
    }

    /// # Safety
    /// See the safety for [`x86_64::structures::paging::mapper::Mapper::map_to`]
    pub unsafe fn map_to(
        &mut self,
        page: Page<S>,
        frame: PhysFrame<S>,
        flags: PageTableFlags,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) where
        S: Debug,
        for<'a> OffsetPageTable<'a>: Mapper<S>,
    {
        if self.range.contains(&page) {
            let mut offset_page_table = self.get_offset_page_table();
            // Safety: same as this function's safety, plus we ensure that the page we are mapping is allocated properly
            unsafe { offset_page_table.map_to(page, frame, flags, frame_allocator) }
                .unwrap()
                .flush();
        } else {
            panic!(
                "Tried to map page {page:?}, which is outside of allocated range {:?}",
                self.range
            )
        }
    }

    /// All pages must be mapped
    pub fn unmap_and_deallocate(self)
    where
        for<'a> OffsetPageTable<'a>: Mapper<S>,
    {
        let pages = self.range.clone();
        let mut offset_page_table = self.get_offset_page_table();
        for page in pages.clone() {
            offset_page_table.unmap(page).unwrap().1.flush();
        }
        let _ = self.virtual_memory.set.cut({
            let start = pages.start().start_address().as_u64();
            let end_inclusive = pages.end().start_address().as_u64() + (S::SIZE - 1);
            Interval::from(start..=end_inclusive)
        });
    }
}
