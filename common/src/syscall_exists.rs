use crate::Syscall;

pub struct SyscallExists;
impl Syscall for SyscallExists {
    const ID: u64 = 0x5AA6D6E077F56B01;
    type Input = u64;
    type Output = bool;
}
