use core::num::NonZero;

use bincode::{Decode, Encode};

use crate::Syscall;

/// Returns a ptr to a struct which is write_count (`AtomicUsize`), read_count (`AtomicUsize`), and slots (`[AtomicU8]`).
/// User mode is responsible for initializing the read and write counts.
#[derive(Debug, Encode, Decode)]
pub struct SyscallSubscribeToKeyboardInput {
    pub capability: NonZero<u64>,
    pub slots_len: NonZero<usize>,
}

#[derive(Debug, Encode, Decode)]
pub struct SyscallSubscribeToKeyboardOutput {
    pub event: NonZero<u64>,
    pub addr: NonZero<usize>,
    pub slots_len: NonZero<usize>,
}

#[derive(Debug, Encode, Decode)]
pub enum SyscallSubscribeToKeyboardError {
    CapabilityNotFound,
    InvalidCapability,
    InvalidQueuePtr,
    OutOfVirtMem,
    OutOfKernelVirtMem,
    OutOfPhysMem,
    InvalidSlotsLen,
}

/// Output is the event id.
pub struct SyscallSubscribeToKeyboard;
impl Syscall for SyscallSubscribeToKeyboard {
    const ID: u64 = 0x2EF02CFFF07EEBD0;
    type Input = SyscallSubscribeToKeyboardInput;
    type Output = Result<SyscallSubscribeToKeyboardOutput, SyscallSubscribeToKeyboardError>;
}
