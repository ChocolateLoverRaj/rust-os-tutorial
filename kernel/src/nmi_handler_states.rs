use alloc::collections::btree_map::BTreeMap;
use atomic_enum::atomic_enum;
use limine::response::MpResponse;
use spin::Once;

#[atomic_enum]
pub enum NmiHandlerState {
    /// If this CPU receives an NMI, it will probably cause a triple fault
    NmiHandlerNotSet,
    /// If this CPU receives an NMI, the kernel's NMI handler function will be called
    NmiHandlerSet,
    /// If you see this while trying to set the NMI, just call the NMI handler now
    KernelPanicked,
}

pub static NMI_HANDLER_STATES: Once<BTreeMap<u32, AtomicNmiHandlerState>> = Once::new();

pub fn init(mp_response: &MpResponse) {
    NMI_HANDLER_STATES.call_once(|| {
        mp_response
            .cpus()
            .iter()
            .map(|cpu| {
                (
                    cpu.lapic_id,
                    AtomicNmiHandlerState::new(NmiHandlerState::NmiHandlerNotSet),
                )
            })
            .collect()
    });
}
