use crate::Syscall;

pub struct SyscallExit;
impl Syscall for SyscallExit {
    const ID: u64 = 0xEA48EA588EACE1D7;
    type Input = ();
    /// In reality you will never get an output
    type Output = ();
}
