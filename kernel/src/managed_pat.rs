use common::PageSize;
use x86_64::{
    registers::model_specific::{Pat, PatMemoryType},
    structures::paging::PageTableFlags,
};

/// A "guarantee" that the PAT MSR won't be modified
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ManagedPat;

impl ManagedPat {
    /// # Safety
    /// Do not write to the PAT MSR after creating this
    pub const unsafe fn new() -> Self {
        Self {}
    }

    // pub fn read(&self) -> [PatMemoryType; 8] {
    //     Pat::read()
    // }

    /// Get the necessary page table bits needed to set the caching memory type of a page.
    /// If for some reason there is no entry in the PAT MSR with the memory type, `None` is returned.
    pub fn get_page_table_flags(
        &self,
        memory_type: PatMemoryType,
        page_size: PageSize,
    ) -> Option<PageTableFlags> {
        let pat_msr_index = Pat::read().iter().position(|v| *v == memory_type)?;
        // See Intel SDM -> Volume 3 -> 13.12.3 Selecting a Memory Type from the PAT
        let mut flags = PageTableFlags::empty();
        if pat_msr_index & 0b001 != 0 {
            flags |= PageTableFlags::WRITE_THROUGH;
        }
        if pat_msr_index & 0b010 != 0 {
            flags |= PageTableFlags::NO_CACHE;
        }
        if pat_msr_index & 0b100 != 0 {
            flags |= match page_size {
                PageSize::_1GiB | PageSize::_2MiB => PageTableFlags::PAT_HUGE_PAGE,
                PageSize::_4KiB => PageTableFlags::PAT_4KIB_PAGE,
            };
        }
        Some(flags)
    }
}

/// Safety: We do not ever modify the PAT MSR. Limine sets it and it doesn't get modified after that.
pub const MANAGED_PAT: ManagedPat = unsafe { ManagedPat::new() };
