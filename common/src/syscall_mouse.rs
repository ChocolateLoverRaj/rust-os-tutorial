use bincode::{Decode, Encode};
use thiserror::Error;

use crate::Syscall;

#[derive(Debug, Error, Encode, Decode)]
pub enum SyscallSubscribeToMouseError {
    #[error("There is no PS/2 mouse available")]
    NoPs2Mouse,
}

/// Returns the event stream id, which is also the event id
pub struct SyscallSubscribeToMouse;
impl Syscall for SyscallSubscribeToMouse {
    const ID: u64 = 0x8F6EF13ECC118443;
    type Input = ();
    type Output = Result<u64, SyscallSubscribeToMouseError>;
}
