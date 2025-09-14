use x86_64::{
    registers::control::Cr2,
    structures::idt::{InterruptStackFrame, PageFaultErrorCode},
};

use crate::guarded_stack::STACK_GUARD_PAGES;

pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let accessed_address = Cr2::read().unwrap();
    if error_code.contains(PageFaultErrorCode::USER_MODE) {
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
        panic!(
            "Page fault! Stack frame: {stack_frame:#?}. Error code: {error_code:#?}. Accessed address: {accessed_address:?}."
        );
    }
}
