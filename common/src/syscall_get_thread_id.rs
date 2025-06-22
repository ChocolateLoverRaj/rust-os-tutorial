use core::num::NonZeroU32;

use crate::Syscall;

pub struct SyscallGetThreadId;
impl Syscall for SyscallGetThreadId {
    const ID: u64 = 0xC6EBCC6BF0C98B00;
    type Input = ();
    type Output = NonZeroU32;
}
