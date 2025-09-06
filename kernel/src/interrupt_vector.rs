use num_enum::IntoPrimitive;

#[derive(Debug, IntoPrimitive)]
#[repr(u8)]
#[allow(clippy::enum_variant_names)]
pub enum InterruptVector {
    LocalApicSpurious = 0x20,
    LocalApicTimer,
    LocalApicError,
}
