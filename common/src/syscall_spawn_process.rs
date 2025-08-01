use core::num::NonZero;

use bincode::{Decode, Encode};
use bitflags::bitflags;
use x86_64::structures::paging::PageTableFlags;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, TryFromBytes};

use crate::{ElfSegmentFlags, SliceData, Syscall};

#[derive(Debug, Encode, Decode, TryFromBytes, Immutable)]
#[repr(u8)]
pub enum SpawnProcessRelativePriority {
    Higher,
    Lower,
}

/// Returns the process id of the new process
#[derive(Debug, Encode, Decode, TryFromBytes, Immutable, KnownLayout)]
pub struct SyscallSpawnProcessInput {
    pub priority: SpawnProcessRelativePriority,
    pub rip: u64,
    pub rsp: u64,
    pub memory_mappings: SliceData,
    pub send_capabilities: SliceData,
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct SpawnProcessMemoryFlags: u64 {
        const EXECUTABLE = 1 << 0;
        const WRITABLE = 1 << 1;
        const READABLE = 1 << 2;

        /// Use 2 MiB pages for this mapping (mapping must perfectly fit 2 MiB pages)
        const _2MiB_PAGE = 1 << 4;
        /// Use 1 GiB pages for this mapping (mapping must perfectly fit 1 GiB pages)
        const _1GiB_PAGE = 1 << 5;

        // The source may set any bits
        const _ = !0;
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

#[derive(Debug, FromBytes, IntoBytes, Immutable, KnownLayout, Clone)]
pub struct SpawnProcessMemoryMapping {
    pub current_process_start: u64,
    pub new_process_start: u64,
    pub len: u64,
    pub flags: u64,
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
    InvalidMemoryMappingSrc,
    InvalidCapabilityPtr,
    /// Got a capability id of 0
    InvalidCapabilityId,
    /// You tried to send a capability that you don't own or doesn't exist
    CapabilityNotFound,
    OutOfPhysMem,
}

pub struct SyscallSpawnProcess;
impl Syscall for SyscallSpawnProcess {
    const ID: u64 = 0x5B0B4092EAC9C9CE;
    type Input = u64;
    type Output = Result<NonZero<u32>, SyscallSpawnProcessError>;
}
