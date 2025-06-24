use x86_64::{
    PrivilegeLevel,
    registers::control::Cr2,
    structures::{
        idt::{InterruptStackFrame, PageFaultErrorCode},
        paging::{Page, Size4KiB},
    },
};

use crate::guarded_stack::{STACK_GUARD_PAGES, StackType};

pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    // Our kernel needs to gracefully handle user mode causing page faults.
    // We should not panic because of anything user mode does.
    if stack_frame.code_segment.rpl() == PrivilegeLevel::Ring3 {
        todo!("User mode program caused a page fault. Terminate process.");
    } else {
        let accessed_address = Cr2::read().unwrap();
        let accessed_page = Page::<Size4KiB>::containing_address(accessed_address);
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
                stack: stack.clone(),
                guard_page: accessed_page,
            };
            panic!("{stack_overflow:?}")
        } else {
            panic!(
                "Page fault! Stack frame: {stack_frame:#?}. Error code: {error_code:#?}. Accessed address: {accessed_address:?}."
            )
        }
    }
}
