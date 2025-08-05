use core::num::NonZero;

use bincode::{Decode, Encode};

use crate::{PageSize, Syscall};

#[derive(Debug, Encode, Decode)]
pub struct SyscallMemProtInput {
    /// This is the address / page size
    pub start_page_index: NonZero<usize>,
    pub page_size: PageSize,
    pub pages_len: NonZero<usize>,
    pub new_prot: u8,
}

#[derive(Debug, Encode, Decode)]
pub enum SyscallMemProtError {
    InvalidInterval,
    NotPlain,
    OutOfPhysMem,
}

/// Change the permissions of plain allocated memory for the calling process.
pub struct SyscallMemProt;
impl Syscall for SyscallMemProt {
    const ID: u64 = 0x4F9A116FA3AD07B9;
    type Input = SyscallMemProtInput;
    type Output = Result<(), SyscallMemProtError>;
}
