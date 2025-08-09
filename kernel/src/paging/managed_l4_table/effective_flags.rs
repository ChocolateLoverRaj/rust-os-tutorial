use common::PageSize;
use x86_64::{registers::model_specific::PatMemoryType, structures::paging::PageTableFlags};

use crate::ManagedPat;

/// All mappings are readable because they require the PRESENT flag.
/// Some flags are also used as flags for sub-pages.
/// For a page to be writable and user accessible, all parent flags must also have WRITABLE and USER_ACCESSIBLE.
/// For a page to be executable, the flags and all parent flags should **not** have the NO_EXECUTE flag.
/// The GLOBAL flag only exists for the lowest level page table. It does not exist in higher page tables, so the mapper does not need to handle setting the GLOBAL flag in parent page tables.
#[derive(Debug, Clone, Copy)]
pub struct EffectiveFlags {
    pub writable: bool,
    pub executable: bool,
    pub user_accessible: bool,
    pub global: bool,
    pub pat_memory_type: PatMemoryType,
}

impl EffectiveFlags {
    pub(super) fn page_table_flags(&self, pat: &ManagedPat, page_size: PageSize) -> PageTableFlags {
        let mut flags = pat
            .get_page_table_flags(self.pat_memory_type, page_size)
            .expect("There are only 6 memory types and 8 slots, so all memory types should be present in the slots");
        if self.writable {
            flags |= PageTableFlags::WRITABLE;
        }
        if !self.executable {
            flags |= PageTableFlags::NO_EXECUTE;
        }
        if self.user_accessible {
            flags |= PageTableFlags::USER_ACCESSIBLE;
        }
        if self.global {
            flags |= PageTableFlags::GLOBAL;
        }
        flags
    }
}
