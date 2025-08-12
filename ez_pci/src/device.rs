use core::ops::RangeInclusive;

use super::*;

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
            Some(PciFunction {
                pci: self.pci,
                bus_number: self.bus_number,
                device_number: self.device_number,
                function_number,
            })
        } else {
            None
        }
    }
}
