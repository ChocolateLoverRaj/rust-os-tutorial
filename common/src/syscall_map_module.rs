use bincode::{Decode, Encode};
use thiserror::Error;

use crate::{SliceData, Syscall};

/// Maps a module from Limine (as read-only) to memory accessible by the caller
pub struct SyscallMapModule;

#[derive(Debug, Error, Encode, Decode)]
pub enum SyscallMapModuleError {
    #[error("No such module is present")]
    NotPresent,
    #[error("This should not really ever happen")]
    OutOfVirtualMemory,
    #[error("Could not allocate because there is not enough memory available")]
    OutOfPhysicalMemory,
}

impl Syscall for SyscallMapModule {
    const ID: u64 = 0x908414050BF8B9D2;
    type Input = u64;
    type Output = Result<SliceData, SyscallMapModuleError>;
}
