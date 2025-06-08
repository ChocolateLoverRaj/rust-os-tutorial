use core::{alloc::Layout, arch::asm, num::NonZeroU64};

use common::{
    Syscall, SyscallAlloc, SyscallAlloc2, SyscallAlloc2Error, SyscallAlloc2Input,
    SyscallAllocError, SyscallAllocInput, SyscallExists, SyscallExit, SyscallLog, SyscallLogInput,
    SyscallMemInfo, SyscallMemInfoOutput, log,
};
use x86_64::structures::paging::{Page, PageSize, Size4KiB};

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

pub fn syscall_mem_info() -> SyscallMemInfoOutput {
    // Safety: there is no possibility of invalid input
    unsafe { syscall::<SyscallMemInfo>(&()) }
}

/// # Safety
/// Can not overlap with existing addresses
pub unsafe fn syscall_alloc(
    start_page: Page<Size4KiB>,
    page_len: u64,
) -> Result<(), SyscallAllocError> {
    let input = SyscallAllocInput {
        start: start_page.start_address().as_u64(),
        len: page_len * Size4KiB::SIZE,
    };
    // Safety: we made sure it's aligned
    unsafe { syscall::<SyscallAlloc>(&input) }
}

pub fn syscall_alloc_2(layout: Layout) -> Result<*mut [u8], SyscallAlloc2Error> {
    let input = layout.into();
    let slice = unsafe { syscall::<SyscallAlloc2>(&input) }?;
    Ok(unsafe { slice.to_slice_mut() })
}
