use core::num::NonZeroU64;

use bincode::{Decode, Encode};
use thiserror::Error;

use crate::{SliceData, Syscall};

#[derive(Debug, Encode, Decode, Clone, Copy)]
pub enum AllocPageSize {
    _4KiB,
    _2MiB,
    _1GiB,
}

impl AllocPageSize {
    pub fn len(self) -> usize {
        match self {
            AllocPageSize::_4KiB => 0x1000,
            AllocPageSize::_2MiB => 512 * 0x1000,
            AllocPageSize::_1GiB => 512 * 512 * 0x1000,
        }
    }

    pub fn size_bytes(self) -> u64 {
        match self {
            AllocPageSize::_4KiB => 0x1000,
            AllocPageSize::_2MiB => 512 * 0x1000,
            AllocPageSize::_1GiB => 512 * 512 * 0x1000,
        }
    }
}

/// The virtual address cannot overlap with existing virtual addresses
#[derive(Debug, Encode, Decode)]
pub struct SyscallAllocInput {
    /// Exact size to allocate. Must be a multiple of the page size.
    pub len: NonZeroU64,
    pub page_size: AllocPageSize,
}

#[derive(Debug, Encode, Decode, Error)]
pub enum SyscallAllocError {
    #[error("This should not really ever happen")]
    OutOfVirtualMemory,
    #[error("Could not allocate because there is not enough memory available")]
    OutOfPhysicalMemory,
}

pub struct SyscallAlloc;
impl Syscall for SyscallAlloc {
    const ID: u64 = 0xD15A06B20965E4D9;
    type Input = SyscallAllocInput;
    /// The base pointer will never be 0 (NULL)
    type Output = Result<SliceData, SyscallAllocError>;
}
