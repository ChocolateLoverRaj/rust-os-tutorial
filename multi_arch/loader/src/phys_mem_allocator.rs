use core::{
    fmt::Debug,
    iter::chain,
    marker::PhantomData,
    ops::{Add, RangeInclusive, Sub},
};

use fdt_raw::Fdt;
use log::info;
use num_traits::{CheckedAdd, CheckedRem, One, Zero};

use crate::align::checked_align_up;

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

#[derive(Debug, Clone, Copy)]
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
    offset_in_chunk: PhysAddr,
}

impl<
    PhysAddr: Debug
        + Add<Output = PhysAddr>
        + Zero
        + One
        + CheckedAdd<Output = PhysAddr>
        + CheckedRem<Output = PhysAddr>
        + Sub<Output = PhysAddr>
        + Ord
        + Copy
        + Eq,
    T: MemInfo<PhysAddr>,
> PhysMemAllocator<PhysAddr, T>
{
    pub fn new(mem_info: T) -> Self {
        Self {
            mem_info,
            _phantom_data: PhantomData,
            memory_index: 0,
            offset_in_chunk: PhysAddr::zero(),
        }
    }

    pub fn allocate(&mut self, request: AllocRequest<PhysAddr>) -> Option<PhysAddr> {
        let memory_index = self.memory_index;
        let offset_in_chunk = self.offset_in_chunk;
        info!("memory_index = {memory_index:#X} offset_in_chunk = {offset_in_chunk:#X?}");
        let mut iterator = self.mem_info.memory().into_iter().skip(self.memory_index);
        let mut chunk = iterator.next()?;
        let mut relative_chunk_index = 0;
        let mut offset_in_chunk = self.offset_in_chunk;
        'find: loop {
            let start = checked_align_up(*chunk.start() + offset_in_chunk, request.align).unwrap();
            // Check that the region to be allocated is within the usable memory range
            let end_inclusive = start
                .checked_add(&(request.size - PhysAddr::one()))
                .unwrap();
            info!("start = {start:#X?} end_inclusive = {end_inclusive:#X?} chunk = {chunk:#X?}");
            if end_inclusive > *chunk.end() {
                // Next chunk
                info!("next chunk");
                chunk = iterator.next()?;
                relative_chunk_index += 1;
                offset_in_chunk = PhysAddr::zero();
                continue;
            }
            info!("checking reserved regions");
            // Make sure that the region to be allocated is not reserved
            for reserved in self.mem_info.reserved_memory() {
                if *reserved.start() <= start && *reserved.end() >= start {
                    offset_in_chunk = *reserved.end() - *chunk.start() + PhysAddr::one();
                    continue 'find;
                }
            }
            // Mark as used
            self.memory_index += relative_chunk_index;
            self.offset_in_chunk = start - *chunk.start() + request.size;
            break Some(start);
        }
    }
}
