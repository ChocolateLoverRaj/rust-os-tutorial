use num_enum::IntoPrimitive;

#[derive(Debug, IntoPrimitive)]
#[repr(u8)]
pub enum InterruptVector {
    LocalApicSpurious = 0x20,
    LocalApicTimer,
    LocalApicError,
    Keyboard,
    Mouse,
    /// An IPI for when a multi-thread process exits while other threads are still running,
    /// or when an external interrupt causes CPUs to have to switch which task they will do
    Preempt,
    /// IPI - tells CPUs to maybe switch threads, depending on priorities
    CheckTasks,
}
