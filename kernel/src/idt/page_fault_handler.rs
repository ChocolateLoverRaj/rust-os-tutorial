use x86_64::{
    PrivilegeLevel,
    registers::control::Cr2,
    structures::idt::{InterruptStackFrame, PageFaultErrorCode},
};

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
        panic!(
            "Page fault! Stack frame: {stack_frame:#?}. Error code: {error_code:#?}. Accessed address: {accessed_address:?}."
        )
    }
}
