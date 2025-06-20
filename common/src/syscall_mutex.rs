use crate::Syscall;

/// Attempts to acquire this mutex without blocking. Returns `true`
/// if the lock was successfully acquired and `false` otherwise.
pub struct SyscallTryAquireLock;
impl Syscall for SyscallTryAquireLock {
    const ID: u64 = 0xAEC8487CB8CB65F9;
    type Input = u64;
    type Output = bool;
}

pub struct SyscallAquireLock;
impl Syscall for SyscallAquireLock {
    const ID: u64 = 0x682560F987F7F2C2;
    type Input = u64;
    type Output = ();
}

pub struct SyscallReleaseLock;
impl Syscall for SyscallReleaseLock {
    const ID: u64 = 0x48D2AA22D39E6EF9;
    type Input = u64;
    type Output = ();
}
