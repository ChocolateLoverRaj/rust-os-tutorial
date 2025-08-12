use super::*;

pub struct PciBus<'a> {
    pub(super) pci: &'a mut PciAccess,
    pub(super) bus_number: u8,
}

impl PciBus<'_> {
    pub fn device(&mut self, device_number: u8) -> Option<PciDevice> {
        assert!((0..32).contains(&device_number));
        let vendor_id = self.pci.read_u32(self.bus_number, device_number, 0, 0x0) as u16;
        if vendor_id != u16::MAX {
            let multi_function = HeaderTypeByte(
                (self.pci.read_u32(self.bus_number, device_number, 0, 0xC) >> 16) as u8,
            )
            .multi_function();
            let pci_device = PciDevice {
                pci: self.pci,
                bus_number: self.bus_number,
                device_number,
                multi_function,
            };
            Some(pci_device)
        } else {
            None
        }
    }
}
