use core::{ops::DerefMut, ptr::NonNull, sync::atomic::Ordering};

use x86_64::{
    registers::control::Cr2,
    structures::idt::{InterruptStackFrame, PageFaultErrorCode},
};

use crate::{
    cpu_local_data::get_local, guarded_stack::STACK_GUARD_PAGES, hlt_loop::hlt_loop,
    scheduler::ThreadState, try_access_user_mem::AccessUserMemError,
};

pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let accessed_address = Cr2::read().unwrap();
    let local = get_local();
    if error_code.contains(PageFaultErrorCode::USER_MODE) {
        log::warn!("Thread caused a page fault. Ending thread.");
        let mut running_thread = local.running_thread.try_lock().unwrap().take().unwrap();
        *running_thread = ThreadState::Ended;
        hlt_loop()
    } else if let Some(stack) = STACK_GUARD_PAGES
        .lock()
        .iter()
        .find_map(|(page, stack_id)| {
            if accessed_address.align_down(page.size().byte_len_u64()) == page.start_addr() {
                Some(*stack_id)
            } else {
                None
            }
        })
    {
        panic!("Stack overflow: {stack:#X?}");
    } else {
        let mut result_guard = local.try_access_user_mem_result.try_lock().unwrap();
        if let Some(result) = result_guard.deref_mut() {
            *result = Err(AccessUserMemError { accessed_address });
            drop(result_guard);

            // We can use relaxed ordering because it was stored on the same CPU with a normal `mov`` instruction
            let rsp = local.try_access_user_mem_rsp.load(Ordering::Relaxed);
            // After the rsp was saved, the call instruction was executed
            // The call instruction pushes the return instruction address to the stack
            // So we can read the usize below the rsp (because the stack grows down) to get the return instruction address
            let rip_ptr = NonNull::new((rsp - size_of::<usize>()) as *mut usize).unwrap();
            // Safety: the assembly code pushed a valid return instruction address to the stack
            let rip = unsafe { rip_ptr.read() };

            // Restore the other stuff from the interrupt stack frame
            let rflags = stack_frame.cpu_flags.bits();
            let code_segment = u64::from(stack_frame.code_segment.0);
            let stack_segment = u64::from(stack_frame.stack_segment.0);

            unsafe {
                core::arch::asm!(
                    "push {stack_segment}",
                    "push {new_stack_pointer}",
                    "push {rflags}",
                    "push {code_segment}",
                    "push {new_instruction_pointer}",
                    "iretq",
                    rflags = in(reg) rflags,
                    new_instruction_pointer = in(reg) rip,
                    new_stack_pointer = in(reg) rsp,
                    code_segment = in(reg) code_segment,
                    stack_segment = in(reg) stack_segment,
                    options(noreturn)
                )
            }
        } else {
            panic!(
                "Page fault! Stack frame: {stack_frame:#?}. Error code: {error_code:#?}. Accessed address: {accessed_address:?}."
            );
        }
    }
}
