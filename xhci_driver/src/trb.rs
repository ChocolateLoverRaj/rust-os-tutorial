use bitfield::bitfield;
use zerocopy::{FromBytes, Immutable, IntoBytes};

use crate::trb_type::XhciTrbType;

/// xHCI 4.11.1 TRB Template
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct AnyTrb {
    pub parameter: u64,
    pub status: u32,
    pub control: AnyTrbControl,
}

bitfield! {
    #[derive(Clone, Copy, FromBytes, IntoBytes, Immutable)]
    pub struct AnyTrbControl(u32);
    impl Debug;

    pub cycle_bit, set_cycle_bit: 0;
    u8; pub trb_type, set_trb_type: 15, 10;
}

#[derive(Debug, Clone, Copy, FromBytes, IntoBytes)]
#[repr(C)]
pub struct LinkTrb {
    parameter: u64,
    status: u32,
    control: LinkTrbControl,
}

bitfield! {
    #[derive(Clone, Copy, FromBytes, IntoBytes)]
    pub struct LinkTrbControl(u32);
    impl Debug;

    cycle_bit, set_cycle_bit: 0;
    toggle_cycle, set_toggle_cycle: 1;
    u8; trb_type, set_trb_type: 15, 10;
}

impl LinkTrb {
    pub fn new(next_trb_phys_addr: u64, cycle_bit: bool, toggle_cycle: bool) -> Self {
        Self {
            parameter: next_trb_phys_addr,
            status: 0,
            control: {
                let mut control = LinkTrbControl(0);
                control.set_cycle_bit(cycle_bit);
                control.set_toggle_cycle(toggle_cycle);
                control.set_trb_type(XhciTrbType::Link.into());
                control
            },
        }
    }
}
