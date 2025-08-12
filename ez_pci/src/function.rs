use super::*;

#[derive(Debug)]
pub struct PciFunction<'a> {
    pub(super) pci: &'a mut PciAccess,
    pub(super) bus_number: u8,
    pub(super) device_number: u8,
    pub(super) function_number: u8,
}

impl PciFunction<'_> {
    pub fn vendor_id(&mut self) -> u16 {
        self.pci.read_u16(
            self.bus_number,
            self.device_number,
            self.function_number,
            0x0,
        )
    }

    pub fn device_id(&mut self) -> u16 {
        self.pci.read_u16(
            self.bus_number,
            self.device_number,
            self.function_number,
            0x2,
        )
    }

    pub fn class_code(&mut self) -> u8 {
        (self.pci.read_u16(
            self.bus_number,
            self.device_number,
            self.function_number,
            0xA,
        ) >> 8) as u8
    }

    pub fn sub_class(&mut self) -> u8 {
        self.pci.read_u16(
            self.bus_number,
            self.device_number,
            self.function_number,
            0xA,
        ) as u8
    }

    pub fn prog_if(&mut self) -> u8 {
        (self.pci.read_u16(
            self.bus_number,
            self.device_number,
            self.function_number,
            0x8,
        ) >> 8) as u8
    }

    pub fn header_type_byte(&mut self) -> HeaderTypeByte {
        HeaderTypeByte(self.pci.read_u16(
            self.bus_number,
            self.device_number,
            self.function_number,
            0xE,
        ) as u8)
    }

    /// Returns `None` if the header type is not known
    pub fn header_type(&mut self) -> Option<HeaderType> {
        self.header_type_byte().header_type().try_into().ok()
    }

    /// Returns `None` if the header type is not known
    pub fn max_bars(&mut self) -> Option<u8> {
        Some(match self.header_type()? {
            HeaderType::GeneralDevice => 6,
            HeaderType::PciToPciBridge => 2,
            HeaderType::PciToCardBusBridge => 0,
        })
    }

    /// Returns `None` if header type is not known.
    /// Returns `Some(None)` if the bar is not present
    pub fn read_bar_with_size(&mut self, bar_index: u8) -> Option<Option<BarWithSize>> {
        assert!((0..self.max_bars()?).contains(&bar_index));
        let register_offset = 0x10 + size_of::<u32>() as u8 * bar_index;
        let raw_addr = self.pci.read_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            register_offset,
        );
        if raw_addr == 0 {
            return Some(None);
        }
        self.pci.write_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            register_offset,
            u32::MAX,
        );
        let raw_size = self.pci.read_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            register_offset,
        );
        self.pci.write_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            register_offset,
            raw_addr,
        );
        Some(Some(if BarCommon(raw_addr).bar_type() == 0x0 {
            BarWithSize::Memory(MemoryBarInfo {
                addr_and_size: match MemorySpaceBar(raw_addr)._type() {
                    0x0 => MemoryBarAddrAndSize::U32(MemoryBarAddrAndSizeU32 {
                        addr: raw_addr & !0b1111,
                        size: (!(raw_size & !0b1111)).wrapping_add(1),
                    }),
                    0x2 => {
                        let register_offset = 0x10 + size_of::<u32>() as u8 * (bar_index + 1);
                        let next_raw_addr = self.pci.read_u32(
                            self.bus_number,
                            self.device_number,
                            self.function_number,
                            register_offset,
                        );
                        self.pci.write_u32(
                            self.bus_number,
                            self.device_number,
                            self.function_number,
                            register_offset,
                            u32::MAX,
                        );
                        let next_raw_size = self.pci.read_u32(
                            self.bus_number,
                            self.device_number,
                            self.function_number,
                            register_offset,
                        );
                        self.pci.write_u32(
                            self.bus_number,
                            self.device_number,
                            self.function_number,
                            register_offset,
                            next_raw_addr,
                        );
                        MemoryBarAddrAndSize::U64(MemoryBarAddrAndSizeU64 {
                            addr: (raw_addr & !0b1111) as u64 | (next_raw_addr as u64) << 32,
                            size: (!((raw_size & !0b1111) as u64 | (next_raw_size as u64) << 32))
                                .wrapping_add(1),
                        })
                    }
                    _ => unreachable!(),
                },
                prefetchable: MemorySpaceBar(raw_addr).prefetchable(),
            })
        } else {
            BarWithSize::Io(IoBarInfo {
                addr: raw_addr & !0b11,
                size: (!(raw_size & !0b11)).wrapping_add(1),
            })
        }))
    }

    /// Returns `None` if header type is unknown
    pub fn interrupt_info(&mut self) -> Option<InterruptInfo> {
        let register_offset = self.header_type()?.interrupt_reg_addr();
        let reg = self.pci.read_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            register_offset,
        );
        Some(InterruptInfo {
            interrupt_pin: (reg >> 8) as u8,
            interrupt_line: reg as u8,
        })
    }

    /// Returns `None` if the header type is unknown
    pub fn capabilities(&mut self) -> Option<Capabilities> {
        let register_offset = match self.header_type()? {
            HeaderType::GeneralDevice => 0x34,
            HeaderType::PciToPciBridge => 0x34,
            HeaderType::PciToCardBusBridge => 0x14,
        };
        Some(Capabilities {
            bus_number: self.bus_number,
            device_number: self.device_number,
            function_number: self.function_number,
            ptr: self.pci.read_u32(
                self.bus_number,
                self.device_number,
                self.function_number,
                register_offset,
            ) as u8,
            pci: self.pci,
        })
    }

    /// # Important
    /// Writing to this will not actually change the IRQ number that this gets routed to.
    /// The firmware writes to the interrupt line to indicate to the OS which one it is.
    /// So the interrupt line should be treated as read-only by the OS.
    ///
    /// Returns `None` if the header type is unknown
    pub fn set_interrupt_line(&mut self, interrupt_line: u8) -> Option<()> {
        let register_offset = self.header_type()?.interrupt_reg_addr();
        let current_reg = self.pci.read_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            register_offset,
        );
        let new_reg = current_reg & !0xFF | interrupt_line as u32;
        self.pci.write_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            register_offset,
            new_reg,
        );
        Some(())
    }

    pub fn msi(&mut self) -> Option<Option<Msi>> {
        Msi::find(self)
    }
}

#[derive(Debug)]
pub struct InterruptInfo {
    pub interrupt_pin: u8,
    pub interrupt_line: u8,
}
