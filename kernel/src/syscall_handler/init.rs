use core::sync::atomic::Ordering;

use x86_64::{
    VirtAddr,
    registers::{
        control::{Efer, EferFlags},
        model_specific::LStar,
    },
};

use crate::{
    cpu_local_data::get_local,
    guarded_stack::{GuardedStack, StackId, StackType},
};

use super::*;

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
