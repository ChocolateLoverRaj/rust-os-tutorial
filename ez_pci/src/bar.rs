use core::fmt::Debug;

use bitfield::bitfield;

bitfield! {
    #[derive( Clone, Copy)]
  pub struct BarCommon(u32);
  impl Debug;
  u8; pub(super) bar_type, _: 1, 1;
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct MemorySpaceBar(u32);
  impl Debug;
    pub(super) prefetchable, _: 3;
    u8; pub(super) _type, _: 2, 1;
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct IoSpaceBar(u32);
}

impl IoSpaceBar {
    pub fn addr(self) -> u32 {
        // The lowest 2 bits should be masked out
        self.0 & !0b11
    }
}

impl Debug for IoSpaceBar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IoSpaceBar")
            .field("addr", &format_args!("0x{:X}", self.addr()))
            .finish()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryBarAddrAndSizeU32 {
    pub addr: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryBarAddrAndSizeU64 {
    pub addr: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum MemoryBarAddrAndSize {
    U32(MemoryBarAddrAndSizeU32),
    U64(MemoryBarAddrAndSizeU64),
}

impl MemoryBarAddrAndSize {
    /// Get the address as a `u64` regardless of whether this is a 32-bit or 64-bit address.
    pub fn addr_u64(&self) -> u64 {
        match self {
            Self::U32(addr_and_size) => addr_and_size.addr as u64,
            Self::U64(addr_and_size) => addr_and_size.addr,
        }
    }

    /// Get the size as a `u64` regardless of whether this is a 32-bit or 64-bit address.
    pub fn size_u64(&self) -> u64 {
        match self {
            Self::U32(addr_and_size) => addr_and_size.size as u64,
            Self::U64(addr_and_size) => addr_and_size.size,
        }
    }

    pub fn addr_and_size_u64(self) -> MemoryBarAddrAndSizeU64 {
        match self {
            Self::U32(addr_and_size) => MemoryBarAddrAndSizeU64 {
                addr: addr_and_size.addr as u64,
                size: addr_and_size.size as u64,
            },
            Self::U64(addr_and_size) => addr_and_size,
        }
    }
}

#[derive(Debug)]
pub struct MemoryBarInfo {
    pub addr_and_size: MemoryBarAddrAndSize,
    /// CPUs can pre-fetch memory, which can result in memory being fetched earlier than your code reads it, fetched multiple times, or memory that your code doesn't read being fetched.
    /// Pre-fetching memory is good for performance, but can cause bad side-effects if the memory is not prefetchable.
    ///
    /// If this is `false`, then the mem type should be UC (strong uncacheable).
    /// If this is `true`, then the mem type should be WT (write-through) for most use cases, and WC (write-combining) for frame buffers.
    pub prefetchable: bool,
}

#[derive(Debug)]
pub struct IoBarInfo {
    pub addr: u32,
    pub size: u32,
}

#[derive(Debug)]
pub enum BarWithSize {
    Memory(MemoryBarInfo),
    Io(IoBarInfo),
}

impl BarWithSize {
    /// How many BAR slots this bar takes up. 64-bit memory addresses use up 2 BAR slots
    pub fn slots_len(&self) -> u8 {
        match self {
            Self::Memory(memory_bar_info) => match memory_bar_info.addr_and_size {
                MemoryBarAddrAndSize::U32(_) => 1,
                MemoryBarAddrAndSize::U64(_) => 2,
            },
            BarWithSize::Io(_) => 1,
        }
    }
}
