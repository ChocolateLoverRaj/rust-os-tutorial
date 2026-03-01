use arbitrary_int::{u20, u22};
use bitbybit::bitfield;
use loader::paging::Paging;

pub struct Sv32PageTable {
    entries: [Sv32PageTableEntry; 0x400],
}

#[bitfield(u32)]
pub struct Sv32PageTableEntry {
    #[bits(10..=31, rw)]
    physical_page_number: u22,
    #[bit(7)]
    d: bool,
    #[bit(6)]
    a: bool,
    #[bit(5)]
    g: bool,
    #[bit(4)]
    u: bool,
    #[bit(3)]
    x: bool,
    #[bit(2)]
    w: bool,
    #[bit(1)]
    r: bool,
    #[bit(0)]
    v: bool,
}

pub struct RiscvPaging;
impl Paging for RiscvPaging {
    const PAGE_SIZE: usize = 0x1000;

    type PhysPagenumber = u22;

    type VirtPageNumber = u20;

    const MAX_NEW_PAGES_NEEDED: usize = 1;

    fn new_page(bytes: &mut [u8; Self::PAGE_SIZE]) {
        todo!()
    }

    fn map_page(
        page_table: &mut [u8; Self::PAGE_SIZE],
        virt_page: Self::VirtPageNumber,
        phys_page: Self::PhysPagenumber,
        flags: loader::paging::MappingFlags,
        new_page_tables: &mut [&mut [u8; Self::PAGE_SIZE]],
    ) -> loader::paging::MapPageResult {
        todo!()
    }
}
