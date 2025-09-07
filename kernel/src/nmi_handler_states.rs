use alloc::boxed::Box;
use atomic_enum::atomic_enum;
use spin::Once;

use crate::cpu_local_data::cpus_count;

#[atomic_enum]
pub enum NmiHandlerState {
    /// If this CPU receives an NMI, it will probably cause a triple fault
    NmiHandlerNotSet,
    /// If this CPU receives an NMI, the kernel's NMI handler function will be called
    NmiHandlerSet,
    /// If you see this while trying to set the NMI, just call the NMI handler now
    KernelPanicked,
}

pub static NMI_HANDLER_STATES: Once<Box<[AtomicNmiHandlerState]>> = Once::new();

pub fn init() {
    NMI_HANDLER_STATES.call_once(|| {
        (0..cpus_count())
            .map(|_| AtomicNmiHandlerState::new(NmiHandlerState::NmiHandlerNotSet))
            .collect()
    });
}
