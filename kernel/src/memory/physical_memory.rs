use core::{mem, num::NonZero};

use alloc::boxed::Box;
use common::PageSize;
use nodit::{InclusiveInterval, Interval, NoditMap, interval::ie};
use x86_64::{
    PhysAddr,
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
};

use crate::{Frame, task::ProcessId};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum KernelMemoryUsageType {
    PageTables,
    GlobalAllocatorHeap,
    Stack,
}

/// Note that there are other memory types (such as ACPI memory) that are not included here
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MemoryType {
    Usable,
    UsedByLimine,
    UsedByKernel(KernelMemoryUsageType),
    UsedByUserMode(ProcessId),
    Shared(u64),
    UsedByXhci,
}

pub struct PhysicalMemory {
    pub(super) map: NoditMap<u64, Interval<u64>, MemoryType>,
}

impl PhysicalMemory {
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

    /// Useful for use with PCI devices which may need memory smaller than a page or have a boundary requirement, such as xHCI.
    pub fn allocate(
        &mut self,
        size: NonZero<u64>,
        align: NonZero<u64>,
        boundary: NonZero<u64>,
        memory_type: MemoryType,
    ) -> Option<PhysAddr> {
        assert!(size <= boundary);
        assert!(align.is_power_of_two());
        assert!(align <= boundary);
        assert!(boundary.is_power_of_two());
        let interval = self
            .map
            .iter()
            .filter_map(|(interval, memory_type)| {
                if matches!(memory_type, MemoryType::Usable) {
                    Some(interval)
                } else {
                    None
                }
            })
            .find_map(|gap| {
                // Check for a valid region before the boundary
                {
                    let start = gap.start().next_multiple_of(align.get());
                    let interval = ie(start, start + size.get());
                    if gap.contains_interval(&interval)
                        && interval.start() / boundary == interval.end() / boundary
                    {
                        return Some(interval);
                    }
                }
                // Check for a valid region after the next boundary
                {
                    let start = gap.start().next_multiple_of(boundary.get());
                    let interval = ie(start, start + size.get());
                    if gap.contains_interval(&interval) {
                        return Some(interval);
                    }
                }
                None
            })?;
        // Interval is no longer Usable
        self.map.cut(interval);
        self.map
            .insert_merge_touching_if_values_equal(interval, memory_type)
            .unwrap();
        Some(PhysAddr::new(interval.start()))
    }

    pub fn get_kernel_frame_allocator(&mut self) -> PhysicalMemoryFrameAllocator<'_> {
        PhysicalMemoryFrameAllocator {
            physical_memory: self,
            memory_type: MemoryType::UsedByKernel(KernelMemoryUsageType::PageTables),
        }
    }

    pub fn get_user_mode_program_frame_allocator(
        &mut self,
        process_id: ProcessId,
    ) -> PhysicalMemoryFrameAllocator<'_> {
        PhysicalMemoryFrameAllocator {
            physical_memory: self,
            memory_type: MemoryType::UsedByUserMode(process_id),
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

    pub fn change_owner(&mut self, frame: Frame, new_owner: ProcessId) {
        let interval = Interval::from({
            let start = frame.start_addr().as_u64();
            start..start + frame.size().byte_len_u64()
        });
        let mut overlapping_mut = self.map.overlapping_mut(interval);
        let (_cut_interval, memory_type) = overlapping_mut.next().unwrap();
        assert_eq!(overlapping_mut.next(), None);
        match memory_type {
            MemoryType::UsedByUserMode(owner) => {
                *owner = new_owner;
            }
            _ => unreachable!(),
        }
    }

    pub fn remove(&mut self, value: &MemoryType) {
        let map = mem::take(&mut self.map);
        self.map = NoditMap::from_iter_strict(map.into_iter().filter(|(_interval, v)| v == value))
            .unwrap();
    }
}

pub struct PhysicalMemoryFrameAllocator<'a> {
    physical_memory: &'a mut PhysicalMemory,
    memory_type: MemoryType,
}

unsafe impl FrameAllocator<Size4KiB> for PhysicalMemoryFrameAllocator<'_> {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self
            .physical_memory
            .allocate_frame_with_type(PageSize::_4KiB, self.memory_type)?;
        Some(PhysFrame::from_start_address(frame.start_addr()).unwrap())
    }
}
