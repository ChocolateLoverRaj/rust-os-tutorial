#![no_std]

use bitfield::bitfield;
use x86_64::instructions::port::Port;
pub fn init(base_address: u16) {
    log::error!("Base address: 0x{base_address:X?}");
    // Registers
    let mut usb_cmd = Port::<u16>::new(base_address + 0x00);
    let mut usb_sts = Port::<u16>::new(base_address + 0x02);
    let mut usb_intr = Port::<u16>::new(base_address + 0x04);
    let mut fr_num = Port::<u16>::new(base_address + 0x06);
    let mut fr_base_addr = Port::<u32>::new(0x08);
    let mut sof_mod = Port::<u8>::new(base_address + 0x0C);
    let mut port_sc_1 = Port::<u16>::new(base_address + 0x10);
    let mut port_sc_2 = Port::<u16>::new(base_address + 0x12);
    let mut legacy_support = Port::<u16>::new(base_address + 0xC0);

    // https://wiki.osdev.org/Universal_Host_Controller_Interface#Initalization
    unsafe {
        legacy_support.write({
            let mut reg = LegacySupportRegister(0);
            reg.set_usb_pirq_enable(true);
            reg.0
        })
    };

    // Set I/O busmastering in PCI
    // Caller does this

    // Reset controller by Host Controller Reset (bit will self-clear) and Global Reset (you need to clear bit after 10ms)
    unsafe {
        let reg = {
            let mut reg = CommandRegister(usb_cmd.read());
            reg.set_host_controller_reset(true);
            reg.0
        };
        usb_cmd.write(reg);
    };
    // TODO: Timeout
    while CommandRegister(unsafe { usb_cmd.read() }).host_controller_reset() {}

    unsafe {
        let reg = {
            let mut reg = CommandRegister(usb_cmd.read());
            reg.set_global_reset(true);
            reg.0
        };
        usb_cmd.write(reg);
    }
    // TODO: wait 10 ms
    unsafe {
        let reg = {
            let mut reg = CommandRegister(usb_cmd.read());
            reg.set_global_reset(false);
            reg.0
        };
        usb_cmd.write(reg);
    }

    log::debug!("Reset UHCI");
}

bitfield! {
    #[derive(Clone, Copy)]
    pub(crate) struct LegacySupportRegister(u16);
    impl Debug;

    pub usb_pirq_enable, set_usb_pirq_enable: 13;
}

bitfield! {
    #[derive(Clone, Copy)]
    pub(crate) struct CommandRegister(u16);
    impl Debug;

    pub run, set_run: 0;
    pub host_controller_reset, set_host_controller_reset: 1;
    pub global_reset, set_global_reset: 2;
    pub global_suspend, set_global_suspend: 3;
    pub global_resume, set_global_resume: 4;
    pub software_debug, set_software_debug: 5;
    pub configure_flag, set_configure_flag: 6;
    pub max_package_size, set_max_packet_size: 7;
}

bitfield! {
    #[derive(Clone, Copy)]
    pub(crate) struct StatusRegister(u16);
    impl Debug;

    pub interrupt, set_interrupt: 0;
    pub error_interrupt, set_error_interrupt: 1;
    pub resume_detected, set_resume_detected: 2;
    pub system_error, set_system_error: 3;
    pub process_error, set_process_error: 4;
    pub halted, set_halted: 5;
}

bitfield! {
    #[derive(Clone, Copy)]
    pub(crate) struct InterruptEnableRegister(u16);
    impl Debug;

    pub timeout_crc, set_timeout_crc: 0;
    pub resume, set_resume: 1;
    pub complete_transfer, set_complete_transfer: 2;
    pub short_package, set_short_packet: 3;
}

bitfield! {
    #[derive(Clone, Copy)]
    pub(crate) struct PortScRegister(u16);
    impl Debug;

    pub connection_status, set_connection_status: 0;
    pub connection_status_change, set_connection_status_change: 1;
    pub device_enable, set_device_enable: 2;
    pub port_enable_changed, set_port_enable_changed: 3;
    u8; pub line_status, set_line_status: 5, 4;
    pub resume_detected, set_resume_detected: 6;
    pub low_speed_device, _: 8;
    pub reset, set_reset: 9;
    pub suspend, set_suspend: 12;
}
