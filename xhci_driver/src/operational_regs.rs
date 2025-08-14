use bitfield::bitfield;
use debug_ignore::DebugIgnore;
use volatile::{
    VolatileFieldAccess,
    access::{NoAccess, ReadOnly, ReadWrite},
};

/// xHCI 5.4 Host Controller Operational Registers
#[derive(Debug, VolatileFieldAccess, Clone, Copy)]
#[repr(C)]
pub struct OperationalRegs {
    #[access(ReadWrite)]
    pub usb_cmd: UsbCmd,
    #[access(ReadWrite)]
    pub usb_sts: UsbSts,
    #[access(ReadOnly)]
    pub page_size: PageSizeReg,
    #[access(NoAccess)]
    _reserved_0: DebugIgnore<[u8; 0x8]>,
    #[access(ReadWrite)]
    pub dn_ctrl: DnCtrl,
    #[access(ReadWrite)]
    pub crcr: Crcr,
    #[access(NoAccess)]
    _reserved_1: DebugIgnore<[u8; 0x10]>,
    #[access(ReadWrite)]
    pub dcbaap: Dcbaap,
    #[access(ReadWrite)]
    pub config: ConfigureRegister,
    #[access(NoAccess)]
    _reserved_2: DebugIgnore<[u8; 0x3C4]>,
}

bitfield! {
    /// xHCI 5.4.1 USB Command Register (USBCMD)
    #[derive(Clone, Copy)]
    pub struct UsbCmd(u32);
    impl Debug;

    pub run_stop, set_run_stop: 0;
    /// Host Controller Reset (HCRST) – RW. Default = ‘0’. This control bit is used by software
    /// to reset the host controller. The effects of this bit on the xHC and the Root Hub
    /// registers are similar to a Chip Hardware Reset.
    ///
    /// When software writes a ‘1’ to this bit, the Host Controller resets its internal
    /// pipelines, timers, counters, state machines, etc. to their initial value. Any
    /// transaction currently in progress on the USB is immediately terminated. A USB reset
    /// shall not be driven on USB2 downstream ports, however a Hot or Warm Reset shall be
    /// initiated on USB3 Root Hub downstream ports.
    ///
    /// PCI Configuration registers are not affected by this reset. All operational registers,
    /// including port registers and port state machines are set to their initial values.
    /// Software shall reinitialize the host controller as described in Section 4.2 in order
    /// to return the host controller to an operational state.
    ///
    /// This bit is cleared to ‘0’ by the Host Controller when the reset process is complete.
    /// Software cannot terminate the reset process early by writing a ‘0’ to this bit and
    /// shall not write any xHC Operational or Runtime registers while HCRST is ‘1’. Note,
    /// the completion of the xHC reset process is not gated by the Root Hub port reset process.
    ///
    /// Software shall not set this bit to ‘1’ when the HCHalted (HCH) bit in the USBSTS
    /// register is a ‘0’. Attempting to reset an actively running host controller may result
    /// in undefined behavior.
    ///
    /// When this register is exposed by a Virtual Function (VF), this bit only resets the xHC
    /// instance presented by the selected VF. Refer to section 8 for more information.
    pub host_controller_reset, set_host_controller_reset: 1;
    pub interrupter_enable, set_interrupter_enable: 2;
    pub hsee, set_hsee: 3;
    pub lhcrst, set_lhcrst: 7;
    pub css, set_css: 8;
    pub crs, set_crs: 9;
    pub ewe, set_ewe: 10;
    pub eu3s, set_eu3s: 11;
    pub cme, set_cme: 13;
    pub ete, set_ete: 14;
    pub tsc_en, set_tsc_en: 15;
    pub vtio_enable, set_vtio_enable: 16;
}

bitfield! {
    /// xHCI 5.4.2 USB Status Register (USBSTS)
    #[derive(Clone, Copy)]
    pub struct UsbSts(u32);
    impl Debug;

    pub hc_halted, _: 0;
    pub hse, set_hse: 2;
    pub eint, set_eint: 3;
    pub pcd, set_pcd: 4;
    pub sss, _: 8;
    pub rss, _: 9;
    pub sre, set_sre: 10;
    pub controller_not_ready, _: 11;
    pub hce, _: 12;
}

bitfield! {
    /// xHCI 5.4.3 Page Size Register (PAGESIZE)
    #[derive(Clone, Copy)]
    pub struct PageSizeReg(u32);
    impl Debug;

    u16; pub page_size, _: 15, 0;
}

bitfield! {
    /// xHCI 5.4.4 Device Notification Control Register (DNCTRL)
    #[derive(Clone, Copy)]
    pub struct DnCtrl(u32);
    impl Debug;

    u16; pub notification_enable, set_notification_enable: 15, 0;
}

bitfield! {
    /// xHCI 5.4.5 Command Ring Control Register (CRCR)
    #[derive(Clone, Copy)]
    pub struct Crcr(u64);
    impl Debug;

    pub ring_cycle_state, set_ring_cycle_state: 0;
    pub cs, set_cs: 1;
    pub ca, set_ca: 2;
    pub crr, _: 3;
    u64; _command_ring_ptr, _set_command_ring_ptr: 63, 6;
}

impl Crcr {
    pub fn command_ring_ptr(&self) -> u64 {
        self._command_ring_ptr() << 6
    }

    pub fn set_command_ring_ptr(&mut self, command_ring_ptr: u64) {
        self._set_command_ring_ptr(command_ring_ptr >> 6);
    }
}

bitfield! {
    /// xHCI 5.4.6 Device Context Base Address Array Pointer Register (DCBAAP)
    #[derive(Clone, Copy)]
    pub struct Dcbaap(u64);
    impl Debug;

    u64;
    /// Note that the high bits of the u64 are stored here. The "u64" still starts at bit 0.
    /// It's just that bits 0-5 are not used for the for dcbaap.
    /// Since the dcbaap is aligned by 64, bits 0-5 will be 0 anyways.
    /// So the spec reserved bits 0-5 so they can be used to encode other information in the future.
    _dcbaap, _set_dcbaap: 63, 6;
}

impl Dcbaap {
    pub fn dcbaap(&self) -> u64 {
        self._dcbaap() << 6
    }

    pub fn set_dcbaap(&mut self, dcbaap: u64) {
        self._set_dcbaap(dcbaap >> 6);
    }
}

bitfield! {
    /// xHCI 5.4.7 Configure Register (CONFIG)
    #[derive(Clone, Copy)]
    pub struct ConfigureRegister(u32);
    impl Debug;

    u8; pub max_slots_en, set_max_slots_en: 7, 0;
    pub u3e, set_u3e: 8;
    pub cie, set_cie: 9;
}

bitfield! {
    /// xHCI 5.4.8 Port Status and Control Register (PORTSC)
    #[derive(Clone, Copy)]
    pub struct PortStatusAndControl(u32);
    impl Debug;

    pub ccs, _: 0;
    pub ped, set_ped: 1;
    pub oca, _: 3;
    pub pr, set_pr: 4;
    u8; pub pls, set_pls: 8, 5;
    pub pp, set_pp: 9;
    u8; pub port_speed, set_port_speed: 13, 10;
    u8; pub pic, set_ic: 15, 14;
    pub lws, set_lws: 16;
    pub csc, set_csc: 17;
    pub pec, set_pec: 18;
    pub wrc, set_wrc: 19;
    pub occ, set_occ: 29;
    pub prc, set_prc: 21;
    pub plc, set_plc: 22;
    pub cec, set_cec: 23;
    pub cas, set_cas: 24;
    pub wce, set_wce: 25;
    pub wde, set_wde: 26;
    pub woe, set_woe: 27;
    pub device_removable, _: 30;
    pub warm_port_reset, set_warm_port_reset: 31;
}
