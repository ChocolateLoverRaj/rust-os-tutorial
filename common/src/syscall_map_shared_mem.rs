use core::num::NonZero;

use bincode::{Decode, Encode};
use bitflags::bitflags;
use x86_64::structures::paging::PageTableFlags;

use crate::{SliceData, Syscall};

/// Outputs the start address
pub struct SyscallMapSharedMem;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct PermissionFlags: u8 {
        const EXECUTABLE = 1 << 0;
        const WRITABLE = 1 << 1;
        const READABLE = 1 << 2;
        // The source may set any bits
        const _ = !0;
    }
}

impl PermissionFlags {
    pub fn page_table_flags(&self) -> PageTableFlags {
        let mut flags = PageTableFlags::empty();
        if self.contains(PermissionFlags::READABLE) {
            flags |= PageTableFlags::PRESENT;
        }
        if self.contains(PermissionFlags::WRITABLE) {
            flags |= PageTableFlags::WRITABLE;
        }
        if !self.contains(PermissionFlags::EXECUTABLE) {
            flags |= PageTableFlags::NO_EXECUTE;
        }
        flags
    }
}

#[derive(Debug, Encode, Decode)]
pub struct SyscallMapSharedMemInput {
    pub capability: NonZero<u64>,
    pub permission_flags: u8,
}

#[derive(Debug, Encode, Decode)]
pub enum SyscallMapSharedMemError {
    NoVirtMem,
}

impl Syscall for SyscallMapSharedMem {
    const ID: u64 = 0x064FFF56E860AFF4;

    type Input = SyscallMapSharedMemInput;
    type Output = Result<SliceData, SyscallMapSharedMemError>;
}
