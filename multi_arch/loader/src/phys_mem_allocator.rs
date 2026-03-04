use core::{
    fmt::Debug,
    iter::chain,
    marker::PhantomData,
    ops::{Add, RangeInclusive},
};

use fdt_raw::Fdt;
use num_traits::{Num, Zero};

pub trait MemInfo<PhysAddr> {
    fn memory(&self) -> impl IntoIterator<Item = RangeInclusive<PhysAddr>>;
    fn reserved_memory(&self) -> impl IntoIterator<Item = RangeInclusive<PhysAddr>>;
}

impl<'a, PhysAddr> MemInfo<PhysAddr> for Fdt<'a>
where
    PhysAddr: Copy,
    PhysAddr: TryFrom<u64>,
    PhysAddr::Error: Debug,
    PhysAddr: Add<Output = PhysAddr>,
{
    fn memory(&self) -> impl IntoIterator<Item = RangeInclusive<PhysAddr>> {
        self.memory()
            .flat_map(|memory| memory.reg().into_iter().flatten())
            .map(|reg| {
                let start = reg.address.try_into().unwrap();
                start..=start + (reg.size.unwrap() - 1).try_into().unwrap()
            })
    }

    fn reserved_memory(&self) -> impl IntoIterator<Item = RangeInclusive<PhysAddr>> {
        chain(
            self.memory_reservations().map(|reservation| {
                let start = reservation.address.try_into().unwrap();
                start..=start + (reservation.size - 1).try_into().unwrap()
            }),
            self.reserved_memory()
                .flat_map(|node| node.reg().into_iter().flatten())
                .map(|reg| {
                    let start = reg.address.try_into().unwrap();
                    start..=start + (reg.size.unwrap() - 1).try_into().unwrap()
                }),
        )
    }
}

pub struct AllocRequest<PhysAddr> {
    pub size: PhysAddr,
    pub align: PhysAddr,
}

/// Note that this allocator simply goes from left to right and can fail to allocate if there are gaps created.
/// It's recommended to keep the requested `align` the same every time.
pub struct PhysMemAllocator<PhysAddr, T: MemInfo<PhysAddr>> {
    mem_info: T,
    _phantom_data: PhantomData<PhysAddr>,
    // Which memory region we are allocating from
    memory_index: usize,
    memory_position: PhysAddr,
}

impl<PhysAddr: Add<Output = PhysAddr> + Zero, T: MemInfo<PhysAddr>> PhysMemAllocator<PhysAddr, T> {
    pub fn new(mem_info: T) -> Self {
        Self {
            mem_info,
            _phantom_data: PhantomData,
            memory_index: 0,
            memory_position: PhysAddr::zero(),
        }
    }

    pub fn allocate(&mut self, request: AllocRequest<PhysAddr>) -> Option<PhysAddr> {
        // self.mem_info
        //     .memory()
        //     .into_iter()
        //     .enumerate()
        //     .skip(self.memory_index)
        //     .find_map(|(memory_index, range)| {
        //         range = self.memory_position;
        //         todo!();
        //     });
        todo!()
    }
}
