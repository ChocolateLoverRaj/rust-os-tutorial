use bitfield::bitfield;
use num_enum::TryFromPrimitive;

bitfield! {
    pub struct HeaderTypeByte(u8);
    impl Debug;
    // The fields default to u16
    pub multi_function, _: 7;
    u8; pub header_type, _: 6, 0;
}

#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
pub enum HeaderType {
    GeneralDevice = 0x0,
    PciToPciBridge = 0x1,
    PciToCardBusBridge = 0x2,
}

impl HeaderType {
    pub fn interrupt_reg_addr(&self) -> u8 {
        0x3C
    }
}
