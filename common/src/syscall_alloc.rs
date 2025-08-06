use core::num::{NonZero, NonZeroUsize};

use bincode::{Decode, Encode};

use crate::Syscall;

#[derive(Debug, Encode, Decode, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PageSize {
    _4KiB,
    _2MiB,
    _1GiB,
}

impl PageSize {
    pub const fn byte_len(self) -> usize {
        self.byte_len_u64() as usize
    }

    pub const fn byte_len_u64(self) -> u64 {
        match self {
            PageSize::_4KiB => 0x1000,
            PageSize::_2MiB => 512 * 0x1000,
            PageSize::_1GiB => 512 * 512 * 0x1000,
        }
    }
}

/// The virtual address cannot overlap with existing virtual addresses
#[derive(Debug, Encode, Decode)]
pub struct SyscallAllocInput {
    pub pages_len: NonZeroUsize,
    pub page_size: PageSize,
    /// If this is `true`, then the allocated pages will be zeroed. If not, then they might not be zeroed.
    pub zero: bool,
    pub mem_prot: u8,
}

#[derive(Debug, Encode, Decode)]
pub enum SyscallAllocError {
    /// This should not really ever happen
    OutOfVirtualMemory,
    /// Could not allocate because there is not enough memory available
    OutOfPhysicalMemory,
    /// The CPU does not support this page size. You should've checked yourself with cpuid.
    PageSizeNotSupported,
}

pub struct SyscallAlloc;
impl Syscall for SyscallAlloc {
    const ID: u64 = 0xD15A06B20965E4D9;
    type Input = SyscallAllocInput;
    /// The base pointer will never be 0 (NULL)
    type Output = Result<NonZero<usize>, SyscallAllocError>;
}
