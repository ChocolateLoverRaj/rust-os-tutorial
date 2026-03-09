use core::{
    fmt::{Arguments, Debug},
    mem::MaybeUninit,
};

use num_traits::{CheckedAdd, CheckedRem, Num};

use crate::paging::Paging;

#[derive(Debug)]
pub struct MapPageError {
    pub n_page_tables_needed: usize,
}

#[derive(Debug)]
pub enum MappingFlags {
    ReadWriteExecute,
    ReadWrite,
    ReadExecute,
    Read,
}

pub trait Arch {
    type Paging: Paging;
    type PhysAddr: Debug
        + Num
        + CheckedAdd<Output = Self::PhysAddr>
        + Ord
        + Copy
        + TryFrom<u64, Error: Debug>
        + TryFrom<usize, Error: Debug>
        + TryInto<usize, Error: Debug>
        + CheckedRem<Output = Self::PhysAddr>;
    /// Must be `[u8; PAGE_SIZE]`
    type Page;
    type PhysPageNumber: TryFrom<Self::PhysAddr, Error: Debug>;
    type VirtPageNumber: TryFrom<usize, Error: Debug>;
    const MAX_NEW_PAGES_NEEDED: usize;

    fn early_log(arguments: Arguments<'_>);
    fn can_shutdown() -> bool;
    fn shutdown() -> !;
    fn low_power_loop() -> !;
    fn new_page(bytes: &mut MaybeUninit<Self::Page>) -> &mut Self::Page;
    /// # Safety
    /// Assumes that the physical addresses of the pointee page tables can be accessed at the exact address (MMU is disabled).
    unsafe fn map_page(
        page_table: &mut Self::Page,
        virt_page: Self::VirtPageNumber,
        phys_page: Self::PhysPageNumber,
        flags: MappingFlags,
        new_page_tables: &mut heapless::VecView<&mut Self::Page>,
    ) -> Result<(), MapPageError>;
    /// # Safety
    /// Assumes that the physical addresses of the pointee page tables can be accessed at the exact address (MMU is disabled).
    unsafe fn debug_page_tables(page_table: &'_ mut Self::Page) -> impl Debug + '_;
}
