use bitfield::{bitfield, bitfield_constructor};
use bitflags::bitflags;
use x86_64::instructions::port::Port;

struct Pci {
    config_address: Port<u32>,
    config_data: Port<u32>,
}

bitfield! {
  struct PciConfig(u32);
  impl Debug;
  // The fields default to u16
  enable, set_enable: 31;
  u8; bus_number, set_bus_number: 23, 15;
  u8; device_number, set_device_number: 15, 11;
  u8; function_number, set_function_number: 10, 8;
  u8; register_offset, set_register_offset: 7,0 ;
}
impl PciConfig {
    bitfield_constructor!(PciConfig);
}

impl Pci {
    /// # Safety: The ports must be PCI and not used by other code.
    pub unsafe fn new() -> Self {
        Self {
            config_address: Port::<u32>::new(0xCF8),
            config_data: Port::<u32>::new(0xCFC),
        }
    }

    pub fn pci_config_read_u32(
        &mut self,
        bus_number: u8,
        device_number: u8,
        function_number: u8,
        register_offset: u8,
    ) -> u32 {
        assert!(
            register_offset.is_multiple_of(size_of::<u32>().try_into().unwrap()),
            "Register offset represents bytes and should be aligned to u32"
        );
        let mut address = PciConfig::new();
        address.set_enable(true);
        address.set_bus_number(bus_number);
        address.set_device_number(device_number);
        address.set_function_number(function_number);
        address.set_register_offset(register_offset);

        unsafe { self.config_address.write(address.0) };
        let data = unsafe { self.config_data.read() };
        data
    }

    pub fn devices(&mut self) -> PciDevices {
        PciDevices {
            pci: self,
            index: 0,
        }
    }
}

pub struct PciDevices<'a> {
    pci: &'a mut Pci,
    index: u8,
}

impl Iterator for PciDevices<'_> {
    type Item = PciDevice;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.index == 32 {
                break None;
            }
            let (vendor_id, device_id) = {
                let reg = self.pci.pci_config_read_u32(0, self.index, 0, 0x0);
                (reg as u16, (reg >> 16) as u16)
            };
            if vendor_id != u16::MAX {
                let (command, status) = {
                    let reg = self.pci.pci_config_read_u32(0, self.index, 0, 0x4);
                    (reg as u16, (reg >> 16) as u16)
                };
                let (revision_id, prog_if, sub_class, class_code) = {
                    let reg = self.pci.pci_config_read_u32(0, self.index, 0, 0x8);
                    (
                        reg as u8,
                        (reg >> 8) as u8,
                        (reg >> 16) as u8,
                        (reg >> 24) as u8,
                    )
                };
                let (cache_line_size, latency_timer, header_type, bist) = {
                    let reg = self.pci.pci_config_read_u32(0, self.index, 0, 0xC);
                    (
                        reg as u8,
                        (reg >> 8) as u8,
                        HeaderType((reg >> 16) as u8),
                        (reg >> 24) as u8,
                    )
                };
                let pci_device = PciDevice {
                    vendor_id,
                    device_id,
                    command,
                    status,
                    revision_id,
                    prog_if,
                    sub_class,
                    class_code,
                    cache_line_size,
                    latency_timer,
                    header_type,
                    bist,
                };
                self.index += 1;
                break Some(pci_device);
            } else {
                self.index += 1;
            }
        }
    }
}

#[derive(Debug)]
pub struct PciDevice {
    pub vendor_id: u16,
    pub device_id: u16,
    pub command: u16,
    pub status: u16,
    pub revision_id: u8,
    pub prog_if: u8,
    pub sub_class: u8,
    pub class_code: u8,
    pub cache_line_size: u8,
    pub latency_timer: u8,
    pub header_type: HeaderType,
    pub bist: u8,
}

bitfield! {
  struct HeaderType(u8);
  impl Debug;
  // The fields default to u16
  multi_function, _: 7;
  u8; header_type, _: 6, 0;
}

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

    let mut pci = unsafe { Pci::new() };
    for device in pci.devices() {
        log::debug!("Device: {device:#X?}");
    }

    // polished_pci::pci_enumeration_demo();
}
