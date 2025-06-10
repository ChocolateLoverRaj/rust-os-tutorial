use bitflags::bitflags;
use elf::segment::ProgramHeader;
use x86_64::structures::paging::PageTableFlags;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct ElfSegmentFlags: u32 {
        const EXECUTABLE = 0b001;
        const WRITABLE = 0b010;
        const READABLE = 0b100;

        // The source may set any bits
        const _ = !0;
    }
}

impl From<ProgramHeader> for ElfSegmentFlags {
    fn from(value: ProgramHeader) -> Self {
        Self::from_bits_retain(value.p_flags)
    }
}

impl ElfSegmentFlags {
    /// Only specifies `WRITABLE` and `NO_EXECUTE` if needed. Other flags such as `PRESENT` and `USER_ACCESSIBLE` must be added.
    /// https://en.wikipedia.org/wiki/Executable_and_Linkable_Format#Program_header
    pub fn to_page_table_flags(self) -> PageTableFlags {
        let mut page_table_flags = PageTableFlags::empty();
        if !self.contains(ElfSegmentFlags::EXECUTABLE) {
            page_table_flags |= PageTableFlags::NO_EXECUTE;
        }
        if self.contains(ElfSegmentFlags::WRITABLE) {
            page_table_flags |= PageTableFlags::WRITABLE;
        }
        page_table_flags
    }
}
