use core::num::NonZero;

use bincode::{Decode, Encode};

use crate::{AllocPageSize, Syscall};

/// Allocates memory for multiple processes to use.
/// Returns the capability for mapping / accessing this shared mem.
pub struct SyscallNewSharedMem;

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
    type Output = Result<NonZero<u64>, SyscallNewShardMemError>;
}
