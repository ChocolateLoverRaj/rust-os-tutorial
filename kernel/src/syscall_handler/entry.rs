use core::{arch::naked_asm, mem::offset_of};

use abi::SyscallNumber;
use num_enum::TryFromPrimitiveError;

use crate::cpu_local_data::CpuLocalData;

use super::*;

#[unsafe(naked)]
pub unsafe extern "sysv64" fn raw_syscall_handler() -> ! {
    naked_asm!(
        "
            // Save the user mode stack pointer
            mov rdx, rsp
            // Switch to the kernel stack pointer
            mov rsp, gs:[{syscall_handler_stack_pointer_offset}]
            mov r8, r11
            call {syscall_handler}
        ",
        syscall_handler_stack_pointer_offset = const offset_of!(CpuLocalData, syscall_handler_stack_pointer),
        syscall_handler = sym syscall_handler,
    )
}

unsafe extern "sysv64" fn syscall_handler(
    syscall_number: u32,
    input: usize,
    return_stack_pointer: u64,
    return_instruction_pointer: u64,
    rflags: u64,
) -> ! {
    let data = SyscallData {
        input,
        return_stack_pointer,
        return_instruction_pointer,
        rflags,
    };
    match SyscallNumber::try_from(syscall_number) {
        Ok(syscall_number) => {
            (get_handler(syscall_number))(data);
        }
        Err(TryFromPrimitiveError { number }) => {
            log::debug!("Unknown syscall number: 0x{number:X}");
            data.ret_no_exist()
        }
    }
}
