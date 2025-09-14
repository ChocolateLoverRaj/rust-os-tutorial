#![no_std]

use core::num::NonZero;

use num_enum::{IntoPrimitive, TryFromPrimitive, TryFromPrimitiveError};

#[repr(u32)]
#[derive(Debug, IntoPrimitive, TryFromPrimitive)]
pub enum Syscall {
    // Data must be `0xCFD4BE13` to test that it's working
    HelloWorld,
}

pub const HELLO_WORLD_MAGIC: usize = 0xCFD4BE13;

/// If there is no error, then the value will be `0`.
#[repr(u32)]
#[derive(Debug, IntoPrimitive, TryFromPrimitive)]
pub enum SyscallError {
    SyscallNoExist = 1,
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
