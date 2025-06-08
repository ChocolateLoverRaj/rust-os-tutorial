use core::{alloc::Layout, num::NonZeroU64};

use bincode::{Decode, Encode};
use thiserror::Error;

use crate::{SliceData, Syscall};

/// The virtual address cannot overlap with existing virtual addresses
#[derive(Debug, Encode, Decode)]
pub struct SyscallAlloc2Input {
    /// Minimum size to allocate. Kernel will allocate at least this amount.
    pub len: NonZeroU64,
    /// Must be a power of 2
    pub align: NonZeroU64,
}

impl From<Layout> for SyscallAlloc2Input {
    fn from(value: Layout) -> Self {
        Self {
            len: (value.size() as u64).try_into().unwrap(),
            align: (value.align() as u64).try_into().unwrap(),
        }
    }
}

#[derive(Debug, Encode, Decode, Error)]
pub enum SyscallAlloc2Error {
    #[error("This should not really ever happen")]
    OutOfVirtualMemory,
    #[error("Could not allocate because there is not enough memory available")]
    OutOfPhysicalMemory,
}

pub struct SyscallAlloc2;
impl Syscall for SyscallAlloc2 {
    const ID: u64 = 0xFC06AA71B5F462A5;
    type Input = SyscallAlloc2Input;
    /// The base pointer will never be 0 (NULL)
    type Output = Result<SliceData, SyscallAlloc2Error>;
}
