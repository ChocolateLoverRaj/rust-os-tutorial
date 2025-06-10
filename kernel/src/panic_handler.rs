use core::sync::atomic::{AtomicBool, Ordering};

use x86_64::instructions::interrupts;

use crate::{
    cpu_local_data::try_get_local,
    hlt_loop::hlt_loop,
    nmi_handler_states::{NMI_HANDLER_STATES, NmiHandlerState},
};

static DID_PANIC: AtomicBool = AtomicBool::new(false);
#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    interrupts::disable();
    match DID_PANIC.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed) {
        Ok(_) => {
            // Since the OS panicked, we need to tell the other CPUs to stop immediately
            // However, if we send an NMI to a CPU that didn't load its IDT yet, the system will triple fault
            if let Some(local) = try_get_local()
                && let Some(mut local_apic) = local
                    .local_apic
                    .get()
                    .and_then(|local_apic| local_apic.try_lock())
            {
                for (cpu_lapic_id, nmi_handler_state) in NMI_HANDLER_STATES
                    .get()
                    .unwrap()
                    .iter()
                    // Make sure to not send an NMI to our own CPU
                    .filter(|(cpu_lapic_id, _)| **cpu_lapic_id != local.cpu.lapic_id)
                {
                    if let NmiHandlerState::NmiHandlerSet =
                        nmi_handler_state.swap(NmiHandlerState::KernelPanicked, Ordering::Release)
                    {
                        // Safety: since the kernel is panicking, we need to tell the other CPUs to hlt
                        unsafe { local_apic.send_nmi(*cpu_lapic_id) };
                    }
                }
            }
            log::error!("{info}");
            hlt_loop();
        }
        Err(_) => {
            hlt_loop();
        }
    }
}
