use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::{cpu_local_data::get_local, gdt::IstStackIndexes};
use page_fault_handler::*;

mod page_fault_handler;

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    log::info!("Breakpoint! Stack frame: {stack_frame:#?}");
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!("Double Fault! Stack frame: {stack_frame:#?}. Error code: {error_code}.")
}

pub fn init() {
    let idt = get_local().idt.call_once(|| {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.page_fault
                .set_handler_fn(page_fault_handler)
                .set_stack_index(u8::from(IstStackIndexes::Exception).into())
        };
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(u8::from(IstStackIndexes::Exception).into())
        };
        idt
    });
    idt.load();
}
