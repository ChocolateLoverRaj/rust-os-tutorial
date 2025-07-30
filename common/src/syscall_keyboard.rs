use core::num::NonZero;

use bincode::{Decode, Encode};

use crate::Syscall;

#[derive(Debug, Encode, Decode)]
pub struct SyscallSubscribeToKeyboardInput {
    pub capability: NonZero<u64>,
    /// A ptr to a struct which is slots_len (`usize`), write_count (`AtomicUsize`), read_count (`AtomicUsize`), and slots (`[AtomicU8]`).
    /// User mode is responsible for initializing the read and write counts.
    pub queue_ptr: u64,
}

#[derive(Debug, Encode, Decode)]
pub enum SyscallSubscribeToKeyboardError {
    InvalidCapability,
}

/// Output is the event id.
pub struct SyscallSubscribeToKeyboard;
impl Syscall for SyscallSubscribeToKeyboard {
    const ID: u64 = 0x2EF02CFFF07EEBD0;
    type Input = SyscallSubscribeToKeyboardInput;
    type Output = Result<NonZero<u64>, SyscallSubscribeToKeyboardError>;
}
