use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::cpu_local_data::get_local;

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    log::info!("Breakpoint! Stack frame: {stack_frame:#?}");
}

pub fn init() {
    let idt = get_local().idt.call_once(|| {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt
    });
    idt.load();
}
