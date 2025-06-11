use core::{alloc::Layout, arch::asm, mem::MaybeUninit};

use common::{
    Syscall, SyscallAlloc, SyscallAllocError, SyscallExists, SyscallExit, SyscallLog,
    SyscallLogInput, SyscallReadKeyboard, SyscallReadMouse, SyscallReleaseFrameBuffer,
    SyscallSubscribeToKeyboard, SyscallSubscribeToMouse, SyscallTakeFrameBuffer,
    SyscallTakeFrameBufferError, SyscallTakeFrameBufferOutput, SyscallWaitUntilEvent, log,
};

/// # Safety
/// The input must be valid. Invalid inputs can lead to undefined behavior or the program being terminated.
unsafe fn raw_syscall(input_and_ouput: &mut [u64; 7]) {
    unsafe {
        asm!(
            "syscall",
            inlateout("rdi") input_and_ouput[0],
            inlateout("rsi") input_and_ouput[1],
            inlateout("rdx") input_and_ouput[2],
            inlateout("r10") input_and_ouput[3],
            inlateout("r8") input_and_ouput[4],
            inlateout("r9") input_and_ouput[5],
            inlateout("rax") input_and_ouput[6],
        );
    }
}

/// # Safety
/// Input must be valid, and the kernel should support the syscall
unsafe fn syscall<T: Syscall>(input: &T::Input) -> T::Output {
    let mut inputs_and_ouputs = T::encode_input(input);
    unsafe { raw_syscall(&mut inputs_and_ouputs) };
    T::decode_output(&inputs_and_ouputs)
}

pub fn syscall_exists(syscall_id: u64) -> bool {
    // Safety: there is nothing that can go wrong with this syscall
    unsafe { syscall::<SyscallExists>(&syscall_id) }
}

pub fn syscall_exit() -> ! {
    // Safety: input is valid
    unsafe { syscall::<SyscallExit>(&()) };
    unreachable!()
}

pub fn syscall_log(level: log::Level, message: &str) {
    unsafe {
        syscall::<SyscallLog>(&SyscallLogInput {
            level,
            message: message.as_bytes().into(),
        })
    }
    // &str means it should be valid
    .unwrap()
}

pub fn syscall_alloc(layout: Layout) -> Result<*mut [u8], SyscallAllocError> {
    let input = layout.into();
    let slice = unsafe { syscall::<SyscallAlloc>(&input) }?;
    Ok(unsafe { slice.to_slice_mut() })
}

pub fn syscall_take_frame_buffer()
-> Result<SyscallTakeFrameBufferOutput, SyscallTakeFrameBufferError> {
    // Safety: input is correct
    unsafe { syscall::<SyscallTakeFrameBuffer>(&()) }
}

pub fn syscall_release_frame_buffer() {
    // Safety: input is correct
    unsafe { syscall::<SyscallReleaseFrameBuffer>(&()) }
}

pub fn syscall_subscribe_to_keyboard() -> u64 {
    // Safety: input is correct
    unsafe { syscall::<SyscallSubscribeToKeyboard>(&()) }
}

pub fn syscall_read_keyboard(buffer: &mut [MaybeUninit<u8>]) -> &mut [u8] {
    let input = buffer.into();
    // Safety: The input is valid
    let count = unsafe { syscall::<SyscallReadKeyboard>(&input) };
    // Safety: the kernel initialized them
    unsafe { buffer[..count as usize].assume_init_mut() }
}

pub fn syscall_wait_until_event(events: &mut [u64]) -> &mut [u64] {
    let input = events.into();
    // Safety: The input is valid
    let count = unsafe { syscall::<SyscallWaitUntilEvent>(&input) };
    &mut events[..count as usize]
}

pub fn syscall_subscribe_to_mouse() -> Result<u64, common::SyscallSubscribeToMouseError> {
    // Safety: input is correct
    unsafe { syscall::<SyscallSubscribeToMouse>(&()) }
}

pub fn syscall_read_mouse(buffer: &mut [MaybeUninit<u8>]) -> &mut [u8] {
    let input = buffer.into();
    // Safety: The input is valid
    let count = unsafe { syscall::<SyscallReadMouse>(&input) };
    // Safety: the kernel initialized them
    unsafe { buffer[..count as usize].assume_init_mut() }
}
