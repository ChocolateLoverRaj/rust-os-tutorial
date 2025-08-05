use core::num::NonZero;

use bincode::{Decode, Encode};
use bitflags::bitflags;
use x86_64::structures::paging::PageTableFlags;
use zerocopy::{FromBytes, Immutable, KnownLayout, TryFromBytes};

use crate::{AllocPageSize, ElfSegmentFlags, Syscall, slice_data::SliceData2};

#[derive(Debug, Encode, Decode, TryFromBytes, Immutable, KnownLayout)]
#[repr(u8)]
pub enum SpawnProcessRelativePriority {
    Higher,
    Lower,
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct SpawnProcessMemoryFlags: u8 {
        const EXECUTABLE = 1 << 0;
        const WRITABLE = 1 << 1;
        const READABLE = 1 << 2;

        /// Use 2 MiB pages for this mapping (mapping must perfectly fit 2 MiB pages)
        const _2MiB_PAGE = 1 << 4;
        /// Use 1 GiB pages for this mapping (mapping must perfectly fit 1 GiB pages)
        const _1GiB_PAGE = 1 << 5;

        // /// Instead of un-mapping the mem from the current process, leave the current process's mem as it is
        // /// and also map it to the other process's mem.
        // /// Currently this is only used for mapped Limine modules.
        // const SHARE = 1 << 6;

        // The source may set any bits
        const _ = !0;
    }
}

impl SpawnProcessMemoryFlags {
    pub fn page_size(&self) -> AllocPageSize {
        if self.contains(SpawnProcessMemoryFlags::_1GiB_PAGE) {
            AllocPageSize::_1GiB
        } else if self.contains(SpawnProcessMemoryFlags::_2MiB_PAGE) {
            AllocPageSize::_2MiB
        } else {
            AllocPageSize::_4KiB
        }
    }
}

impl From<ElfSegmentFlags> for SpawnProcessMemoryFlags {
    fn from(value: ElfSegmentFlags) -> Self {
        let mut flags = SpawnProcessMemoryFlags::empty();
        if value.contains(ElfSegmentFlags::READABLE) {
            flags |= SpawnProcessMemoryFlags::READABLE;
        };
        if value.contains(ElfSegmentFlags::WRITABLE) {
            flags |= SpawnProcessMemoryFlags::WRITABLE;
        };
        if value.contains(ElfSegmentFlags::EXECUTABLE) {
            flags |= SpawnProcessMemoryFlags::EXECUTABLE;
        };
        flags
    }
}

impl From<SpawnProcessMemoryFlags> for PageTableFlags {
    fn from(value: SpawnProcessMemoryFlags) -> Self {
        let mut page_table_flags = Self::empty();
        if value.contains(SpawnProcessMemoryFlags::WRITABLE) {
            page_table_flags |= Self::WRITABLE;
        }
        if !value.contains(SpawnProcessMemoryFlags::EXECUTABLE) {
            page_table_flags |= Self::NO_EXECUTE;
        }
        page_table_flags
    }
}

#[derive(Debug, FromBytes, Immutable, KnownLayout, Clone)]
#[repr(C)]
pub struct SpawnProcessMemoryMapping {
    pub current_process_start: usize,
    pub new_process_start: usize,
    pub pages_len: usize,
    pub flags: u8,
}

#[derive(Debug, Clone, TryFromBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct MapModule {
    pub module_id: usize,
    pub start_page_offset: usize,
    /// Each page is 4 KiB
    pub pages_len: NonZero<usize>,
    pub new_process_start: usize,
    pub executable: bool,
}

/// Returns the process id of the new process
#[derive(Debug, TryFromBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct SyscallSpawnProcessInput {
    pub priority: SpawnProcessRelativePriority,
    pub rip: u64,
    pub rsp: u64,
    pub send_memory: SliceData2,
    pub map_modules: SliceData2,
    pub send_capabilities: SliceData2,
}

#[derive(Debug, Encode, Decode)]
pub enum SyscallSpawnProcessError {
    /// Invalid pointer to input
    InvalidInputPtr,
    /// Input is not valid, checked by `zerocopy`
    InvalidInput,
    InvalidMemoryMappingPtr,
    // Invalid memory mapping, checked by `zerocopy`
    InvalidMemoryMapping,
    InvalidSendMemSrcInterval,
    /// The src is a mix of different memory types. This is not allowed.
    SendMemSrcMix,
    /// The src only partially exists
    SendMemSrcPartial,
    /// You tried to send memory that is not plain memory
    SendMemNotPlain,
    InvalidCapabilityPtr,
    InvalidSendMemDestInterval,
    InvalidMapModulesPtr,
    /// There were overlapping regions for the new process's memory
    DestMemOverlap,
    /// Checked by `zerocopy`
    InvalidMapModule,
    /// Got a capability id of 0
    CapabilityIdZero,
    /// You tried to send a capability that you don't own or doesn't exist
    CapabilityNotFound,
    OutOfPhysMem,
    ModuleNotFound,
    OutOfModuleRange,
    InvalidModuleRange,
    InvalidModuleDest,
    ModuleUnalignedDest,
}

pub struct SyscallSpawnProcess;
impl Syscall for SyscallSpawnProcess {
    const ID: u64 = 0x5B0B4092EAC9C9CE;
    type Input = NonZero<usize>;
    type Output = Result<NonZero<u32>, SyscallSpawnProcessError>;
}
