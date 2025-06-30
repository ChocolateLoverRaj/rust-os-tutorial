use bincode::{Decode, Encode};
use bitflags::bitflags;
use x86_64::structures::paging::PageTableFlags;
use zerocopy::{FromBytes, Immutable, IntoBytes};

use crate::{ElfSegmentFlags, SliceData, Syscall};

#[derive(Debug, Encode, Decode)]
pub enum ProcessRelativePriority {
    Higher,
    Lower,
}

#[derive(Debug, Encode, Decode)]
pub struct SyscallSpawnProcessInput {
    pub priority: ProcessRelativePriority,
    pub rip: u64,
    pub rsp: u64,
    pub memory_mappings: SliceData,
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct PagePermissions: u64 {
        const EXECUTABLE = 1 << 0;
        const WRITABLE = 1 << 1;
        const READABLE = 1 << 2;

        // The source may set any bits
        const _ = !0;
    }
}

impl From<ElfSegmentFlags> for PagePermissions {
    fn from(value: ElfSegmentFlags) -> Self {
        let mut flags = PagePermissions::empty();
        if value.contains(ElfSegmentFlags::READABLE) {
            flags |= PagePermissions::READABLE;
        };
        if value.contains(ElfSegmentFlags::WRITABLE) {
            flags |= PagePermissions::WRITABLE;
        };
        if value.contains(ElfSegmentFlags::EXECUTABLE) {
            flags |= PagePermissions::EXECUTABLE;
        };
        flags
    }
}

impl From<PagePermissions> for PageTableFlags {
    fn from(value: PagePermissions) -> Self {
        let mut page_table_flags = Self::empty();
        if value.contains(PagePermissions::WRITABLE) {
            page_table_flags |= Self::WRITABLE;
        }
        if !value.contains(PagePermissions::EXECUTABLE) {
            page_table_flags |= Self::NO_EXECUTE;
        }
        page_table_flags
    }
}

#[derive(Debug, FromBytes, IntoBytes, Immutable, Clone)]
pub struct SpawnProcessMemoryMapping {
    pub current_process_start: u64,
    pub new_process_start: u64,
    pub len: u64,
    pub permissions: u64,
}

pub struct SyscallSpawnProcess;
impl Syscall for SyscallSpawnProcess {
    const ID: u64 = 0x5B0B4092EAC9C9CE;
    type Input = SyscallSpawnProcessInput;
    type Output = ();
}
