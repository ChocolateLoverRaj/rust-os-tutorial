use bincode::{Decode, Encode};
use thiserror::Error;

use crate::{SliceData, Syscall};

#[derive(Debug, Encode, Decode)]
pub struct SyscallAllocStackInput {
    /// Minimum size to allocate. Kernel will allocate at least this amount.
    pub len: u64,
}

#[derive(Debug, Encode, Decode, Error)]
pub enum SyscallAllocStackError {
    #[error("This should not really ever happen")]
    OutOfVirtualMemory,
    #[error("Could not allocate because there is not enough memory available")]
    OutOfPhysicalMemory,
}

#[derive(Debug, Encode, Decode)]
pub struct SyscallAllocStackOutput {
    /// Contains the usable part of the allocated stack. Does not contain the guard page.
    pub usable_stack: SliceData,
}

pub struct SyscallAllocStack;
impl Syscall for SyscallAllocStack {
    const ID: u64 = 0xAFEA8B1D744E0EA6;
    type Input = SyscallAllocStackInput;
    type Output = Result<SyscallAllocStackOutput, SyscallAllocStackError>;
}
