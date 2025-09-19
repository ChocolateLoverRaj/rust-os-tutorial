#![no_std]

use core::{mem::MaybeUninit, num::NonZero};

use num_enum::{IntoPrimitive, TryFromPrimitive, TryFromPrimitiveError};

#[repr(u32)]
#[derive(Debug, IntoPrimitive, TryFromPrimitive)]
pub enum SyscallNumber {
    /// Data must be `0xCFD4BE13` to test that it's working
    HelloWorld,
    /// Data must be a pointer to a [`SyscallLog`]. If the pointer is invalid, [`SyscallError::InvalidInput`] will be returned.
    Log,
}

pub const HELLO_WORLD_MAGIC: usize = 0xCFD4BE13;

/// If there is no error, then the value will be `0`.
#[repr(u32)]
#[derive(Debug, IntoPrimitive, TryFromPrimitive)]
pub enum SyscallError {
    SyscallNoExist = 1,
    /// This means that the syscall-specific input was invalid
    /// The exact causes for this error depends on the syscall
    InvalidInput,
}

pub fn encode_syscall_output(value: Result<(), SyscallError>) -> u32 {
    match value {
        Ok(_) => 0,
        Err(error) => error.into(),
    }
}

pub fn decode_syscall_output(
    value: u32,
) -> Result<Result<(), SyscallError>, TryFromPrimitiveError<SyscallError>> {
    match NonZero::new(value) {
        None => Ok(Ok(())),
        Some(error) => Ok(Err(error.get().try_into()?)),
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct Slice {
    pub addr: usize,
    pub len: usize,
}

#[derive(Debug)]
#[repr(C)]
pub struct SyscallLog {
    pub slice: Slice,
    pub output: MaybeUninit<SyscallLogOutput>,
}

#[derive(Debug)]
#[repr(usize)]
pub enum SyscallLogOutput {
    Ok,
    /// Reading the string caused a page fault (just panic if this happens)
    InvalidSlice,
    /// The string was not valid UTF-8
    InvalidUtf8,
}
