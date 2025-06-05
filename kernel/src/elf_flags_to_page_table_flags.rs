use bitflags::bitflags;
use x86_64::structures::paging::PageTableFlags;

bitflags! {
    struct ElfSegmentFlags: u32 {
        const EXECUTABLE = 0b001;
        const WRITABLE = 0b010;
        const READABLE = 0b100;

        // The source may set any bits
        const _ = !0;
    }
}

/// Only specifies `WRITABLE` and `NO_EXECUTE` if needed. Other flags such as `PRESENT` and `USER_ACCESSIBLE` must be added.
/// https://en.wikipedia.org/wiki/Executable_and_Linkable_Format#Program_header
pub fn elf_flags_to_page_table_flags(elf_flags: u32) -> PageTableFlags {
    let elf_segment_flags = ElfSegmentFlags::from_bits_truncate(elf_flags);
    let mut page_table_flags = PageTableFlags::empty();
    if !elf_segment_flags.contains(ElfSegmentFlags::EXECUTABLE) {
        page_table_flags |= PageTableFlags::NO_EXECUTE;
    }
    if elf_segment_flags.contains(ElfSegmentFlags::WRITABLE) {
        page_table_flags |= PageTableFlags::WRITABLE;
    }
    page_table_flags
}
