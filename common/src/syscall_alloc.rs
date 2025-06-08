use bincode::{Decode, Encode};
use thiserror::Error;

use crate::Syscall;

/// The virtual address cannot overlap with existing virtual addresses
#[derive(Debug, Encode, Decode)]
pub struct SyscallAllocInput {
    /// Must be 4 KiB aligned
    pub start: u64,
    /// Must be a multiple of 4 KiB
    pub len: u64,
}

#[derive(Debug, Encode, Decode, Error)]
pub enum SyscallAllocError {
    #[error("Could not allocate because there is not enough memory available")]
    OutOfMemory,
}

pub struct SyscallAlloc;
impl Syscall for SyscallAlloc {
    const ID: u64 = 0xAA0F96DAD3B78EDD;
    type Input = SyscallAllocInput;
    type Output = Result<(), SyscallAllocError>;
}
