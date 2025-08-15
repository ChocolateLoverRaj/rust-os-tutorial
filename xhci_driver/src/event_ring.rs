use core::{
    mem::MaybeUninit,
    num::NonZero,
    ptr::{NonNull, slice_from_raw_parts_mut},
};

use split_slice::SplitSlice;
use volatile::VolatilePtr;

use crate::*;

pub struct EventRing2<'a> {
    mem: AllocResponse,
    ring: &'a mut [AnyTrb],
    dequeue_pointer: usize,
    consumer_cycle_state: bool,
}

impl EventRing2<'_> {
    pub fn new(len: usize, allocator: &mut impl XhciMemAllocator) -> Self {
        let event_ring_len = len;
        let event_ring_size = event_ring_len * size_of::<AnyTrb>();
        let event_ring_mem = allocator.alloc(AllocRequest {
            size: NonZero::new(event_ring_size as u64).unwrap(),
            align: XHCI_EVENT_RING_SEGMENTS_ALIGNMENT,
            boundary: XHCI_EVENT_RING_SEGMENTS_BOUNDARY,
        });
        let event_ring = {
            {
                let mut ptr = NonNull::new(slice_from_raw_parts_mut(
                    event_ring_mem.virt_addr.get() as *mut MaybeUninit<AnyTrb>,
                    event_ring_len,
                ))
                .unwrap();
                let event_ring_uninit = unsafe { ptr.as_mut() };
                // Initially when the TRB Ring is created in memory, or if it is ever re -initialized, all TRBs in the ring shall be cleared to ‘0’. This state represents an empty queue.
                event_ring_uninit.fill(MaybeUninit::zeroed());
            }
            let mut ptr = NonNull::new(slice_from_raw_parts_mut(
                event_ring_mem.virt_addr.get() as *mut AnyTrb,
                event_ring_len,
            ))
            .unwrap();
            unsafe { ptr.as_mut() }
        };
        Self {
            mem: event_ring_mem,
            ring: event_ring,
            dequeue_pointer: 0,
            consumer_cycle_state: true,
        }
    }

    pub fn phys_addr(&self) -> u64 {
        self.mem.phys_addr
    }

    pub fn len(&self) -> usize {
        self.ring.len()
    }

    pub fn update_erdp(&self, erdp: VolatilePtr<Erdp>) {
        erdp.update(|mut erdp| {
            erdp.set_event_ring_dequeue_pointer(self.mem.phys_addr);
            erdp
        });
    }

    pub fn peek(&self) -> SplitSlice<AnyTrb> {
        for (i, trb) in self.ring[self.dequeue_pointer..].iter().enumerate() {
            if trb.control.cycle_bit() != self.consumer_cycle_state {
                return SplitSlice(&self.ring[self.dequeue_pointer..i], &[]);
            }
        }
        // At this point we will loop around
        let next_cycle_state = !self.consumer_cycle_state;
        for (i, trb) in self.ring[..self.dequeue_pointer].iter().enumerate() {
            if trb.control.cycle_bit() != next_cycle_state {
                return SplitSlice(&self.ring[self.dequeue_pointer..], &self.ring[..i]);
            }
        }
        // At this point every single slot is full
        SplitSlice(
            &self.ring[self.dequeue_pointer..],
            &self.ring[..self.dequeue_pointer],
        )
    }

    pub fn advance_dequeue_pointer(&mut self, advance_len: usize, erdp: VolatilePtr<Erdp>) {
        // Check that we aren't advancing to an invalid state
        let max_advance_len = self.peek().len();
        assert!(
            advance_len <= max_advance_len,
            "must advance only the amount that was consumed"
        );
        self.dequeue_pointer =
            wrapping_add_custom(self.dequeue_pointer, advance_len, self.ring.len());
        erdp.update(|mut erdp| {
            erdp.set_event_ring_dequeue_pointer(
                self.mem.phys_addr + self.dequeue_pointer as u64 * size_of::<AnyTrb>() as u64,
            );
            // Tell the xHC that we can receive more interrupts
            erdp.set_event_handler_busy(true);
            erdp
        });
    }
}

/// Handles integer overflows too
fn wrapping_add_custom(position: usize, advance_len: usize, ring_size: usize) -> usize {
    let mut new_position = position;
    let mut len_left_to_advance = advance_len;
    {
        let advance_amount = (ring_size - position).min(len_left_to_advance);
        len_left_to_advance -= advance_amount;
        new_position += advance_amount;
    }
    if new_position == ring_size {
        new_position = len_left_to_advance;
    }
    new_position
}
