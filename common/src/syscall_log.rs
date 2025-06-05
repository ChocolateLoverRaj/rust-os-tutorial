use bincode::{Decode, Encode};
pub use log;
use thiserror::Error;

use crate::{Syscall, slice_data::SliceData};

#[derive(Debug, Encode, Decode)]
pub struct SyscallLogInput {
    pub level: log::Level,
    pub message: SliceData,
}

#[derive(Debug, Error, Encode, Decode)]
pub enum SyscallLogError {
    #[error("The string is not valid UTF-8")]
    InvalidString,
}

pub struct SyscallLog;
impl Syscall for SyscallLog {
    const ID: u64 = 0x78F01CBF79E9479A;
    type Input = SyscallLogInput;
    type Output = Result<(), SyscallLogError>;
}
