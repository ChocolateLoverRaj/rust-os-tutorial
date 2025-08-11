use num_enum::IntoPrimitive;

#[derive(Debug, IntoPrimitive)]
#[repr(u8)]
pub enum InterruptVector {
    LocalApicSpurious = 0x20,
    LocalApicTimer,
    LocalApicError,
    Keyboard,
    Mouse,
    /// IPI - tells CPUs to maybe switch threads, depending on priorities
    CheckTasks,
    /// IPI - tells CPUs to flush their TLB
    FlushTlb,
    PciIntA,
}
