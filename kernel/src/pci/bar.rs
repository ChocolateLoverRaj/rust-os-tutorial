use core::fmt::Debug;

use bitfield::bitfield;

#[derive(Clone, Copy)]
pub struct FullBarMemory {
    bar: MemorySpaceBar,
    next_bar: u32,
}

impl FullBarMemory {
    /// Get the address as a u64, whether it's a 32 bit or 64 bit address
    pub fn addr_u64(&self) -> u64 {
        match self.bar._type() {
            0x0 => (self.bar.0 & !0b1111) as u64,
            0x2 => (self.bar.0 & !0b1111) as u64 | (self.next_bar as u64) << 32,
            _ => unreachable!(),
        }
    }

    pub fn width_bits(&self) -> u8 {
        match self.bar._type() {
            0x0 => 32,
            0x2 => 64,
            _ => unreachable!(),
        }
    }
}

impl Debug for FullBarMemory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MemorySpaceBar")
            .field("width_bits", &self.width_bits())
            .field("addr", &format_args!("0x{:X}", self.addr_u64()))
            .field("prefetchable", &self.bar.prefetchable())
            .finish()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FullBarType {
    Memory(FullBarMemory),
    Io(IoSpaceBar),
}

#[derive(Clone, Copy)]
pub struct FullBar {
    pub(super) bar: BarCommon,
    pub(super) next_bar: u32,
}

impl Debug for FullBar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.get_enum().fmt(f)
    }
}

impl FullBar {
    pub fn get_enum(&self) -> FullBarType {
        if self.bar.bar_type() == 0x0 {
            FullBarType::Memory(FullBarMemory {
                bar: MemorySpaceBar(self.bar.0),
                next_bar: self.next_bar,
            })
        } else {
            FullBarType::Io(IoSpaceBar(self.bar.0))
        }
    }

    /// The number of BARs that make up this BAR
    pub fn slots_len(&self) -> u8 {
        if self.bar.bar_type() == 0x0 {
            if MemorySpaceBar(self.bar.0)._type() == 0x2 {
                2
            } else {
                1
            }
        } else {
            1
        }
    }
}

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
