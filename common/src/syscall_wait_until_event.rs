use bincode::{Decode, Encode};

use crate::{SliceData, Syscall};

#[derive(Debug, Encode, Decode)]
pub struct SyscallWaitUntilEventInput {
    /// `&[u64]`. Should always have at least 1 item.
    events_to_wait_for: SliceData,
    events_that_happened: SliceData,
}

pub struct SyscallWaitUntilEvent;
impl Syscall for SyscallWaitUntilEvent {
    const ID: u64 = 0xCECBF60BD6839CA8;
    type Input = SliceData;
    type Output = u64;
}
