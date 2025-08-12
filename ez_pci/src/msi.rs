use core::fmt::Debug;

use bitfield::bitfield;

use super::*;

pub struct Msi<'a> {
    pci: &'a mut PciAccess,
    bus_number: u8,
    device_number: u8,
    function_number: u8,
    ptr: u8,
}

impl<'a> Msi<'a> {
    pub(super) fn find(function: &'a mut PciFunction) -> Option<Option<Self>> {
        if let Some(capability) = function
            .capabilities()?
            .find(|capability| capability.id == 0x5)
        {
            Some(Some(Self {
                pci: function.pci,
                bus_number: function.bus_number,
                device_number: function.device_number,
                function_number: function.function_number,
                ptr: capability.ptr_to_self,
            }))
        } else {
            Some(None)
        }
    }

    pub fn get_message_control(&mut self) -> MessageControlRegister {
        MessageControlRegister(self.pci.read_u16(
            self.bus_number,
            self.device_number,
            self.function_number,
            self.ptr + 0x2,
        ))
    }

    pub fn set_message_control(&mut self, message_control_register: MessageControlRegister) {
        self.pci.write_u16(
            self.bus_number,
            self.device_number,
            self.function_number,
            self.ptr + 0x2,
            message_control_register.0,
        )
    }

    #[deprecated = "You might misinterpret the address if 64-bit message address is supported"]
    pub fn get_message_addr_u32(&mut self) -> u32 {
        self.pci.read_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            self.ptr + 0x4,
        )
    }

    #[deprecated = "If 64-bit message address is supported and upper bits are not 0, then the effective address will be one that you didn't expect"]
    pub fn set_message_addr_u32(&mut self, addr: u32) {
        self.pci.write_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            self.ptr + 0x4,
            addr,
        );
    }

    /// Remember to check the message control register to see if a 64-bit message address is supported.
    #[deprecated = "You might accidentally read the wrong register if 64-bit message address is not supported. Open an issue if you need to get an address that's >u32::MAX."]
    pub fn get_message_addr_u64(&mut self) -> u64 {
        let low = self.pci.read_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            self.ptr + 0x4,
        );
        let high = self.pci.read_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            self.ptr + 0x8,
        );
        low as u64 | ((high as u64) << 32)
    }

    /// Remember to check the message control register to see if a 64-bit message address is supported.
    #[deprecated = "You might accidentally read the wrong register if 64-bit message address is not supported. Open an issue if you need to set an address to be >u32::MAX."]
    pub fn set_message_addr_u64(&mut self, addr: u64) {
        self.pci.write_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            self.ptr + 0x4,
            addr as u32,
        );
        self.pci.write_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            self.ptr + 0x8,
            (addr >> 32) as u32,
        );
    }

    /// Sets the address to a u32 address. This will work whether 64 bit addresses are supported or not.
    pub fn set_message_addr(&mut self, addr: u32) {
        if self.get_message_control().supports_64_bit_addresses() {
            self.pci.write_u32(
                self.bus_number,
                self.device_number,
                self.function_number,
                self.ptr + 0x4,
                addr,
            );
            self.pci.write_u32(
                self.bus_number,
                self.device_number,
                self.function_number,
                self.ptr + 0x8,
                0,
            );
        } else {
            self.pci.write_u32(
                self.bus_number,
                self.device_number,
                self.function_number,
                self.ptr + 0x4,
                addr,
            );
        }
    }

    fn get_message_data_offset(&mut self) -> u8 {
        if self.get_message_control().supports_64_bit_addresses() {
            0xC
        } else {
            0x8
        }
    }

    pub fn get_message_data(&mut self) -> u16 {
        let message_data_offset = self.get_message_data_offset();
        self.pci.read_u16(
            self.bus_number,
            self.device_number,
            self.function_number,
            self.ptr + message_data_offset,
        )
    }

    /// Note that if you enable multiple interrupts in the message control register, the PCI function will override the lowest N bits of the message data when writing the message data to the message address.
    /// This effectively lets you assign multiple interrupt vectors to a PCI function.
    /// This is useful for balancing interrupts between multiple CPUs.
    /// If you only want the PCI function to send interrupts to 1 interrupt vector, make sure to set the `multiple_message_enable` to `0b000`.
    pub fn set_message_data(&mut self, message_data: u16) {
        let message_data_offset = self.get_message_data_offset();
        self.pci.write_u16(
            self.bus_number,
            self.device_number,
            self.function_number,
            self.ptr + message_data_offset,
            message_data,
        )
    }
}

impl Debug for Msi<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MSI")
            .field("ptr", &format_args!("0x{:X}", self.ptr))
            .finish()
    }
}

bitfield! {
    pub struct MessageControlRegister(u16);
    impl Debug;

    /// If this is 1, you can use the MSI mask and pending registers.
    pub per_message_masking, _: 8;
    /// If this is 1, you can set the message address to a 64 bit address.
    /// If this is 0, you can only set the message address to a 32-bit address.
    pub supports_64_bit_addresses, _: 7;
    u8; pub multiple_message_enable, set_multiple_message_enable: 6, 4;
    u8; pub multiple_message_capable, _: 3, 1;
    pub enable, set_enable: 0;
}

bitfield! {
    /// See Intel SDM -> Volume 3 -> 12.11.1 Message Address Register Format
    pub struct ApicMsiMessageAddress(u32);
    impl Debug;

    u16;
    /// If this is 1, you can use the MSI mask and pending registers.
    fixed_value, set_fixed_value: 31, 20;
    u8;
    /// If this is 1, you can set the message address to a 64 bit address.
    /// If this is 0, you can only set the message address to a 32-bit address.
    destination_id, set_destination_id: 19, 12;
    u8;
    /// Can be set to efficiently distribute the interrupt to a less-busy CPU.
    /// However I don't understand this so read the SDM if you want this.
    redirection_hint, set_redirection_hint: 3;
    u8;
    /// Only applies if redirection hint (RH) is enabled.
    /// I don't understand this so read the SDM if you want this.
    destination_mode, set_destination_mode: 2;
}

impl Default for ApicMsiMessageAddress {
    fn default() -> Self {
        let mut address = Self(0);
        address.set_fixed_value(0xFEE);
        address
    }
}

bitfield! {
    /// See Intel SDM -> Volume 3 -> 12.11.2 Message Data Register Format
    pub struct ApicMsiMessageData(u16);
    impl Debug;

    u8; pub trigger_mode, set_trigger_mode: 15;
    u8; pub trigger_mode_level, set_trigger_mode_level: 14;
    u8; pub delivery_mode, set_delivery_mode: 10, 8;
    u8; pub vector, set_vector: 7, 0;
}
