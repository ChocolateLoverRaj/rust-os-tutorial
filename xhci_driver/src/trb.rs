use bitfield::bitfield;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TransferRequestBlock {
    pub parameter: u64,
    pub status: u32,
    pub control: TrbControl,
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct TrbControl(u32);
    impl Debug;

    pub cycle_bit, set_cycle_bit: 0;
    pub toggle_cycle, set_toggle_cycle: 1;
    u8; pub trb_type, set_trb_type: 15, 10;
}
