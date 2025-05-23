use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use crate::cpu_local_data::get_local;

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
    panic!(
        "Page fault! Stack frame: {:#?}. Error code: {:#?}.",
        stack_frame, error_code
    )
}

pub fn init() {
    let idt = &get_local().idt;
    let idt = {
        idt.try_init_once(|| {
            let mut idt = InterruptDescriptorTable::new();
            idt.breakpoint.set_handler_fn(breakpoint_handler);
            idt.double_fault.set_handler_fn(double_fault_handler);
            idt.page_fault.set_handler_fn(page_fault_handler);
            idt
        })
        .unwrap();
        idt.try_get().unwrap()
    };
    idt.load();
}
