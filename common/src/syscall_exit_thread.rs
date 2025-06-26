use crate::Syscall;

/// Never returns, stops executing code on the calling thread. Does not stop other threads in the process.
pub struct SyscallExitThread;

impl Syscall for SyscallExitThread {
    const ID: u64 = 0x3B14A4214BBF38A8;
    type Input = ();
    type Output = ();
}
