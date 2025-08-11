use crate::pci::{
    BarWithSize, IoBarInfo, MemoryBarAddrAndSize, MemoryBarAddrAndSizeU32, MemoryBarAddrAndSizeU64,
    MemoryBarInfo, MemorySpaceBar,
};

use super::{BarCommon, HeaderType, HeaderTypeByte, PciAccess};

#[derive(Debug)]
pub struct PciFunction<'a> {
    pub(super) pci: &'a mut PciAccess,
    pub(super) bus_number: u8,
    pub(super) device_number: u8,
    pub(super) function_number: u8,
    pub(super) vendor_id: u16,
    pub(super) device_id: u16,
    pub(super) command: u16,
    pub(super) status: u16,
    pub(super) revision_id: u8,
    pub(super) prog_if: u8,
    pub(super) sub_class: u8,
    pub(super) class_code: u8,
    pub(super) cache_line_size: u8,
    pub(super) latency_timer: u8,
    pub(super) header_type: HeaderTypeByte,
    pub(super) bist: u8,
}

impl PciFunction<'_> {
    pub fn header_type(&self) -> Option<HeaderType> {
        self.header_type.header_type().try_into().ok()
    }

    pub fn max_bars(&self) -> u8 {
        match self.header_type().unwrap() {
            HeaderType::GeneralDevice => 6,
            HeaderType::PciToPciBridge => 2,
            HeaderType::PciToCardBusBridge => 0,
        }
    }

    pub fn read_bar_with_size(&mut self, bar_index: u8) -> Option<BarWithSize> {
        assert!((0..self.max_bars()).contains(&bar_index));
        let register_offset = 0x10 + size_of::<u32>() as u8 * bar_index;
        let raw_addr = self.pci.read_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            register_offset,
        );
        if raw_addr == 0 {
            return None;
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
        Some(if BarCommon(raw_addr).bar_type() == 0x0 {
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
        })
    }
}
