use core::num::NonZero;

use bincode::{Decode, Encode};

use crate::Syscall;

#[derive(Debug, Encode, Decode)]
pub enum SyscallCloneCapabilityError {
    /// The capability does not exist
    InvalidCapability,
    /// This capability cannot be cloned
    CannotClone,
}

/// Clones a capability so the current process can have it along with other processes at the same time.
/// Not all capabilities can be cloned.
/// Returns the new capability id.
pub struct SyscallCloneCapability;
impl Syscall for SyscallCloneCapability {
    const ID: u64 = 0x99DD4F805F0D6E91;
    type Input = NonZero<u64>;
    type Output = Result<NonZero<u64>, SyscallCloneCapabilityError>;
}
