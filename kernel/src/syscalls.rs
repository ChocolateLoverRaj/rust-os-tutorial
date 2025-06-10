use core::{arch::naked_asm, mem::offset_of};

use spin::Lazy;
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
    syscall_handlers::SyscallHandlers,
    syscall_saved_regs::SyscallSavedRegs,
};

/// We use `Lazy` because we cannot initialize `SyscallHandlers` without the global allocator enabled
static SYSCALL_HANDLERS: Lazy<SyscallHandlers> = Lazy::new(Default::default);

unsafe extern "sysv64" fn syscall_handler(
    input0: u64,
    input1: u64,
    input2: u64,
    input3: u64,
    input4: u64,
    input5: u64,
    input6: u64,
    syscall_saved_regs: &mut SyscallSavedRegs,
) -> ! {
    SYSCALL_HANDLERS.handle_syscall(
        input0,
        input1,
        input2,
        input3,
        input4,
        input5,
        input6,
        syscall_saved_regs,
    )
}

#[unsafe(naked)]
unsafe extern "sysv64" fn raw_syscall_handler() -> ! {
    naked_asm!(
        "
            // Save the user mode stack pointer
            mov gs:[{user_mode_stack_pointer_offset}], rsp
            // Switch to the kernel stack pointer
            mov rsp, gs:[{syscall_handler_stack_pointer_offset}]

            // backup registers for sysretq
            push gs:[{user_mode_stack_pointer_offset}]
            push rcx
            push r11

            // save callee-saved registers on the stack
            push rbp
            push rbx
            push r12
            push r13
            push r14
            push r15

            // Call the function
            // Convert `syscall`s `r10` input to `sysv64`s `rcx` input
            mov rcx, r10
            // After the first 6 inputs, additional inputs go on the stack **in reverse order**. So we put `rax` on the stack
            push rsp // I added an extra input which is the kernel's stack pointer
            push rax // Move rax to the stack which is where additional inputs go in sysv64
            call {syscall_handler}
        ",
        syscall_handler = sym syscall_handler,
        user_mode_stack_pointer_offset = const offset_of!(CpuLocalData, user_mode_stack_pointer),
        syscall_handler_stack_pointer_offset = const offset_of!(CpuLocalData, syscall_handler_stack_pointer)
    )
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
