use bincode::{Decode, Encode};

use crate::{SliceData, Syscall};

/// Returns the event stream id, which is also an event id
pub struct SyscallSubscribeToKeyboard;
impl Syscall for SyscallSubscribeToKeyboard {
    const ID: u64 = 0x2E86EF26DE7F979F;
    type Input = ();
    type Output = u64;
}

// #[derive(Debug, Encode, Decode)]
// pub struct SyscallSubscribeToKeybaord2Input {
//     pub read_count: usize,
//     pub write_count: usize,
//     pub slots: SliceData,
// }

/// The input is a ptr to a struct which is slots_len (usize), write_count (AtomicUsize), read_count (AtomicUsize), and slots ([AtomicU8]).
/// User mode is responsible for initializing the read and write counts.
/// Output is the event id.
pub struct SyscallSubscribeToKeyboard2;
impl Syscall for SyscallSubscribeToKeyboard2 {
    const ID: u64 = 0x57DB05EF0ABEA474;
    type Input = u64;
    type Output = u64;
}
