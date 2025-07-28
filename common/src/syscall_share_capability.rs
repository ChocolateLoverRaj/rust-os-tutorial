use crate::Syscall;

/// Share a capability that you have with another process.
/// Outputs the capability for the other process.
/// Currently this is just used for shared memory.
pub struct SyscallShareCapability;

impl Syscall for SyscallShareCapability {
    const ID: u64 = 0x6E5EAFB54F8EA4DE;
    type Input = u64;
    type Output = u64;
}
