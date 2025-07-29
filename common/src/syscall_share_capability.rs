use core::num::NonZero;

use bincode::{Decode, Encode};

use crate::Syscall;

/// Share a capability that you have with another process.
/// Outputs the capability for the other process.
/// Currently this is just used for shared memory.
pub struct SyscallShareCapability;

#[derive(Debug, Encode, Decode)]
pub struct SyscallShareCapabilityInput {
    pub capability: NonZero<u64>,
    pub process_id: NonZero<u32>,
}

impl Syscall for SyscallShareCapability {
    const ID: u64 = 0x6E5EAFB54F8EA4DE;
    type Input = SyscallShareCapabilityInput;
    type Output = NonZero<u64>;
}
