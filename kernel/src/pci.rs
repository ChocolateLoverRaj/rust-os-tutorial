use core::ops::RangeInclusive;

use acpi::{AcpiHandler, AcpiTables, mcfg::Mcfg};
use bitfield::{bitfield, bitfield_constructor};
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

    pub fn pcie(mcfg: &Mcfg) {
        for entry in mcfg.entries() {
            log::debug!("Entry: {entry:#X?}");
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

    pub fn device(&mut self, device_number: u8) -> Option<PciDevice> {
        assert!((0..32).contains(&device_number));
        let vendor_id = self.pci_config_read_u32(0, device_number, 0, 0x0) as u16;
        if vendor_id != u16::MAX {
            let multi_function =
                HeaderType((self.pci_config_read_u32(0, device_number, 0, 0xC) >> 16) as u8)
                    .multi_function();
            let pci_device = PciDevice {
                pci: self,
                index: device_number,
                multi_function,
            };
            Some(pci_device)
        } else {
            None
        }
    }
}
pub struct PciDevice<'a> {
    pci: &'a mut Pci,
    index: u8,
    multi_function: bool,
}

impl PciDevice<'_> {
    pub fn possible_functions(&self) -> RangeInclusive<u8> {
        if self.multi_function { 0..=7 } else { 0..=0 }
    }

    pub fn function(&mut self, function_number: u8) -> Option<PciFunction> {
        assert!((0..=7).contains(&function_number));
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
            Some(PciFunction {
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
            })
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct PciFunction {
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

pub fn init(acpi_tables: &AcpiTables<impl AcpiHandler>) {
    if let Ok(mcfg) = acpi_tables.find_table::<Mcfg>() {
        Pci::pcie(&mcfg);
    } else {
        let mut pci = unsafe { Pci::new() };
        for device_number in 0..32 {
            if let Some(mut device) = pci.device(device_number) {
                for function_number in device.possible_functions() {
                    if let Some(function) = device.function(function_number) {
                        log::debug!(
                            "Device: {device_number}. Function: {function_number}. {function:#X?}"
                        );
                    }
                }
            }
        }
    }
}
