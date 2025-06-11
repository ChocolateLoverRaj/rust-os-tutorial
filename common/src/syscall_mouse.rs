use bincode::{Decode, Encode};
use thiserror::Error;

use crate::{SliceData, Syscall};

#[derive(Debug, Error, Encode, Decode)]
pub enum SyscallSubscribeToMouseError {
    #[error("The PS/2 mouse did not respond, indicating that there is no PS/2 mouse available")]
    NoPs2Mouse,
}

pub struct SyscallSubscribeToMouse;
impl Syscall for SyscallSubscribeToMouse {
    const ID: u64 = 0xE8A02E04B1B38B82;
    type Input = ();
    type Output = Result<u64, SyscallSubscribeToMouseError>;
}

pub struct SyscallReadMouse;
impl Syscall for SyscallReadMouse {
    const ID: u64 = 0x6EFDED43FFBE642E;
    type Input = SliceData;
    type Output = u64;
}
