use bincode::{Decode, Encode};

use crate::{SliceData, Syscall};

#[derive(Debug, Encode, Decode)]
pub struct SyscallReadEventStreamInput {
    pub buffer: SliceData,
    pub stream_id: u64,
}

/// Returns the number of bytes copied into the buffer
pub struct SyscallReadEventStream;
impl Syscall for SyscallReadEventStream {
    const ID: u64 = 0x4C96570E34D779B5;
    type Input = SyscallReadEventStreamInput;
    type Output = u64;
}
