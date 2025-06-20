use bincode::{Decode, Encode};

use crate::Syscall;

#[derive(Debug, Encode, Decode)]
pub enum SpawnThreadRelativePriority {
    Higher,
    Lower,
}

#[derive(Debug, Encode, Decode)]
pub struct SyscallSpawnThreadInput {
    pub priority: SpawnThreadRelativePriority,
    pub rip: u64,
    pub rsp: u64,
}

pub struct SyscallSpawnThread;
impl Syscall for SyscallSpawnThread {
    const ID: u64 = 0x55AC7F20398547E3;
    type Input = SyscallSpawnThreadInput;
    type Output = ();
}
