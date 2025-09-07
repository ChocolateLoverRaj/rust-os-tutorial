use core::sync::atomic::{AtomicBool, Ordering};

use crate::{
    cpu_local_data::{local_apic_id_of, try_get_local},
    hlt_loop::hlt_loop,
    nmi_handler_states::{NMI_HANDLER_STATES, NmiHandlerState},
};

static DID_PANIC: AtomicBool = AtomicBool::new(false);
#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    if !DID_PANIC.swap(true, Ordering::Relaxed) {
        log::error!("{info}");
        // Since the OS panicked, we need to tell the other CPUs to stop immediately
        // However, if we send an NMI to a CPU that didn't load its IDT yet, the system will triple fault
        if let Some(local) = try_get_local()
            && let Some(mut local_apic) = local
                .local_apic
                .get()
                .and_then(|local_apic| local_apic.try_lock())
            && let Some(nmi_handler_states) = NMI_HANDLER_STATES.get()
        {
            for (cpu_id, nmi_handler_state) in nmi_handler_states
                .iter()
                .enumerate()
                // Make sure to not send an NMI to our own CPU
                .filter(|(cpu_id, _)| *cpu_id as u32 != local.kernel_assigned_id)
            {
                if let NmiHandlerState::NmiHandlerSet =
                    nmi_handler_state.swap(NmiHandlerState::KernelPanicked, Ordering::Release)
                {
                    // Safety: since the kernel is panicking, we need to tell the other CPUs to hlt
                    unsafe { local_apic.send_nmi(local_apic_id_of(cpu_id as u32)) };
                }
            }
        }
        hlt_loop();
    } else {
        hlt_loop();
    }
}
