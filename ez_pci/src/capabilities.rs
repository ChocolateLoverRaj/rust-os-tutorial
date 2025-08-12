use super::*;

pub struct Capabilities<'a> {
    pub(super) pci: &'a mut PciAccess,
    pub(super) bus_number: u8,
    pub(super) device_number: u8,
    pub(super) function_number: u8,
    pub(super) ptr: u8,
}

impl Iterator for Capabilities<'_> {
    type Item = Capability;
    fn next(&mut self) -> Option<Self::Item> {
        if self.ptr == 0 {
            return None;
        }
        let reg = self.pci.read_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            self.ptr,
        );
        let capability = Capability {
            ptr_to_self: self.ptr,
            id: reg as u8,
            next_ptr: (reg << 8) as u8,
        };
        self.ptr = capability.next_ptr;
        Some(capability)
    }
}

#[derive(Debug)]
pub struct Capability {
    pub ptr_to_self: u8,
    pub id: u8,
    /// The offset in the function's memory where the next capability is
    pub next_ptr: u8,
}
