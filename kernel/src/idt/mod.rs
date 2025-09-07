use core::sync::atomic::Ordering;

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::{
    InterruptVector,
    cpu_local_data::get_local,
    gdt::IstStackIndexes,
    hlt_loop,
    nmi_handler_states::{NMI_HANDLER_STATES, NmiHandlerState},
};
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

extern "x86-interrupt" fn apic_timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    log::info!("Received APIC timer interrupt");
    // We must notify the local APIC that it's the end of interrupt, otherwise we won't receive any more interrupts from it
    let mut local_apic = get_local().local_apic.get().unwrap().try_lock().unwrap();
    // Safety: We are done with an interrupt triggered by the local APIC
    unsafe { local_apic.end_of_interrupt() };
}

fn handle_panic_originating_on_other_cpu() -> ! {
    hlt_loop()
}

extern "x86-interrupt" fn nmi_handler(_stack_frame: InterruptStackFrame) {
    handle_panic_originating_on_other_cpu()
}

pub fn init() {
    let local = get_local();
    let idt = local.idt.call_once(|| {
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
        idt[u8::from(InterruptVector::LocalApicTimer)].set_handler_fn(apic_timer_interrupt_handler);
        idt.non_maskable_interrupt.set_handler_fn(nmi_handler);
        idt
    });
    idt.load();
    // Now that we loaded the IDT, we are ready to receive NMIs
    // Let's update our state to indicate that we are ready to receive NMIs
    if NMI_HANDLER_STATES.get().unwrap()[local.kernel_assigned_id as usize]
        .compare_exchange(
            NmiHandlerState::NmiHandlerNotSet,
            NmiHandlerState::NmiHandlerSet,
            Ordering::Relaxed,
            Ordering::Relaxed,
        )
        .is_err()
    {
        // `compare_exchange` will "fail" if the value is currently not what we expected it to be.
        // In this case, the kernel already panicked and updated our state to `KernelPanicked` before we tried to indicate that we are ready to receive NMIs.
        handle_panic_originating_on_other_cpu()
    };
}
