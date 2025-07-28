use bincode::{Decode, Encode};
use bitflags::bitflags;

use crate::{AllocPageSize, Syscall};

/// Allocates memory for multiple processes to use.
/// Returns the capability for mapping / accessing this shared mem.
pub struct SyscallNewSharedMem;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct SyscallNewSharedMemFlags: u64 {
        const EXECUTABLE = 1 << 0;
        const WRITABLE = 1 << 1;
        const READABLE = 1 << 2;
        const _2MiB_PAGE = 1 << 3;
        const _1GiB_PAGE = 1 << 4;

        // The source may set any bits
        const _ = !0;
    }
}

#[derive(Debug, Encode, Decode)]
pub struct SyscallNewSharedMemInput {
    pub page_size: AllocPageSize,
    pub pages_len: usize,
}

#[derive(Debug, Encode, Decode)]
pub enum SyscallNewShardMemError {
    OutOfMem,
}

impl Syscall for SyscallNewSharedMem {
    const ID: u64 = 0x7BA4CCC1A32CE6DE;
    type Input = SyscallNewSharedMemInput;
    type Output = Result<u64, SyscallNewShardMemError>;
}
