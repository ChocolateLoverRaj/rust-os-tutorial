use core::num::NonZero;

use crate::pci::MemorySpaceBar;

use super::{BarCommon, FullBar, HeaderType, HeaderTypeByte, PciAccess};

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

    fn read_bar_raw(&mut self, bar_index: u8) -> u32 {
        assert!((0..self.max_bars()).contains(&bar_index));
        self.pci.read_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            0x10 + size_of::<u32>() as u8 * bar_index,
        )
    }

    pub fn read_bar(&mut self, bar_index: u8) -> Option<FullBar> {
        let bar0 = NonZero::new(self.read_bar_raw(bar_index))?.get();
        Some(if BarCommon(bar0).bar_type() == 0x0 {
            let bar = MemorySpaceBar(bar0);
            match bar._type() {
                0x0 => FullBar {
                    bar: BarCommon(bar0),
                    next_bar: Default::default(),
                },
                0x2 => FullBar {
                    bar: BarCommon(bar0),
                    next_bar: self.read_bar_raw(bar_index + 1),
                },
                bar_type => panic!("unsupported bar type: 0x{bar_type:X}"),
            }
        } else {
            FullBar {
                bar: BarCommon(bar0),
                next_bar: Default::default(),
            }
        })
    }
}
