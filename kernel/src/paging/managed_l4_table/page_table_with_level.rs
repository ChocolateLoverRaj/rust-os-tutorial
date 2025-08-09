use core::ptr::NonNull;

use common::PageSize;
use x86_64::structures::paging::{PageTable, PageTableIndex};

use super::{ManagedL4PageTable, PageTableEntryWithLevelMut};

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
pub struct PageTableWithLevelMut<'a> {
    pub(super) l4: &'a ManagedL4PageTable,
    pub(super) page_table: NonNull<PageTable>,
    pub(super) level: PageTableLevel,
}

impl<'a> PageTableWithLevelMut<'a> {
    pub fn entry_mut(mut self, index: PageTableIndex) -> PageTableEntryWithLevelMut<'a> {
        if self.level == PageTableLevel::L4 {
            let range = self.l4._type.l4_managed_entry_range();
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
