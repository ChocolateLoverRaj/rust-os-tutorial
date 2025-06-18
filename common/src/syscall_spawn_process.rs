use bincode::{Decode, Encode};

use crate::Syscall;

#[derive(Debug, Encode, Decode)]
pub enum ProcessRelativePriority {
    Higher,
    Lower,
}

#[derive(Debug, Encode, Decode)]
pub struct SyscallSpawnProcessInput {
    priority: ProcessRelativePriority,
    rip: u64,
    rsp: u64,
}

pub struct SyscallSpawnProcess;
impl Syscall for SyscallSpawnProcess {
    const ID: u64 = 0x5B0B4092EAC9C9CE;
    type Input = SyscallSpawnProcessInput;
    type Output = ();
}
