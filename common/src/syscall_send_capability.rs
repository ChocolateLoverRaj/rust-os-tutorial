use core::num::NonZero;

use bincode::{Decode, Encode};

use crate::Syscall;

/// Transfer ownership of a capability to another process.
/// The capability id does not change.
/// The new process then owns the capability.
/// The current process can no longer use the capability.
/// You may want to clone the capability first before sending if you want multiple processes to have it.
pub struct SyscallSendCapability;

#[derive(Debug, Encode, Decode)]
pub struct SyscallSendCapabilityInput {
    pub capability: NonZero<u64>,
    pub process_id: NonZero<u32>,
}

#[derive(Debug, Encode, Decode)]
pub enum SyscallSendCapabilityError {
    /// The capability does not exist
    InvalidCapability,
    /// This capability cannot be sent to another process
    CannotSend,
}

impl Syscall for SyscallSendCapability {
    const ID: u64 = 0x6E5EAFB54F8EA4DE;
    type Input = SyscallSendCapabilityInput;
    type Output = Result<(), SyscallSendCapabilityError>;
}
