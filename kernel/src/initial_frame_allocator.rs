use core::ops::RangeInclusive;

use limine::response::MemoryMapResponse;
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};

use crate::initial_usable_frames_iterator::InitialUsableFramesIterator;

pub struct InitialFrameAllocator {
    iterator: InitialUsableFramesIterator,
    memory_map: &'static MemoryMapResponse,
    reserved_range: RangeInclusive<u64>,
}

impl InitialFrameAllocator {
    /// # Safety
    /// You must not accidentally create two of these, because that will allocate the same frames
    pub unsafe fn new(
        memory_map: &'static MemoryMapResponse,
        reserved_range: RangeInclusive<u64>,
    ) -> Self {
        Self {
            iterator: InitialUsableFramesIterator::new(memory_map, reserved_range.clone()),
            memory_map,
            reserved_range,
        }
    }

    /// Finish using this as a frame allocator, and get an iterator of allocated frames so that you can mark them as used
    pub fn finish(self) -> impl Iterator<Item = PhysFrame<Size4KiB>> {
        InitialUsableFramesIterator::new(self.memory_map, self.reserved_range)
            .take(self.iterator.allocated_frames() as usize)
    }
}

unsafe impl FrameAllocator<Size4KiB> for InitialFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.iterator.next()
    }
}
