use core::num::NonZero;

use bincode::{Decode, Encode};

use crate::{SliceData, Syscall};

#[derive(Debug, Encode, Decode)]
pub struct SyscallWaitUntilEventInput {
    /// `&[u64]`. Should always have at least 1 item.
    events_to_wait_for: SliceData,
    events_that_happened: SliceData,
}

#[derive(Debug, Encode, Decode)]
pub enum SyscallWaitUntilEventError {
    /// 0 events were inputted
    Empty,
    InvalidEventsPtr,
    CapabilityZero,
    EventNotFound,
    /// You tried to use a capability as an event which cannot be used as an event
    InvalidCapability,
}

pub struct SyscallWaitUntilEvent;
impl Syscall for SyscallWaitUntilEvent {
    const ID: u64 = 0xCECBF60BD6839CA8;
    type Input = SliceData;
    type Output = Result<NonZero<usize>, SyscallWaitUntilEventError>;
}
