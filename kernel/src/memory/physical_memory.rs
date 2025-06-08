use alloc::boxed::Box;
use nodit::{Interval, NoditMap};
use x86_64::{
    PhysAddr,
    structures::paging::{FrameAllocator, PageSize, PhysFrame},
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum KernelMemoryUsageType {
    PageTables,
    GlobalAllocatorHeap,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UserModeMemoryUsageType {
    PageTables,
    Elf,
    Stack,
    /// Memory that the user mode program requested in run time
    Heap,
}

/// Note that there are other memory types (such as ACPI memory) that are not included here
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MemoryType {
    Usable,
    UsedByLimine,
    UsedByKernel(KernelMemoryUsageType),
    UsedByUserMode(UserModeMemoryUsageType),
}

pub struct PhysicalMemory {
    pub(super) map: NoditMap<u64, Interval<u64>, MemoryType>,
}

impl PhysicalMemory {
    pub fn allocate_frame_with_type<S: PageSize>(
        &mut self,
        memory_type: MemoryType,
    ) -> Option<PhysFrame<S>> {
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
        let range = aligned_start..=aligned_start + (S::SIZE - 1);
        let _ = self.map.cut(Interval::from(range.clone()));
        self.map
            .insert_merge_touching_if_values_equal(range.into(), memory_type)
            .unwrap();
        Some(PhysFrame::from_start_address(PhysAddr::new(aligned_start)).unwrap())
    }

    pub fn get_kernel_frame_allocator(&mut self) -> PhysicalMemoryFrameAllocator<'_> {
        PhysicalMemoryFrameAllocator {
            physical_memory: self,
            memory_type: MemoryType::UsedByKernel(KernelMemoryUsageType::PageTables),
        }
    }

    pub fn get_user_mode_program_frame_allocator(&mut self) -> PhysicalMemoryFrameAllocator<'_> {
        PhysicalMemoryFrameAllocator {
            physical_memory: self,
            memory_type: MemoryType::UsedByUserMode(UserModeMemoryUsageType::PageTables),
        }
    }

    /// Marks all user mode memory as unused
    pub fn remove_user_mode_memory(&mut self) {
        let intervals_to_remove = self
            .map
            .iter()
            .filter_map(|(interval, memory_type)| {
                if let MemoryType::UsedByUserMode(_) = memory_type {
                    Some(*interval)
                } else {
                    None
                }
            })
            .collect::<Box<[_]>>();
        for interval in intervals_to_remove {
            let _ = self.map.cut(interval);
            self.map
                .insert_merge_touching_if_values_equal(interval, MemoryType::Usable)
                .unwrap();
        }
    }
}

pub struct PhysicalMemoryFrameAllocator<'a> {
    physical_memory: &'a mut PhysicalMemory,
    memory_type: MemoryType,
}

unsafe impl<S: PageSize> FrameAllocator<S> for PhysicalMemoryFrameAllocator<'_> {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        self.physical_memory
            .allocate_frame_with_type(self.memory_type)
    }
}
