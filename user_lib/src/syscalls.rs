use core::{alloc::Layout, arch::asm, mem::MaybeUninit, num::NonZeroU32};

use common::{
    SpawnProcessMemoryMapping, SpawnProcessRelativePriority, SpawnThreadRelativePriority, Syscall,
    SyscallAlloc, SyscallAllocError, SyscallAllocStack, SyscallAllocStackError,
    SyscallAllocStackInput, SyscallAllocStackOutput, SyscallCreateChannel, SyscallExists,
    SyscallExitProcess, SyscallExitThread, SyscallGetThreadId, SyscallLog, SyscallLogInput,
    SyscallMapModule, SyscallMapModuleError, SyscallReadEventStream, SyscallReadEventStreamInput,
    SyscallReleaseFrameBuffer, SyscallSpawnProcess, SyscallSpawnProcessInput, SyscallSpawnThread,
    SyscallSpawnThreadInput, SyscallSubscribeToKeyboard, SyscallSubscribeToMouse,
    SyscallTakeFrameBuffer, SyscallTakeFrameBufferError, SyscallTakeFrameBufferOutput,
    SyscallTxSend, SyscallWaitUntilEvent, log,
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
            lateout("rcx") _,
            lateout("r11") _,
        );
    }
}

/// # Safety
/// Input must be valid, and the kernel should support the syscall
pub unsafe fn syscall<T: Syscall>(input: &T::Input) -> T::Output {
    let mut inputs_and_ouputs = T::encode_input(input);
    unsafe { raw_syscall(&mut inputs_and_ouputs) };
    T::decode_output(&inputs_and_ouputs)
}

pub fn syscall_exists(syscall_id: u64) -> bool {
    // Safety: there is nothing that can go wrong with this syscall
    unsafe { syscall::<SyscallExists>(&syscall_id) }
}

pub fn syscall_exit_process() -> ! {
    // Safety: input is valid
    unsafe { syscall::<SyscallExitProcess>(&()) };
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

pub fn syscall_read_event_stream(stream_id: u64, buffer: &mut [MaybeUninit<u8>]) -> &mut [u8] {
    let input = SyscallReadEventStreamInput {
        stream_id,
        buffer: buffer.into(),
    };
    // Safety: The input is valid
    let count = unsafe { syscall::<SyscallReadEventStream>(&input) };
    // Safety: the kernel initialized them
    unsafe { buffer[..count as usize].assume_init_mut() }
}

/// # Safety
/// The stack pointer must be valid. Passing a bad stack pointer could corrupt memory you aren't expecting to be modified.
pub unsafe fn syscall_spawn_thread(
    f: extern "sysv64" fn() -> !,
    stack_pointer: *const (),
    priority: SpawnThreadRelativePriority,
) {
    let input = SyscallSpawnThreadInput {
        priority,
        rsp: stack_pointer as u64,
        rip: f as *const () as u64,
    };
    unsafe { syscall::<SyscallSpawnThread>(&input) };
}

/// Get this thread's id
pub fn syscall_get_thread_id() -> NonZeroU32 {
    unsafe { syscall::<SyscallGetThreadId>(&()) }
}

pub fn syscall_alloc_stack(len: usize) -> Result<SyscallAllocStackOutput, SyscallAllocStackError> {
    let input = SyscallAllocStackInput { len: len as u64 };
    unsafe { syscall::<SyscallAllocStack>(&input) }
}

pub fn syscall_exit_thread() -> ! {
    // Safety: input ok
    unsafe { syscall::<SyscallExitThread>(&()) }
    unreachable!()
}

pub fn syscall_map_module(module_id: usize) -> Result<&'static [u8], SyscallMapModuleError> {
    let input = module_id as u64;
    // Safety: input ok
    let output = unsafe { syscall::<SyscallMapModule>(&input) }?;
    // Safety: slice is a to a [u8]
    let slice = unsafe { output.to_slice() };
    Ok(slice)
}

#[derive(Debug)]
pub struct RustSyscallSpawnProcessInput<'a> {
    pub priority: SpawnProcessRelativePriority,
    pub rip: u64,
    pub rsp: u64,
    pub memory_mapping: &'a [SpawnProcessMemoryMapping],
    pub send_channels: &'a [u64],
}

pub fn syscall_spawn_process(input: RustSyscallSpawnProcessInput) {
    let input = SyscallSpawnProcessInput {
        priority: input.priority,
        rip: input.rip,
        rsp: input.rsp,
        memory_mappings: input.memory_mapping.into(),
        send_channels: input.send_channels.into(),
    };
    // Safety: input ok
    unsafe { syscall::<SyscallSpawnProcess>(&(&input as *const _ as u64)) };
}

pub fn syscall_create_channel() -> u64 {
    unsafe { syscall::<SyscallCreateChannel>(&()) }
}

pub fn syscall_tx_send(channel_id: u64) {
    unsafe { syscall::<SyscallTxSend>(&channel_id) };
}
