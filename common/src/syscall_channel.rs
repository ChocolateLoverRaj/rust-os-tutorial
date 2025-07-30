use core::num::NonZero;

use bincode::{Decode, Encode};

use crate::Syscall;

#[derive(Debug, Encode, Decode)]
pub struct SyscallCreateChannelOutput {
    pub send_capability: NonZero<u64>,
    pub receive_capability: NonZero<u64>,
}

/// Creates a single producer single consumer channel which can produce multiple events.
/// Events don't contain data, they basically say "something happened".
pub struct SyscallCreateChannel;
impl Syscall for SyscallCreateChannel {
    const ID: u64 = 0xB03F1007BFCB1F53;
    type Input = ();
    type Output = NonZero<u64>;
}

/// Creates an event on the receiver, used to wake up receiving threads.
pub struct SyscallTxSend;
impl Syscall for SyscallTxSend {
    const ID: u64 = 0x846E569C392646AF;
    type Input = NonZero<u64>;
    type Output = ();
}
