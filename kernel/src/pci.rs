// use x86_64::instructions::port::Port;

pub fn init() {
    // let mut config_address = Port::<u32>::new(0xCF8);
    // let mut config_data = Port::<u32>::new(0xCFC);

    // let mut pci_config_read_u16 = |bus: u8, slot: u8, func: u8, offset: u8| {
    //     let address = ((bus as u32) << 16)
    //         | ((slot as u32) << 11)
    //         | ((func as u32) << 8)
    //         | ((offset as u32) & 0xFC)
    //         | 0x80000000;
    //     unsafe { config_address.write(address) };
    //     let data = unsafe { config_data.read() };
    //     ((data >> ((offset & 2) * 8)) & 0xFFFF) as u16
    // };

    // let mut pci_config_read_u32 = |bus: u8, slot: u8, func: u8, offset: u8| {
    //     let address = ((bus as u32) << 16)
    //         | ((slot as u32) << 11)
    //         | ((func as u32) << 8)
    //         | ((offset as u32) & 0xFC)
    //         | 0x80000000;
    //     unsafe { config_address.write(address) };
    //     let data = unsafe { config_data.read() };
    //     data >> ((offset & 2) * 8)
    // };

    // let mut pci_check_vendor = |bus: u8, slot: u8| {
    //     let vendor = pci_config_read_u16(bus, slot, 0, 0);
    //     let device = pci_config_read_u16(bus, slot, 0, 2);
    //     vendor
    // };

    // let reg_0 = pci_config_read_u32(0, 0, 0, 0);
    // log::info!("Reg 0: {reg_0}");

    // for bus in u8::MIN..u8::MAX {
    //     for device in 0..32 {
    //         let reg = pci_config_read_u32(bus, device, 0, 0x0);
    //         let vendor_id = reg as u16;
    //         if vendor_id != u16::MAX {
    //             let device_id = (reg >> 16) as u16;
    //             let reg = pci_config_read_u32(bus, device, 0, 0x8);
    //             let revision_id = reg as u8;
    //             let sub_class = (reg >> 16) as u8;
    //             let class_code = (reg >> 24) as u8;
    //             let reg = pci_config_read_u32(bus, device, 0, 0xC);
    //             let header_type = (reg >> 16) as u8;
    //             #[derive(Debug)]
    //             struct PciDevice {
    //                 vendor_id: u16,
    //                 device_id: u16,
    //                 revision_id: u8,
    //                 sub_class: u8,
    //                 class_code: u8,
    //                 header_type: u8,
    //             }
    //             let pci_device = PciDevice {
    //                 vendor_id,
    //                 device_id,
    //                 revision_id,
    //                 sub_class,
    //                 class_code,
    //                 header_type,
    //             };
    //             log::debug!("bus {bus:x} device {device:x} {pci_device:#X?}");
    //         }
    //     }
    // }

    polished_pci::pci_enumeration_demo();
}
