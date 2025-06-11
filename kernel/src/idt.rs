use core::sync::atomic::Ordering;

use keyboard_interrupt_handler::raw_keyboard_interrupt_handler;
use page_fault_handler::page_fault_handler;
use x86_64::{
    PrivilegeLevel, VirtAddr,
    instructions::{interrupts, port::PortReadOnly},
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame},
};

use crate::{
    cpu_local_data::get_local,
    gdt::{DOUBLE_FAULT_STACK_INDEX, FIRST_EXCEPTION_STACK_INDEX},
    hlt_loop::hlt_loop,
    interrupt_vector::InterruptVector,
    mouse::MOUSE,
    nmi_handler_states::{NMI_HANDLER_STATES, NmiHandlerState},
};

mod keyboard_interrupt_handler;
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
    let mut local_apic = get_local().local_apic.get().unwrap().lock();
    // Safety: We are done with an interrupt triggered by the local APIC
    unsafe { local_apic.end_of_interrupt() };
}

fn handle_panic_originating_on_other_cpu() -> ! {
    interrupts::disable();
    hlt_loop()
}

extern "x86-interrupt" fn nmi_handler(_stack_frame: InterruptStackFrame) {
    handle_panic_originating_on_other_cpu()
}

extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port = PortReadOnly::new(0x60);
    let byte = unsafe { port.read() };
    if let Some(packet) = MOUSE.try_lock().unwrap().as_mut().unwrap().add_byte(byte) {
        log::info!("Received mouse packet: {packet:?}");
    }
    unsafe {
        get_local()
            .local_apic
            .get()
            .unwrap()
            .try_lock()
            .unwrap()
            .end_of_interrupt()
    };
}

pub fn init() {
    let local = get_local();
    let idt = local.idt.call_once(|| {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            idt.breakpoint
                .set_handler_fn(breakpoint_handler)
                .set_stack_index(FIRST_EXCEPTION_STACK_INDEX)
                // This let's Ring3 do int3 and trigger our breakpoint handler.
                // Without this, a GP fault will happen if Ring3 does int3.
                .set_privilege_level(PrivilegeLevel::Ring3)
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
        idt.non_maskable_interrupt.set_handler_fn(nmi_handler);
        idt[u8::from(InterruptVector::LocalApicTimer)].set_handler_fn(apic_timer_interrupt_handler);
        unsafe {
            idt[u8::from(InterruptVector::Keyboard)].set_handler_addr(VirtAddr::from_ptr(
                raw_keyboard_interrupt_handler as *const (),
            ))
        };
        idt[u8::from(InterruptVector::Mouse)].set_handler_fn(mouse_interrupt_handler);
        idt
    });
    idt.load();
    // Now that we loaded the IDT, we are ready to receive NMIs
    // Let's update our state to indicate that we are ready to receive NMIs
    if NMI_HANDLER_STATES
        .get()
        .unwrap()
        .get(&local.cpu.lapic_id)
        .unwrap()
        .compare_exchange(
            NmiHandlerState::NmiHandlerNotSet,
            NmiHandlerState::NmiHandlerSet,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_err()
    {
        // `compare_exchange` will "fail" if the value is currently not what we expected it to be.
        // In this case, the kernel already panicked and updated our state to `KernelPanicked` before we tried to indicate that we are ready to receive NMIs.
        handle_panic_originating_on_other_cpu()
    };
}
