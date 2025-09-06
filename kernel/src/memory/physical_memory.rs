use ez_paging::{Frame, Owned4KibFrame, PageSize};
use limine::{memory_map::EntryType, response::MemoryMapResponse};
use nodit::{Interval, NoditMap};
use x86_64::{
    PhysAddr,
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
};

use super::global_allocator;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum KernelMemoryUsageType {
    PageTables,
    GlobalAllocatorHeap,
}

/// Note that there are other memory types (such as ACPI memory) that are not included here
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MemoryType {
    Usable,
    UsedByLimine,
    UsedByKernel(KernelMemoryUsageType),
}

#[derive(Debug)]
pub struct PhysicalMemory {
    map: NoditMap<u64, Interval<u64>, MemoryType>,
}

impl PhysicalMemory {
    pub(super) fn new(
        memory_map: &'static MemoryMapResponse,
        global_allocator_start: PhysAddr,
    ) -> Self {
        Self {
            map: {
                let mut map = NoditMap::default();
                // We start with the state when Limine booted our kernel
                for entry in memory_map.entries() {
                    let should_insert = match entry.entry_type {
                        EntryType::USABLE => Some(MemoryType::Usable),
                        EntryType::BOOTLOADER_RECLAIMABLE => Some(MemoryType::UsedByLimine),
                        _ => {
                            // The entry might overlap, so let's not add it
                            None
                        }
                    };
                    if let Some(memory_type) = should_insert {
                        map
                            // Although they are guaranteed to not overlap and be ascending, Limine doesn't specify that they aren't guaranteed to not be touching even if they are the same.
                            .insert_merge_touching_if_values_equal(
                                (entry.base..entry.base + entry.length).into(),
                                memory_type,
                            )
                            .unwrap();
                    }
                }
                // We track the memory used for the global allocator
                let interval = Interval::from(
                    global_allocator_start.as_u64()
                        ..global_allocator_start.as_u64() + global_allocator::GLOBAL_ALLOCATOR_SIZE,
                );
                let _ = map.cut(interval);
                map.insert_merge_touching_if_values_equal(
                    interval,
                    MemoryType::UsedByKernel(KernelMemoryUsageType::GlobalAllocatorHeap),
                )
                .unwrap();
                map
            },
        }
    }

    pub fn allocate_frame_with_type(
        &mut self,
        page_size: PageSize,
        memory_type: MemoryType,
    ) -> Option<Frame> {
        let aligned_start = self.map.iter().find_map(|(interval, memory_type)| {
            if let MemoryType::Usable = memory_type {
                let aligned_start = interval.start().next_multiple_of(page_size.byte_len_u64());
                let required_end = aligned_start + page_size.byte_len_u64();
                if required_end <= interval.end() {
                    Some(aligned_start)
                } else {
                    None
                }
            } else {
                None
            }
        })?;
        let range = aligned_start..aligned_start + page_size.byte_len_u64();
        let _ = self.map.cut(Interval::from(range.clone()));
        self.map
            .insert_merge_touching_if_values_equal(range.into(), memory_type)
            .unwrap();
        Some(Frame::new(PhysAddr::new(aligned_start), page_size).unwrap())
    }

    pub fn get_kernel_frame_allocator(&mut self) -> PhysicalMemoryFrameAllocator<'_> {
        PhysicalMemoryFrameAllocator {
            physical_memory: self,
            memory_type: MemoryType::UsedByKernel(KernelMemoryUsageType::PageTables),
        }
    }
}

pub struct PhysicalMemoryFrameAllocator<'a> {
    physical_memory: &'a mut PhysicalMemory,
    memory_type: MemoryType,
}

impl PhysicalMemoryFrameAllocator<'_> {
    pub fn allocate_4kib_frame(&mut self) -> Option<Owned4KibFrame> {
        let frame = self
            .physical_memory
            .allocate_frame_with_type(PageSize::_4KiB, self.memory_type)?;
        let frame = PhysFrame::from_start_address(frame.start_addr()).unwrap();
        // Safety: we exclusively access the frame
        let frame = unsafe { Owned4KibFrame::new(frame) };
        Some(frame)
    }
}

unsafe impl FrameAllocator<Size4KiB> for PhysicalMemoryFrameAllocator<'_> {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        Some(self.allocate_4kib_frame()?.into())
    }
}
