use core::ptr::NonNull;

use common::PageSize;
use raw_cpuid::CpuId;
use x86_64::structures::paging::{
    PageTable, PageTableFlags, PageTableIndex, PhysFrame, page_table::PageTableEntry,
};

use crate::{EffectiveFlags, Frame, translate_addr::TranslateToVirt};

use super::ManagedL4PageTable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTableLevel {
    L1,
    L2,
    L3,
    L4,
}

impl PageTableLevel {
    pub fn sub_level(self) -> Option<Self> {
        match self {
            PageTableLevel::L1 => None,
            PageTableLevel::L2 => Some(PageTableLevel::L1),
            PageTableLevel::L3 => Some(PageTableLevel::L2),
            PageTableLevel::L4 => Some(PageTableLevel::L3),
        }
    }

    pub fn target_frame_size(self) -> Option<PageSize> {
        match self {
            PageTableLevel::L1 => Some(PageSize::_4KiB),
            PageTableLevel::L2 => Some(PageSize::_2MiB),
            PageTableLevel::L3 => Some(PageSize::_1GiB),
            PageTableLevel::L4 => None,
        }
    }
}

#[derive(Debug)]
pub struct PageTableEntryWithLevelMut<'a> {
    entry: &'a mut PageTableEntry,
    level: PageTableLevel,
    l4: &'a ManagedL4PageTable,
}

#[derive(Debug)]
pub enum SetFrameError {
    /// Either the page table is a L4 table (you can't map a 512 GiB frame) or the frame size is incompatible with the table level.
    NotAllowed,
    /// This CPU cannot have 1 GiB page sizes
    PageSizeNotSupported,
}

#[derive(Debug)]
pub enum SetTableError {
    /// This page table is a L1 table and L1 entries don't point to another page table.
    IsL1,
}

#[derive(Debug)]
pub enum GetTableError {
    /// This is a L1 table and cannot point to another table
    IsL1,
    /// This entry is not mapped
    NotMapped,
    /// This entry is mapped, but not mapped to a table
    MappedToFrame,
}

#[derive(Debug)]
pub enum UnmapFrameError {
    IsL4,
    NotPresent,
    IsPageTable,
}

#[derive(Debug)]
pub enum SetFlagsError {
    IsL4,
    NotPresent,
    IsPageTable,
}

impl PageTableEntryWithLevelMut<'_> {
    pub fn is_empty(&self) -> bool {
        self.entry.is_unused()
    }

    fn get_auto_flags(&self) -> PageTableFlags {
        let mut flags = PageTableFlags::PRESENT;
        if !matches!(self.level, PageTableLevel::L1) {
            flags |= PageTableFlags::HUGE_PAGE
        }
        flags
    }

    pub fn set_frame(&mut self, frame: Frame, flags: EffectiveFlags) -> Result<(), SetFrameError> {
        let level_frame_match = match self.level {
            PageTableLevel::L1 => matches!(frame.size(), PageSize::_4KiB),
            PageTableLevel::L2 => matches!(frame.size(), PageSize::_2MiB),
            PageTableLevel::L3 => matches!(frame.size(), PageSize::_1GiB),
            PageTableLevel::L4 => false,
        };
        if !level_frame_match {
            return Err(SetFrameError::NotAllowed);
        }
        if frame.size() == PageSize::_1GiB
            && !CpuId::new()
                .get_extended_processor_and_feature_identifiers()
                .is_some_and(|info| info.has_1gib_pages())
        {
            return Err(SetFrameError::PageSizeNotSupported);
        }
        self.entry.set_addr(
            frame.start_addr(),
            self.get_auto_flags() | flags.page_table_flags(&self.l4.pat, frame.size()),
        );
        Ok(())
    }

    /// Returns the frame that was unmapped
    pub fn unmap_frame(&mut self) -> Result<Frame, UnmapFrameError> {
        let frame_size = self
            .level
            .target_frame_size()
            .ok_or(UnmapFrameError::IsL4)?;
        if !self.entry.flags().contains(PageTableFlags::PRESENT) {
            return Err(UnmapFrameError::NotPresent);
        }
        if !self.entry.flags().contains(PageTableFlags::HUGE_PAGE)
            && !matches!(frame_size, PageSize::_4KiB)
        {
            return Err(UnmapFrameError::IsPageTable);
        }
        let start_addr = self.entry.addr();
        self.entry.set_unused();
        Ok(Frame::new(start_addr, frame_size).unwrap())
    }

    /// Only sets flags for pointing to a frame, not pointing to a table
    pub fn set_flags(&mut self, flags: EffectiveFlags) -> Result<(), SetFlagsError> {
        let page_size = self.level.target_frame_size().ok_or(SetFlagsError::IsL4)?;
        if !self.entry.flags().contains(PageTableFlags::PRESENT) {
            return Err(SetFlagsError::NotPresent);
        }
        if !self.entry.flags().contains(PageTableFlags::HUGE_PAGE)
            && !matches!(page_size, PageSize::_4KiB)
        {
            return Err(SetFlagsError::IsPageTable);
        }
        self.entry
            .set_flags(self.get_auto_flags() | flags.page_table_flags(&self.l4.pat, page_size));
        Ok(())
    }
}

impl<'a> PageTableEntryWithLevelMut<'a> {
    /// This method also zeroes the frame
    pub fn set_page_table(
        self,
        frame: PhysFrame,
    ) -> Result<PageTableWithLevelMut<'a>, SetTableError> {
        let page_table_level = self.level.sub_level().ok_or(SetTableError::IsL1)?;
        if self.level == PageTableLevel::L4 && !self.l4.map_restrictions.can_create_new_l4_entries()
        {
            panic!(
                "Cannot create new L3 pages because the kernel page table would be out of sync with user page tables"
            )
        }
        let ptr = NonNull::new(frame.start_address().to_virt().as_mut_ptr::<PageTable>()).unwrap();
        unsafe { ptr.write_bytes(0, 1) };

        self.entry.set_frame(
            frame,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
        );
        Ok(PageTableWithLevelMut {
            page_table: ptr,
            level: page_table_level,
            l4: self.l4,
        })
    }

    pub fn get_page_table_mut(self) -> Result<PageTableWithLevelMut<'a>, GetTableError> {
        let page_table_level = self.level.sub_level().ok_or(GetTableError::IsL1)?;
        if self.entry.is_unused() {
            return Err(GetTableError::NotMapped);
        }
        if self.entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            return Err(GetTableError::MappedToFrame);
        }
        let frame = self.entry.frame(false).unwrap();
        let ptr = NonNull::new(frame.start_address().to_virt().as_mut_ptr::<PageTable>()).unwrap();
        Ok(PageTableWithLevelMut {
            page_table: ptr,
            level: page_table_level,
            l4: self.l4,
        })
    }
}

#[derive(Debug)]
pub struct PageTableWithLevelMut<'a> {
    pub(super) l4: &'a ManagedL4PageTable,
    pub(super) page_table: NonNull<PageTable>,
    pub(super) level: PageTableLevel,
}

impl<'a> PageTableWithLevelMut<'a> {
    pub fn entry_mut(mut self, index: PageTableIndex) -> PageTableEntryWithLevelMut<'a> {
        if self.level == PageTableLevel::L4 {
            let range = self.l4.map_restrictions.l4_managed_entry_range();
            if !range.contains(&index) {
                panic!(
                    "Cannot access L4 entry {index:?} because it is outside of the range managed by this page table ({range:?})"
                )
            }
        }
        PageTableEntryWithLevelMut {
            entry: {
                let mut ptr = NonNull::from_mut(&mut unsafe { self.page_table.as_mut() }[index]);
                // Safety: We are still capturing a &mut to the managed L4 table
                unsafe { ptr.as_mut() }
            },
            level: self.level,
            l4: self.l4,
        }
    }
}
