use x86_64::{
    registers::control::Cr2,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
};

use crate::{
    cpu_local_data::get_local,
    gdt::{DOUBLE_FAULT_STACK_INDEX, FIRST_EXCEPTION_STACK_INDEX},
    interrupt_vector::InterruptVector,
};

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    log::info!("Breakpoint! Stack frame: {stack_frame:#?}");
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!("Double Fault! Stack frame: {stack_frame:#?}. Error code: {error_code}.")
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let accessed_address = Cr2::read().unwrap();
    panic!(
        "Page fault! Stack frame: {stack_frame:#?}. Error code: {error_code:#?}. Accessed address: {accessed_address:?}."
    )
}

extern "x86-interrupt" fn apic_timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    log::info!("Received APIC timer interrupt");
    // We must notify the local APIC that it's the end of interrupt, otherwise we won't receive any more interrupts from it
    let mut local_apic = get_local().local_apic.get().unwrap().lock();
    // Safety: We are done with an interrupt triggered by the local APIC
    unsafe { local_apic.end_of_interrupt() };
}

pub fn init() {
    let idt = get_local().idt.call_once(|| {
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
        idt[u8::from(InterruptVector::LocalApicTimer)].set_handler_fn(apic_timer_interrupt_handler);
        idt
    });
    idt.load();
}
