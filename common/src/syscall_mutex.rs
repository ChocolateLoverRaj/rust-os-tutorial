use crate::Syscall;

pub struct SyscallFutexLock;
impl Syscall for SyscallFutexLock {
    const ID: u64 = 0xCC3FF32B184D545F;
    type Input = u64;
    type Output = ();
}

pub struct SyscallFutexUnlock;
impl Syscall for SyscallFutexUnlock {
    const ID: u64 = 0x858AB9720B65415E;
    type Input = u64;
    type Output = ();
}
