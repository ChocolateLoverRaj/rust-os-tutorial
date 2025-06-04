use core::{
    arch::{asm, naked_asm},
    mem::offset_of,
};

use x86_64::{
    VirtAddr,
    registers::{
        control::{Efer, EferFlags},
        model_specific::{LStar, SFMask},
        rflags::RFlags,
    },
};

use crate::{
    boxed_stack::BoxedStack,
    cpu_local_data::{CpuLocalData, get_local},
};

#[unsafe(naked)]
unsafe extern "sysv64" fn raw_syscall_handler() -> ! {
    naked_asm!(
        "
            // Save the user mode stack pointer
            mov gs:[{user_mode_stack_pointer_offset}], rsp
            // Switch to the kernel stack pointer
            mov rsp, gs:[{syscall_handler_stack_pointer_offset}]

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
        syscall_handler = sym syscall_handler,
        user_mode_stack_pointer_offset = const offset_of!(CpuLocalData, user_mode_stack_pointer),
        syscall_handler_stack_pointer_offset = const offset_of!(CpuLocalData, syscall_handler_stack_pointer)
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
) -> ! {
    let mut inputs = [input0, input1, input2, input3, input4, input5, input6];
    for input in &mut inputs {
        *input = input.wrapping_add(5);
    }
    let user_mode_stack_pointer_ptr = get_local().user_mode_stack_pointer.get();
    // Safety: the stack pointer was saved by the raw_syscall_handler
    let user_mode_stack_pointer = unsafe { user_mode_stack_pointer_ptr.read() };
    unsafe {
        asm!(
            "
                mov rsp, {}
                sysretq
            ",
            in(reg) user_mode_stack_pointer,
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
    let syscall_handler_stack = local
        .syscall_handler_stack
        .call_once(|| BoxedStack::new_uninit(64 * 0x400));
    let syscall_handler_stack_pointer_ptr = local.syscall_handler_stack_pointer.get();
    let syscall_handler_stack_pointer = syscall_handler_stack.top().as_u64();
    unsafe { syscall_handler_stack_pointer_ptr.write(syscall_handler_stack_pointer) };

    // Enable syscall in IA32_EFER
    // https://shell-storm.org/x86doc/SYSCALL.html
    // https://wiki.osdev.org/CPU_Registers_x86-64#IA32_EFER
    unsafe {
        Efer::update(|flags| {
            *flags = flags.union(EferFlags::SYSTEM_CALL_EXTENSIONS);
        })
    };

    // clear Interrupt flag on syscall with AMD's MSR_FMASK register
    // This makes it so that interrupts are disabled during the syscall handler
    SFMask::write(RFlags::INTERRUPT_FLAG);

    // write handler address to AMD's MSR_LSTAR register
    LStar::write(VirtAddr::from_ptr(raw_syscall_handler as *const ()));
}
