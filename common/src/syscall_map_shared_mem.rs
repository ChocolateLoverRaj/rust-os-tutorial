use core::num::NonZero;

use bincode::{Decode, Encode};
use bitflags::bitflags;

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
