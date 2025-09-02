use core::{
    ptr::{NonNull, null_mut},
    sync::atomic::Ordering,
};

use ez_paging::Page;
use x86_64::{
    PrivilegeLevel,
    registers::control::Cr2,
    structures::idt::{InterruptStackFrame, PageFaultErrorCode},
};

use crate::{
    cpu_local_data::get_local,
    guarded_stack::{STACK_GUARD_PAGES, STACK_PAGE_SIZE, StackType},
    smep_smap::{clac, has_smap},
    try_access_user_mem::AccessUserMemError,
};

pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    // Our kernel needs to gracefully handle user mode causing page faults.
    // We should not panic because of anything user mode does.
    let accessed_address = Cr2::read().unwrap();
    let local = get_local();
    if stack_frame.code_segment.rpl() == PrivilegeLevel::Ring3 {
        let running_thread = local.running_thread.try_lock().unwrap().unwrap();
        todo!("Thread {running_thread:?} caused a page fault. Terminate process.");
    } else {
        let accessed_page = Page::new(
            accessed_address.align_down(STACK_PAGE_SIZE.byte_len_u64()),
            STACK_PAGE_SIZE,
        )
        .unwrap();
        if error_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE)
            && let Some(stack) = STACK_GUARD_PAGES.lock().get(&accessed_page)
        {
            #[derive(Debug)]
            #[allow(unused)]
            struct StackOverflow {
                stack: StackType,
                guard_page: Page,
            }
            let stack_overflow = StackOverflow {
                stack: *stack,
                guard_page: accessed_page,
            };
            panic!("{stack_overflow:?}")
        } else if let Some(mut access_user_mem_error_pointer) = NonNull::new(
            local
                .access_user_mem_error_pointer
                .swap(null_mut(), Ordering::Relaxed),
        ) {
            // Page fault, but it was in a "try" function
            if has_smap() {
                clac();
            }

            // Safety: We have exclusive access to this, and it was enforced by Rust to be a valid pointer
            let error = unsafe { access_user_mem_error_pointer.as_mut() };
            error.write(AccessUserMemError { accessed_address });

            let copy_from_user_rsp = local.copy_from_user_rsp.load(Ordering::Relaxed);
            let rflags = stack_frame.cpu_flags.bits();
            let new_instruction_pointer_ptr = copy_from_user_rsp as *mut u64;
            let new_instruction_pointer = unsafe { new_instruction_pointer_ptr.read() };
            let new_stack_pointer = copy_from_user_rsp + u64::try_from(size_of::<u64>()).unwrap();
            let code_segment = u64::from(stack_frame.code_segment.0);
            let stack_segment = u64::from(stack_frame.stack_segment.0);

            let rbx = local.copy_from_user_rbx.load(Ordering::Relaxed);
            let rbp = local.copy_from_user_rbp.load(Ordering::Relaxed);
            let r12 = local.copy_from_user_r12.load(Ordering::Relaxed);
            let r13 = local.copy_from_user_r13.load(Ordering::Relaxed);
            let r14 = local.copy_from_user_r14.load(Ordering::Relaxed);
            let r15 = local.copy_from_user_r15.load(Ordering::Relaxed);

            unsafe {
                core::arch::asm!(
                    "mov rbx, {rbx}",
                    "mov rbp, {rbp}",
                    "mov rax, 0",
                    "push {stack_segment}",
                    "push {new_stack_pointer}",
                    "push {rflags}",
                    "push {code_segment}",
                    "push {new_instruction_pointer}",
                    "iretq",
                    rflags = in(reg) rflags,
                    new_instruction_pointer = in(reg) new_instruction_pointer,
                    new_stack_pointer = in(reg) new_stack_pointer,
                    code_segment = in(reg) code_segment,
                    stack_segment = in(reg) stack_segment,
                    rbx = in(reg) rbx,
                    rbp = in(reg) rbp,
                    in("r12") r12,
                    in("r13") r13,
                    in("r14") r14,
                    in("r15") r15,
                    options(noreturn)
                )
            }
        } else {
            panic!(
                "Page fault! Stack frame: {stack_frame:#?}. Error code: {error_code:#?}. Accessed address: {accessed_address:?}."
            )
        }
    }
}
