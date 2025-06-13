use crate::Syscall;

/// Returns the event stream id, which is also an event id
pub struct SyscallSubscribeToKeyboard;
impl Syscall for SyscallSubscribeToKeyboard {
    const ID: u64 = 0x2E86EF26DE7F979F;
    type Input = ();
    type Output = u64;
}
