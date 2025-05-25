use core::ops::RangeInclusive;

use limine::{memory_map::EntryType, response::MemoryMapResponse};
use x86_64::{
    PhysAddr,
    structures::paging::{PageSize, PhysFrame, Size4KiB},
};

use crate::cut_range::CutRange;

pub struct InitialUsableFramesIterator {
    reserved_range: RangeInclusive<u64>,
    allocated_frames: u64,
    memory_map: &'static MemoryMapResponse,
}

impl InitialUsableFramesIterator {
    pub fn new(
        memory_map: &'static MemoryMapResponse,
        reserved_range: RangeInclusive<u64>,
    ) -> Self {
        Self {
            reserved_range,
            allocated_frames: 0,
            memory_map,
        }
    }

    pub fn allocated_frames(&self) -> u64 {
        self.allocated_frames
    }
}

impl Iterator for InitialUsableFramesIterator {
    type Item = PhysFrame<Size4KiB>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut skipped_frames = 0;
        let frame_start = self
            .memory_map
            .entries()
            .iter()
            .filter_map(|entry| {
                if entry.entry_type == EntryType::USABLE {
                    Some(entry.base..=entry.base + (entry.length - 1))
                } else {
                    None
                }
            })
            .flat_map(|range| range.cut(&self.reserved_range))
            .find_map(|entry| {
                let first_frame_start = entry.start().next_multiple_of(Size4KiB::SIZE);
                let full_frames = (entry.end() - first_frame_start + 1) / Size4KiB::SIZE;
                let frames_left_to_skip = self.allocated_frames - skipped_frames;
                let frames_skipped_in_this_entry = frames_left_to_skip.min(full_frames);
                skipped_frames += frames_skipped_in_this_entry;
                if frames_skipped_in_this_entry < full_frames {
                    Some(first_frame_start + frames_skipped_in_this_entry * Size4KiB::SIZE)
                } else {
                    None
                }
            })?;
        self.allocated_frames += 1;
        Some(PhysFrame::from_start_address(PhysAddr::new(frame_start)).unwrap())
    }
}
