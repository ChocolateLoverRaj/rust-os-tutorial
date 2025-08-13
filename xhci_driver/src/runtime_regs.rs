use volatile::{
    VolatileFieldAccess,
    access::{NoAccess, ReadOnly, ReadWrite},
};

use crate::interrupter_regs::InterrupterRegs;

/// xHCI 5.5 Host Controller Runtime Registers
#[derive(Debug, VolatileFieldAccess)]
#[repr(C)]
pub struct RuntimeRegisters {
    #[access(ReadOnly)]
    pub mfindex: u32,
    #[access(NoAccess)]
    _reserved_0: [u8; 0x1C],
    #[access(ReadWrite)]
    interrupter_register_sets: [InterrupterRegs; 0x400],
}
