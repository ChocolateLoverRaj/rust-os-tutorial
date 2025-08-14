use volatile::{
    VolatileFieldAccess,
    access::{NoAccess, ReadWrite},
};

/// xHCI 6.5 Event Ring Segment Table
#[derive(Debug, VolatileFieldAccess)]
#[repr(C)]
pub struct XhciErstEntry {
    #[access(ReadWrite)]
    pub ring_segment_base_address: u64,
    #[access(ReadWrite)]
    pub ring_segment_size: u16,
    #[access(NoAccess)]
    pub _reserved_0: [u8; 6],
}
