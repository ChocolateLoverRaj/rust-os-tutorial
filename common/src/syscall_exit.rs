use crate::Syscall;

pub struct SyscallExitProcess;
impl Syscall for SyscallExitProcess {
    const ID: u64 = 0xE2AAD19FB543D33A;
    type Input = ();
    /// In reality you will never get an output
    type Output = ();
}
