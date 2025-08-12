use core::ops::RangeInclusive;

use crate::pci::{HeaderTypeByte, PciFunction};

use super::PciAccess;

pub struct PciDevice<'a> {
    pub(super) pci: &'a mut PciAccess,
    pub(super) bus_number: u8,
    pub(super) device_number: u8,
    pub(super) multi_function: bool,
}

impl PciDevice<'_> {
    pub fn possible_functions(&self) -> RangeInclusive<u8> {
        if self.multi_function { 0..=7 } else { 0..=0 }
    }

    pub fn function(&mut self, function_number: u8) -> Option<PciFunction> {
        assert!((0..=7).contains(&function_number));
        let vendor_id =
            self.pci
                .read_u16(self.bus_number, self.device_number, function_number, 0x0);
        if vendor_id != u16::MAX {
            let (command, status) = {
                let reg =
                    self.pci
                        .read_u32(self.bus_number, self.device_number, function_number, 0x4);
                (reg as u16, (reg >> 16) as u16)
            };
            let (cache_line_size, latency_timer, header_type, bist) = {
                let reg =
                    self.pci
                        .read_u32(self.bus_number, self.device_number, function_number, 0xC);
                (
                    reg as u8,
                    (reg >> 8) as u8,
                    HeaderTypeByte((reg >> 16) as u8),
                    (reg >> 24) as u8,
                )
            };
            Some(PciFunction {
                pci: self.pci,
                bus_number: self.bus_number,
                device_number: self.device_number,
                function_number,
                command,
                status,
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
