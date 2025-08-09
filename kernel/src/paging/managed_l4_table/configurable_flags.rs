use x86_64::registers::model_specific::PatMemoryType;

/// All mappings are readable because they require the PRESENT flag.
/// Some flags are also used as flags for sub-pages.
/// For a page to be writable and user accessible, all parent flags must also have WRITABLE and USER_ACCESSIBLE.
/// For a page to be executable, the flags and all parent flags should **not** have the NO_EXECUTE flag.
/// The GLOBAL flag only exists for the lowest level page table. It does not exist in higher page tables, so the mapper does not need to handle setting the GLOBAL flag in parent page tables.
#[derive(Debug, Clone, Copy)]
pub struct ConfigurableFlags {
    pub writable: bool,
    pub executable: bool,
    pub pat_memory_type: PatMemoryType,
}
