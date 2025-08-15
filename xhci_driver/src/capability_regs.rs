use bitfield::bitfield;
use volatile::{
    VolatileFieldAccess,
    access::{NoAccess, ReadOnly},
};

#[derive(Debug, VolatileFieldAccess, Clone, Copy)]
#[repr(C)]
pub struct CapabilityRegs {
    #[access(ReadOnly)]
    pub cap_length: u8,
    #[access(NoAccess)]
    _reserved_0: u8,
    #[access(ReadOnly)]
    pub hci_version: HciVersion,
    #[access(ReadOnly)]
    pub hcs_params_1: HcsParams1,
    #[access(ReadOnly)]
    pub hcs_params_2: HcsParams2,
    #[access(ReadOnly)]
    pub hcs_params_3: u32,
    #[access(ReadOnly)]
    pub hcc_params_1: HccParams1,
    #[access(ReadOnly)]
    pub doorbell_offset: u32,
    #[access(ReadOnly)]
    pub rts_off: u32,
    #[access(ReadOnly)]
    pub hcc_params_2: u32,
}

bitfield! {
    /// xHCI 5.3.2 Host Controller Interface Version Number (HCIVERSION)
    #[derive(Clone, Copy)]
    pub struct HciVersion(u16);
    impl Debug;

    u8; pub major_revision, _: 15, 8;
    u8; pub minor_revision_extensions, _: 7, 0;
}

bitfield! {
    /// xHCI 5.3.3 Structural Parameters 1 (HCSPARAMS1)
    #[derive(Clone, Copy)]
    pub struct HcsParams1(u32);
    impl Debug;

    u8; pub max_slots, _: 7, 0;
    u16; pub max_interrupters, _: 18, 8;
    u8; pub max_ports, _: 31, 24;
}

bitfield! {
    /// xHCI 5.3.4 Structural Parameters 2 (HCSPARAMS2)
    #[derive(Clone, Copy)]
    pub struct HcsParams2(u32);
    impl Debug;

    u8; pub isochronous_scheduling_threshold, _: 3, 0;
    u8; pub erst_max, _: 7, 4;
    u8; max_scratchpad_buffers_hi, _: 25, 21;
    bool; pub scratchpad_restore, _: 26;
    u8; max_scratchpad_buffers_lo, _: 31, 27;
}

impl HcsParams2 {
    pub fn max_scratchpad_buffers(&self) -> u8 {
        self.max_scratchpad_buffers_lo() | (self.max_scratchpad_buffers_hi() << 4)
    }
}

bitfield! {
    /// xHCI 5.3.5 Structural Parameters 3 (HCSPARAMS3)
    #[derive(Clone, Copy)]
    pub struct HcsParams3(u32);
    impl Debug;

    u8; pub u1_device_exit_latency, _: 7, 0;
    u16; pub u2_device_exit_latency, _: 31, 16;
}

bitfield! {
    /// xHCI 5.3.6 Capability Parameters 1 (HCCPARAMS1)
    #[derive(Clone, Copy)]
    pub struct HccParams1(u32);
    impl Debug;

    pub ac64, _: 0;
    pub bnc, _: 1;
    pub csz, _: 2;
    pub ppc, _: 3;
    pub pind, _: 4;
    pub lhrc, _: 5;
    pub ltc, _: 6;
    pub nss, _: 7;
    pub pae, _: 8;
    pub spc, _: 9;
    pub sec, _: 10;
    pub cfc, _: 11;
    u8; pub max_psa_size, _: 15, 12;
    u16;
    /// An offset in `u32`s from the base to the first extended capability.
    /// So we can get the address by doing the value of this * size_of::<u32>().
    /// If this is `0` that means there are no extended capabilities.
    pub xhci_extended_capabilities_ptr, _: 31, 16;
}

bitfield! {
    /// xHCI 5.3.9 Capability Parameters 2 (HCCPARAMS2)
    #[derive(Clone, Copy)]
    pub struct HccParams2(u32);
    impl Debug;

    pub u3_entry_capability, _: 0;
    pub cmc, _: 1;
    pub fsc, _: 2;
    pub ctc, _: 3;
    pub lec, _: 4;
    pub cic, _: 5;
    pub etc, _: 6;
    pub etc_tsc, _: 7;
    pub gsc, _: 8;
    pub vtc, _: 9;
}
