use bincode::{Decode, Encode};
use thiserror::Error;

use crate::Syscall;

pub const FUTEX_WAITERS: u64 = 1 << 63;

#[derive(Debug, Error, Encode, Decode)]
pub enum FutexLockError {
    #[error("The value is just FUTEX_WAITERS, try exchange again")]
    CheckWithWaiters,
    #[error("The thread id for the lock owner does not reference a valid thread")]
    UnknownLockOwner,
}

pub struct SyscallFutexLock;
impl Syscall for SyscallFutexLock {
    const ID: u64 = 0xCC3FF32B184D545F;
    type Input = u64;
    type Output = Result<(), FutexLockError>;
}

pub struct SyscallFutexUnlock;
impl Syscall for SyscallFutexUnlock {
    const ID: u64 = 0x858AB9720B65415E;
    type Input = u64;
    type Output = ();
}
