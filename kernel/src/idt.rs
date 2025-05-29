use x86_64::{
    registers::control::Cr2,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
};

use crate::{
    cpu_local_data::get_local,
    gdt::{DOUBLE_FAULT_STACK_INDEX, FIRST_EXCEPTION_STACK_INDEX},
};

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    log::info!("Breakpoint! Stack frame: {:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!(
        "Double Fault! Stack frame: {:#?}. Error code: {}.",
        stack_frame, error_code
    )
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let accessed_address = Cr2::read().unwrap();
    panic!(
        "Page fault! Stack frame: {:#?}. Error code: {:#?}. Accessed address: {:?}.",
        stack_frame, error_code, accessed_address
    )
}

pub fn init() {
    let idt = &get_local().idt;
    let idt = {
        idt.try_init_once(|| {
            let mut idt = InterruptDescriptorTable::new();
            unsafe {
                idt.breakpoint
                    .set_handler_fn(breakpoint_handler)
                    .set_stack_index(FIRST_EXCEPTION_STACK_INDEX)
            };
            unsafe {
                idt.double_fault
                    .set_handler_fn(double_fault_handler)
                    .set_stack_index(DOUBLE_FAULT_STACK_INDEX)
            };
            unsafe {
                idt.page_fault
                    .set_handler_fn(page_fault_handler)
                    .set_stack_index(FIRST_EXCEPTION_STACK_INDEX)
            };
            idt
        })
        .unwrap();
        idt.try_get().unwrap()
    };
    idt.load();
}
