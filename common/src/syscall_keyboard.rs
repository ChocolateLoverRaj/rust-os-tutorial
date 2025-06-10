use crate::{SliceData, Syscall};

/// Returns the event id
pub struct SyscallSubscribeToKeyboard;
impl Syscall for SyscallSubscribeToKeyboard {
    const ID: u64 = 0xC42902F7144895C9;
    type Input = ();
    type Output = u64;
}

pub struct SyscallReadKeyboard;
impl Syscall for SyscallReadKeyboard {
    const ID: u64 = 0x7170D08BCE495082;
    type Input = SliceData;
    type Output = u64;
}
