use core::{ops::RangeInclusive, ptr::NonNull};

use x86_64::{
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{PageTable, PageTableIndex, PhysFrame},
};

use crate::translate_addr::{TranslateToVirt, ZeroFrame};

use super::page_table_with_level::{PageTableLevel, PageTableWithLevelMut};

#[derive(Debug)]
pub struct HigherHalfRestrictions {
    is_referenced: bool,
}

#[derive(Debug)]
pub(super) enum MapRestrictions {
    LowerHalf,
    HigherHalf(HigherHalfRestrictions),
}

impl MapRestrictions {
    pub fn l4_managed_entry_range(&self) -> RangeInclusive<PageTableIndex> {
        match self {
            Self::LowerHalf => PageTableIndex::new(0)..=PageTableIndex::new(255),
            Self::HigherHalf(_) => PageTableIndex::new(256)..=PageTableIndex::new(511),
        }
    }

    pub fn can_create_new_l4_entries(&self) -> bool {
        match self {
            Self::LowerHalf => true,
            Self::HigherHalf(HigherHalfRestrictions { is_referenced }) => !is_referenced,
        }
    }
}

#[derive(Debug)]
pub struct ManagedL4PageTable {
    frame: PhysFrame,
    map_restrictions: MapRestrictions,
}

impl ManagedL4PageTable {
    /// This method also zeroes the frame.
    ///
    /// # Safety
    /// You must "own" the frame (nothing else can reference it)
    pub unsafe fn new_kernel(frame: PhysFrame) -> Self {
        unsafe { frame.zero() };
        Self {
            frame,
            map_restrictions: MapRestrictions::HigherHalf(HigherHalfRestrictions {
                is_referenced: false,
            }),
        }
    }

    /// # Safety
    /// You must "own" the frame (nothing else can reference it)
    pub unsafe fn new_user(&mut self, frame: PhysFrame) -> Self {
        match &mut self.map_restrictions {
            MapRestrictions::LowerHalf => {
                panic!("self must be a kernel's l4 frame to copy from it")
            }
            MapRestrictions::HigherHalf(HigherHalfRestrictions { is_referenced }) => {
                *is_referenced = true;
            }
        };
        unsafe { frame.zero() };
        let mut lower_half = Self {
            frame,
            map_restrictions: MapRestrictions::LowerHalf,
        };
        let range_to_copy = self.map_restrictions.l4_managed_entry_range();
        let kernel_page_table = unsafe { self.page_table_mut().as_mut() };
        let user_page_table = unsafe { lower_half.page_table_mut().as_mut() };
        for index in range_to_copy {
            user_page_table[index].clone_from(&kernel_page_table[index]);
        }
        lower_half
    }

    pub fn frame(&self) -> PhysFrame {
        self.frame
    }

    fn page_table_mut(&mut self) -> NonNull<PageTable> {
        NonNull::new(
            self.frame
                .start_address()
                .to_virt()
                .as_mut_ptr::<PageTable>(),
        )
        .unwrap()
    }

    pub(super) fn table_mut(&mut self) -> PageTableWithLevelMut {
        PageTableWithLevelMut {
            page_table: self.page_table_mut(),
            level: PageTableLevel::L4,
            l4_restrictions: &self.map_restrictions,
        }
    }

    /// # Safety
    /// Changes Cr3 value
    pub unsafe fn switch_to(&self, flags: Cr3Flags) {
        unsafe { Cr3::write(self.frame, flags) };
    }
}
