use bitfield::bitfield;
use debug_ignore::DebugIgnore;
use volatile::{
    VolatileFieldAccess,
    access::{NoAccess, ReadWrite},
};

/// xHCI 5.5.2 Interrupter Register Set
#[derive(Debug, VolatileFieldAccess, Clone, Copy)]
#[repr(C)]
pub struct InterrupterRegs {
    #[access(ReadWrite)]
    pub iman: Iman,
    #[access(ReadWrite)]
    pub imod: Imod,
    #[access(ReadWrite)]
    pub erstsz: Erstsz,
    #[access(NoAccess)]
    _reserved_0: DebugIgnore<[u8; 4]>,
    #[access(ReadWrite)]
    erstba: Erstba,
    #[access(ReadWrite)]
    erdp: Erdp,
}

bitfield! {
    /// xHCI 5.5.2.1 Interrupter Management Register (IMAN)
    #[derive(Clone, Copy)]
    pub struct Iman(u32);
    impl Debug;

    pub interrupt_pending, set_interrupt_pending: 0;
    pub interrupt_enable, set_interrupt_enable: 1;
}

bitfield! {
    /// xHCI 5.5.2.2 Interrupter Moderation Register (IMOD)
    #[derive(Clone, Copy)]
    pub struct Imod(u32);
    impl Debug;

    u16; pub imodi, set_imodi: 15, 0;
    u16; pub imodc, set_imodc: 31, 16;
}

bitfield! {
    /// xHCI 5.5.2.3.1 Event Ring Segment Table Size Register (ERSTSZ)
    #[derive(Clone, Copy)]
    pub struct Erstsz(u32);
    impl Debug;

    u16; pub erstsz, set_erstsz: 15, 0;
}

bitfield! {
    /// xHCI 5.5.2.3.2 Event Ring Segment Table Base Address Register (ERSTBA)
    #[derive(Clone, Copy)]
    pub struct Erstba(u64);
    impl Debug;

    u64; _erstba, _set_erstba: 63, 6;
}

impl Erstba {
    pub fn erstba(&self) -> u64 {
        self._erstba() << 6
    }

    pub fn set_erstba(&mut self, erstba: u64) {
        self._set_erstba(erstba >> 6);
    }
}

bitfield! {
    /// xHCI 5.5.2.3.3 Event Ring Dequeue Pointer Register (ERDP)
    #[derive(Clone, Copy)]
    pub struct Erdp(u64);
    impl Debug;

    u8; pub desi, set_desi: 2, 0;
    pub event_handler_busy, set_event_handler_busy: 3;
    u64; _event_ring_dequeue_pointer, _set_event_ring_dequeue_pointer: 63, 4;
}

impl Erdp {
    pub fn event_ring_dequeue_pointer(&self) -> u64 {
        self._event_ring_dequeue_pointer() << 4
    }

    pub fn set_event_ring_dequeue_pointer(&mut self, event_ring_dequeue_pointer: u64) {
        self._set_event_ring_dequeue_pointer(event_ring_dequeue_pointer >> 4);
    }
}
