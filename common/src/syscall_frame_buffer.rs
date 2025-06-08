use bincode::{Decode, Encode};
use thiserror::Error;

use crate::{FrameBufferInfo, Syscall};

#[derive(Debug, Encode, Decode)]
pub struct SyscallTakeFrameBufferOutput {
    pub ptr: u64,
    pub info: FrameBufferInfo,
}

#[derive(Debug, Encode, Decode, Error)]
pub enum SyscallTakeFrameBufferError {
    #[error("A program (this program or different one) is already using the frame buffer")]
    InUse,
    #[error("There is no screen available")]
    NotAvailable,
    #[error(
        "This error will happen if the frame buffer is not contained in its own 4KiB physical frames. 
        In this case, the kernel cannot give user mode access to the entire frame buffer because that would
        also give user space access to other physical memory which could be MMIO."
    )]
    WouldNotBeSecure,
    #[error("This should never happen")]
    OutOfVirtualMemory,
    #[error("Out of physical memory while trying to map page tables")]
    OutOfPhysicalMemory,
}

pub struct SyscallTakeFrameBuffer;
impl Syscall for SyscallTakeFrameBuffer {
    const ID: u64 = 0xA0A349B1D4505FD7;
    type Input = ();
    type Output = Result<SyscallTakeFrameBufferOutput, SyscallTakeFrameBufferError>;
}

/// Your process will be terminated if you try to release the frame buffer and you don't own it currently
pub struct SyscallReleaseFrameBuffer;
impl Syscall for SyscallReleaseFrameBuffer {
    const ID: u64 = 0xBCBCEA5D5EECCDC1;
    type Input = ();
    type Output = ();
}
