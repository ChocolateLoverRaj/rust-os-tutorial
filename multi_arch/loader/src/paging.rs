pub enum MapPageResult {
    Mapped { new_page_tables_used: usize },
    NotMapped { n_page_tables_needed: usize },
}

pub enum MappingFlags {
    ReadWriteExecute,
    ReadWrite,
    ReadExecute,
    Read,
}

pub trait Paging {
    const PAGE_SIZE: usize;
    type PhysPagenumber;
    type VirtPageNumber;
    const MAX_NEW_PAGES_NEEDED: usize;

    fn new_page(bytes: &mut [u8; Self::PAGE_SIZE]);
    fn map_page(
        page_table: &mut [u8; Self::PAGE_SIZE],
        virt_page: Self::VirtPageNumber,
        phys_page: Self::PhysPagenumber,
        flags: MappingFlags,
        new_page_tables: &mut [&mut [u8; Self::PAGE_SIZE]],
    ) -> MapPageResult;
}
