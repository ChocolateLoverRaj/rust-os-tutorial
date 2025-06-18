use bincode::{Decode, Encode};

use crate::Syscall;

#[derive(Debug, Encode, Decode)]
pub enum ThreadRelativePriority {
    Higher,
    Lower,
}

#[derive(Debug, Encode, Decode)]
pub struct SyscallSpawnThreadInput {
    pub priority: ThreadRelativePriority,
    pub rip: u64,
    pub rsp: u64,
}

pub struct SyscallSpawnThread;
impl Syscall for SyscallSpawnThread {
    const ID: u64 = 0x55AC7F20398547E3;
    type Input = SyscallSpawnThreadInput;
    type Output = ();
}
