use core::num::NonZero;

use bincode::{Decode, Encode};

use crate::Syscall;

pub struct SyscallGetThreadId;

#[derive(Debug, Encode, Decode)]
pub struct SyscallGetThreadIdOutput {
    pub thread_id: NonZero<u32>,
    pub process_id: NonZero<u32>,
}

impl Syscall for SyscallGetThreadId {
    const ID: u64 = 0xFE67BB557A17199E;
    type Input = ();
    type Output = SyscallGetThreadIdOutput;
}
