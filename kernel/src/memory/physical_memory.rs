use nodit::{Interval, NoditMap};
use x86_64::{
    PhysAddr,
    structures::paging::{FrameAllocator, PageSize, PhysFrame},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum KernelMemoryUsageType {
    PageTables,
    GlobalAllocatorHeap,
}

/// Note that there are other memory types (such as ACPI memory) that are not included here
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MemoryType {
    Usable,
    UsedByLimine,
    UsedByKernel(KernelMemoryUsageType),
}

pub struct PhysicalMemory {
    pub(super) map: NoditMap<u64, Interval<u64>, MemoryType>,
}

unsafe impl<S: PageSize> FrameAllocator<S> for PhysicalMemory {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        let aligned_start = self.map.iter().find_map(|(interval, memory_type)| {
            if let MemoryType::Usable = memory_type {
                let aligned_start = interval.start().next_multiple_of(S::SIZE);
                let required_end_inclusive = aligned_start + (S::SIZE - 1);
                if required_end_inclusive <= interval.end() {
                    Some(aligned_start)
                } else {
                    None
                }
            } else {
                None
            }
        })?;
        let _ = self.map.insert_overwrite(
            (aligned_start..=aligned_start + (S::SIZE - 1)).into(),
            MemoryType::UsedByKernel(KernelMemoryUsageType::PageTables),
        );
        Some(PhysFrame::from_start_address(PhysAddr::new(aligned_start)).unwrap())
    }
}
