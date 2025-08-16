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
    /// Note that PCI devices can actually send interrupts to >1 interrupt vector.
    /// To maximize performance, more interrupt vectors should be used for PCI, and they should be distributed to multiple CPUs.
    /// However for now we will just use a single vector.
    Pci,
    Hpet,
}
