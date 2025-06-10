use num_enum::IntoPrimitive;

/// <https://wiki.osdev.org/Interrupts#Standard_ISA_IRQs>
#[derive(IntoPrimitive)]
#[repr(u8)]
pub enum Pic8259Interrupts {
    Timer,
    Keyboard,
    Rtc = 8,
}
