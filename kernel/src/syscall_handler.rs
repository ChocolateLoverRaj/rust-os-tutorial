use core::{
    arch::{asm, naked_asm},
    mem::offset_of,
    sync::atomic::Ordering,
};

use x86_64::{
    VirtAddr,
    registers::{
        control::{Efer, EferFlags},
        model_specific::LStar,
    },
};

use crate::{
    cpu_local_data::{CpuLocalData, get_local},
    guarded_stack::{GuardedStack, StackId, StackType},
};

#[unsafe(naked)]
unsafe extern "sysv64" fn raw_syscall_handler() -> ! {
    naked_asm!(
        "
            // Save the user mode stack pointer
            mov gs:[{syscall_handler_scratch_offset}], rsp
            // Switch to the kernel stack pointer
            mov rsp, gs:[{syscall_handler_stack_pointer_offset}]

            // This is input[9]
            push gs:[{syscall_handler_scratch_offset}]
            // This is input[8]
            // Make sure to save `rcx` before modifying it
            push rcx
            // This is input[7]
            push r11
            // This is input[6]
            push rax
            // Convert `syscall`s `r10` input to `sysv64`s `rcx` input
            mov rcx, r10
            call {syscall_handler}
        ",
        syscall_handler_scratch_offset = const offset_of!(CpuLocalData, syscall_handler_scratch),
        syscall_handler_stack_pointer_offset = const offset_of!(CpuLocalData, syscall_handler_stack_pointer),
        syscall_handler = sym syscall_handler,
    )
}

unsafe extern "sysv64" fn syscall_handler(
    input0: u64,
    input1: u64,
    input2: u64,
    input3: u64,
    input4: u64,
    input5: u64,
    input6: u64,
    rflags: u64,
    return_instruction_pointer: u64,
    return_stack_pointer: u64,
) -> ! {
    let mut inputs = [input0, input1, input2, input3, input4, input5, input6];
    log::debug!("Inputs: {inputs:?}");
    for input in &mut inputs {
        *input = input.wrapping_add(1);
    }
    unsafe {
        asm!(
            "
                mov rsp, {}
                sysretq
            ",
            in(reg) return_stack_pointer,
            in("rcx") return_instruction_pointer,
            in("r11") rflags,
            in("rdi") inputs[0],
            in("rsi") inputs[1],
            in("rdx") inputs[2],
            in("r10") inputs[3],
            in("r8") inputs[4],
            in("r9") inputs[5],
            in("rax") inputs[6],
            options(noreturn)
        );
    }
}

pub fn init() {
    let local = get_local();
    let syscall_handler_stack = GuardedStack::new(
        64 * 0x400,
        StackId {
            _type: StackType::SyscallHandler,
            cpu_id: local.kernel_assigned_id,
        },
    );
    local
        .syscall_handler_stack_pointer
        .store(syscall_handler_stack.top().as_u64(), Ordering::Relaxed);

    // Enable syscall in IA32_EFER
    // https://shell-storm.org/x86doc/SYSCALL.html
    // https://wiki.osdev.org/CPU_Registers_x86-64#IA32_EFER
    unsafe {
        Efer::update(|flags| {
            *flags = flags.union(EferFlags::SYSTEM_CALL_EXTENSIONS);
        })
    };

    // This tells the CPU the address of our syscall handler
    LStar::write(VirtAddr::from_ptr(raw_syscall_handler as *const ()));
}
