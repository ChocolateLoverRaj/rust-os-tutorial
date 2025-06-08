use core::ops::RangeInclusive;

use bincode::{Decode, Encode};

use crate::Syscall;

#[derive(Debug, Encode, Decode, Clone)]
pub struct SyscallMemInfoOutput {
    pub elf: RangeInclusive<u64>,
    pub stack: RangeInclusive<u64>,
}

pub struct SyscallMemInfo;
impl Syscall for SyscallMemInfo {
    const ID: u64 = 0x6E49D4B77B893C67;
    type Input = ();
    type Output = SyscallMemInfoOutput;
}
