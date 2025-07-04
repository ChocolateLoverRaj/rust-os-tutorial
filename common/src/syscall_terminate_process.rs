use core::num::NonZeroU32;

use crate::Syscall;

/// Terminate a process that's different than the calling process.
/// You must have permission to terminate the process.
/// The process might already be terminated / exited, in which case the output will be `false`.
/// If the process gets terminated by this syscall, then the output will be `true`.
pub struct SyscallTerminateProcess;
impl Syscall for SyscallTerminateProcess {
    const ID: u64 = 0x2E8AFBD64F738805;
    type Input = NonZeroU32;
    type Output = bool;
}
